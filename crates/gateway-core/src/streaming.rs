//! SSE streaming helpers — wraps provider streams with gateway-side logging.

use std::pin::Pin;
use std::task::{Context, Poll};

use axum::response::sse::Event;
use futures::StreamExt;
use futures::Stream;
use gateway_db::RequestRepo;
use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error};
use uuid::Uuid;

/// Wraps a provider SSE stream and updates the request log when the stream
/// ends or the client disconnects.
pub struct LoggingStream {
    inner: ReceiverStream<Result<Event, String>>,
    db_pool: Option<PgPool>,
    request_id: Option<Uuid>,
    org_id: Option<Uuid>,
    completed: bool,
    start: std::time::Instant,
    estimated_prompt_tokens: u64,
    estimated_completion_tokens: u64,
    model: String,
}

impl LoggingStream {
    pub fn new(
        inner: ReceiverStream<Result<Event, String>>,
        db_pool: PgPool,
        request_id: Uuid,
        org_id: Uuid,
        model: String,
        estimated_prompt_tokens: u64,
        estimated_completion_tokens: u64,
    ) -> Self {
        Self {
            inner,
            db_pool: Some(db_pool),
            request_id: Some(request_id),
            org_id: Some(org_id),
            completed: false,
            start: std::time::Instant::now(),
            estimated_prompt_tokens,
            estimated_completion_tokens,
            model,
        }
    }

    fn finish(&mut self, status: &str, status_code: Option<i32>, error_code: Option<&str>, error_message: Option<&str>) {
        if self.completed {
            return;
        }
        self.completed = true;

        let Some(pool) = self.db_pool.take() else { return };
        let Some(req_id) = self.request_id.take() else { return };
        let Some(org_id) = self.org_id.take() else { return };
        let latency_ms = self.start.elapsed().as_millis() as i32;
        let model = self.model.clone();
        let prompt_tokens = self.estimated_prompt_tokens as i32;
        let completion_tokens = self.estimated_completion_tokens as i32;
        let total_tokens = prompt_tokens + completion_tokens;

        // Estimate cost using the same pricing as the orchestrator
        let (input_price, output_price) = super::orchestrator::model_pricing(&model);
        let input_cost = self.estimated_prompt_tokens as f64 * input_price / 1_000_000.0;
        let output_cost = self.estimated_completion_tokens as f64 * output_price / 1_000_000.0;
        let total_cost = input_cost + output_cost;

        let status = status.to_string();
        let error_code = error_code.map(|s| s.to_string());
        let error_message = error_message.map(|s| s.to_string());

        tokio::spawn(async move {
            let repo = RequestRepo::new(pool);
            if let Err(e) = repo
                .update_response(
                    req_id,
                    org_id,
                    Some(&model),
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                    Decimal::try_from(input_cost).unwrap_or_default(),
                    Decimal::try_from(output_cost).unwrap_or_default(),
                    Decimal::try_from(total_cost).unwrap_or_default(),
                    &status,
                    status_code,
                    error_code.as_deref(),
                    error_message.as_deref(),
                    latency_ms,
                    latency_ms,
                    false,
                )
                .await
            {
                error!(error = %e, "Failed to update streaming request record");
            } else {
                debug!(request_id = %req_id, status = %status, "Streaming request record updated");
            }
        });
    }
}

impl Stream for LoggingStream {
    type Item = Result<Event, String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.poll_next_unpin(cx) {
            Poll::Ready(None) => {
                self.finish("success", Some(200), None, None);
                Poll::Ready(None)
            }
            Poll::Ready(Some(Err(e))) => {
                self.finish("error", Some(502), Some("provider_error"), Some(&e));
                Poll::Ready(Some(Err(e)))
            }
            other => other,
        }
    }
}

impl Drop for LoggingStream {
    fn drop(&mut self) {
        if !self.completed {
            self.finish("error", Some(499), Some("client_disconnect"), Some("Client disconnected before stream completed"));
        }
    }
}
