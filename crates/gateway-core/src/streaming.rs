//! SSE streaming helpers — wraps provider streams with gateway-side logging.

use std::pin::Pin;
use std::task::{Context, Poll};

use axum::response::sse::Event;
use futures::{Stream, StreamExt};
use gateway_db::DbBackend;
use gateway_db::RequestRepo;
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use uuid::Uuid;

/// Wraps a provider SSE stream and updates the request log when the stream
/// ends or the client disconnects.
pub struct LoggingStream<S> {
    inner: S,
    db_pool: Option<DbBackend>,
    request_id: Option<Uuid>,
    org_id: Option<Uuid>,
    completed: bool,
    start: std::time::Instant,
    estimated_prompt_tokens: u64,
    estimated_completion_tokens: u64,
    model: String,
    cancel_token: CancellationToken,
}

impl<S> LoggingStream<S> {
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

impl<S> LoggingStream<S>
where
    S: Stream<Item = Result<Event, String>> + Send + 'static,
{
    pub fn new(
        inner: S,
        db_pool: DbBackend,
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
            cancel_token: CancellationToken::new(),
        }
    }

    /// Get a clone of the cancellation token to pass to spawned tasks.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
}

impl<S> Stream for LoggingStream<S>
where
    S: Stream<Item = Result<Event, String>> + Unpin,
{
    type Item = Result<Event, String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check cancellation first
        if self.cancel_token.is_cancelled() {
            self.finish("cancelled", Some(499), Some("client_disconnect"), Some("Request cancelled by client disconnect"));
            return Poll::Ready(None);
        }

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

impl<S> Drop for LoggingStream<S> {
    fn drop(&mut self) {
        if !self.completed {
            self.cancel_token.cancel();
            self.finish("error", Some(499), Some("client_disconnect"), Some("Client disconnected before stream completed"));
        }
    }
}
