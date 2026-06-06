//! Model registry repository — queries provider_models with provider_configs.

use crate::pool::DbBackend;
use crate::types::DbDecimal;
use tracing::debug;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{Capabilities, ModelEntry, PricingInfo, ProviderModel};

/// Repository for the model registry.
#[derive(Clone)]
pub struct ModelRegistry {
    pool: DbBackend,
}

impl ModelRegistry {
    /// Create a new model registry repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// List all active models for an organization.
    pub async fn list_models(&self, org_id: Uuid) -> Result<Vec<ModelEntry>, DbError> {
        let sql = r#"
            SELECT
                pm.id, pm.org_id, pm.provider_config_id,
                pm.model_id, pm.model_name, pm.aliases,
                pm.input_cost_per_1k, pm.output_cost_per_1k,
                pm.context_window, pm.max_tokens,
                pm.supports_streaming, pm.supports_tools, pm.supports_vision,
                pm.status, pm.config,
                pm.created_at, pm.updated_at, pm.deleted_at
            FROM provider_models pm
            WHERE pm.org_id = $1
              AND pm.status = 'active'
              AND pm.deleted_at IS NULL
            ORDER BY pm.model_name
            "#;
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, ProviderModel>(sql)
                    .bind(org_id)
                    .fetch_all(pg)
                    .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, ProviderModel>(sql)
                    .bind(org_id)
                    .fetch_all(sqlite)
                    .await?
            }
        };

        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let provider_name = self
                .get_provider_name(org_id, row.provider_config_id)
                .await?;
            entries.push(row_to_entry(row, provider_name));
        }

        debug!(org_id = %org_id, count = entries.len(), "Listed models from registry");
        Ok(entries)
    }

    /// Get a specific model by ID.
    pub async fn get_model(
        &self,
        org_id: Uuid,
        model_id: &str,
    ) -> Result<Option<ModelEntry>, DbError> {
        let sql = r#"
            SELECT
                pm.id, pm.org_id, pm.provider_config_id,
                pm.model_id, pm.model_name, pm.aliases,
                pm.input_cost_per_1k, pm.output_cost_per_1k,
                pm.context_window, pm.max_tokens,
                pm.supports_streaming, pm.supports_tools, pm.supports_vision,
                pm.status, pm.config,
                pm.created_at, pm.updated_at, pm.deleted_at
            FROM provider_models pm
            WHERE pm.org_id = $1
              AND pm.model_id = $2
              AND pm.status = 'active'
              AND pm.deleted_at IS NULL
            "#;
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, ProviderModel>(sql)
                    .bind(org_id)
                    .bind(model_id)
                    .fetch_optional(pg)
                    .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, ProviderModel>(sql)
                    .bind(org_id)
                    .bind(model_id)
                    .fetch_optional(sqlite)
                    .await?
            }
        };

        match row {
            Some(pm) => {
                let provider_name = self
                    .get_provider_name(org_id, pm.provider_config_id)
                    .await?;
                Ok(Some(row_to_entry(pm, provider_name)))
            }
            None => Ok(None),
        }
    }

    /// Resolve an alias to a model_id.
    pub async fn resolve_alias(
        &self,
        org_id: Uuid,
        alias: &str,
    ) -> Result<Option<String>, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row: Option<(String,)> = sqlx::query_as(
                    r#"
                    SELECT pm.model_id
                    FROM provider_models pm
                    WHERE pm.org_id = $1
                      AND ($2 = ANY(pm.aliases) OR pm.model_id = $2)
                      AND pm.status = 'active'
                      AND pm.deleted_at IS NULL
                    LIMIT 1
                    "#,
                )
                .bind(org_id)
                .bind(alias)
                .fetch_optional(pg)
                .await?;
                Ok(row.map(|r| r.0))
            }
            DbBackend::Sqlite(sqlite) => {
                // SQLite: aliases stored as JSON TEXT, use LIKE for matching
                let pattern = format!("%\"{}\"%", alias);
                let row: Option<(String,)> = sqlx::query_as(
                    r#"
                    SELECT pm.model_id
                    FROM provider_models pm
                    WHERE pm.org_id = $1
                      AND (pm.aliases LIKE $2 OR pm.model_id = $3)
                      AND pm.status = 'active'
                      AND pm.deleted_at IS NULL
                    LIMIT 1
                    "#,
                )
                .bind(org_id)
                .bind(&pattern)
                .bind(alias)
                .fetch_optional(sqlite)
                .await?;
                Ok(row.map(|r| r.0))
            }
        }
    }

    /// Get pricing for a specific model.
    pub async fn get_pricing(
        &self,
        org_id: Uuid,
        model_id: &str,
    ) -> Result<Option<PricingInfo>, DbError> {
        let sql = r#"
            SELECT pm.input_cost_per_1k, pm.output_cost_per_1k
            FROM provider_models pm
            WHERE pm.org_id = $1
              AND pm.model_id = $2
              AND pm.status = 'active'
              AND pm.deleted_at IS NULL
            "#;
        let row: Option<(DbDecimal, DbDecimal)> = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as(sql)
                    .bind(org_id)
                    .bind(model_id)
                    .fetch_optional(pg)
                    .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as(sql)
                    .bind(org_id)
                    .bind(model_id)
                    .fetch_optional(sqlite)
                    .await?
            }
        };

        Ok(row.map(|(input, output)| PricingInfo {
            input_cost_per_1k: input,
            output_cost_per_1k: output,
        }))
    }

    /// Create a provider model entry.
    pub async fn create_model(
        &self,
        org_id: Uuid,
        provider_config_id: Uuid,
        model_id: &str,
        model_name: &str,
    ) -> Result<ProviderModel, DbError> {
        let sql = r#"
            INSERT INTO provider_models (
                org_id, provider_config_id, model_id, model_name,
                aliases, input_cost_per_1k, output_cost_per_1k,
                context_window, max_tokens, supports_streaming, supports_tools, supports_vision,
                status, config
            )
            VALUES ($1, $2, $3, $4, '[]', '0', '0', NULL, NULL, 1, 0, 0, 'active', '{}')
            RETURNING *
            "#;
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, ProviderModel>(sql)
                    .bind(org_id)
                    .bind(provider_config_id)
                    .bind(model_id)
                    .bind(model_name)
                    .fetch_one(pg)
                    .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, ProviderModel>(sql)
                    .bind(org_id)
                    .bind(provider_config_id)
                    .bind(model_id)
                    .bind(model_name)
                    .fetch_one(sqlite)
                    .await?
            }
        };
        Ok(row)
    }

    /// Update pricing for a specific model.
    pub async fn update_pricing(
        &self,
        org_id: Uuid,
        provider_config_id: Uuid,
        model_id: &str,
        input_cost_per_1k: rust_decimal::Decimal,
        output_cost_per_1k: rust_decimal::Decimal,
    ) -> Result<(), DbError> {
        if input_cost_per_1k < rust_decimal::Decimal::ZERO
            || output_cost_per_1k < rust_decimal::Decimal::ZERO
        {
            return Err(DbError::Unsupported(
                "Pricing must be non-negative".to_string(),
            ));
        }
        let max_price = rust_decimal::Decimal::from(1);
        if input_cost_per_1k > max_price || output_cost_per_1k > max_price {
            return Err(DbError::Unsupported(
                "Pricing must be at most $1.00 per 1k tokens".to_string(),
            ));
        }

        let sql = r#"
            UPDATE provider_models
            SET input_cost_per_1k = $1,
                output_cost_per_1k = $2,
                updated_at = NOW()
            WHERE org_id = $3
              AND provider_config_id = $4
              AND model_id = $5
              AND deleted_at IS NULL
            "#;
        let rows_affected = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(sql)
                    .bind(DbDecimal::new(input_cost_per_1k))
                    .bind(DbDecimal::new(output_cost_per_1k))
                    .bind(org_id)
                    .bind(provider_config_id)
                    .bind(model_id)
                    .execute(pg)
                    .await?
                    .rows_affected()
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE provider_models SET input_cost_per_1k = $1, output_cost_per_1k = $2, updated_at = datetime('now') WHERE org_id = $3 AND provider_config_id = $4 AND model_id = $5 AND deleted_at IS NULL"
                )
                .bind(DbDecimal::new(input_cost_per_1k))
                .bind(DbDecimal::new(output_cost_per_1k))
                .bind(org_id)
                .bind(provider_config_id)
                .bind(model_id)
                .execute(sqlite)
                .await?
                .rows_affected()
            }
        };

        if rows_affected == 0 {
            return Err(DbError::NotFound("provider model".to_string()));
        }

        debug!(org_id = %org_id, model_id = %model_id, "Updated model pricing");
        Ok(())
    }

    /// Delete all models for a provider config.
    pub async fn delete_models_by_provider(
        &self,
        org_id: Uuid,
        provider_config_id: Uuid,
    ) -> Result<(), DbError> {
        let sql = r#"
            UPDATE provider_models
            SET deleted_at = NOW(), status = 'inactive'
            WHERE org_id = $1 AND provider_config_id = $2 AND deleted_at IS NULL
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(sql)
                    .bind(org_id)
                    .bind(provider_config_id)
                    .execute(pg)
                    .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE provider_models SET deleted_at = datetime('now'), status = 'inactive' WHERE org_id = $1 AND provider_config_id = $2 AND deleted_at IS NULL"
                )
                .bind(org_id)
                .bind(provider_config_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }

    /// Get provider name from provider_configs.
    async fn get_provider_name(
        &self,
        org_id: Uuid,
        provider_config_id: Uuid,
    ) -> Result<String, DbError> {
        let sql = r#"
            SELECT name
            FROM provider_configs
            WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
            "#;
        let row: Option<(String,)> = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as(sql)
                    .bind(provider_config_id)
                    .bind(org_id)
                    .fetch_optional(pg)
                    .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as(sql)
                    .bind(provider_config_id)
                    .bind(org_id)
                    .fetch_optional(sqlite)
                    .await?
            }
        };

        Ok(row.map(|r| r.0).unwrap_or_else(|| "unknown".to_string()))
    }
}

/// Convert a ProviderModel row to a ModelEntry.
fn row_to_entry(row: ProviderModel, provider_name: String) -> ModelEntry {
    ModelEntry {
        model_id: row.model_id,
        model_name: row.model_name,
        provider_config_id: row.provider_config_id,
        provider_name,
        provider_kind: row.provider_config_id.to_string(), // placeholder
        aliases: row.aliases,
        pricing: PricingInfo {
            input_cost_per_1k: row.input_cost_per_1k,
            output_cost_per_1k: row.output_cost_per_1k,
        },
        capabilities: Capabilities {
            streaming: row.supports_streaming,
            tools: row.supports_tools,
            vision: row.supports_vision,
            json_mode: true, // default
            max_context: row.context_window,
            max_tokens: row.max_tokens,
        },
        status: row.status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_to_entry() {
        let row = ProviderModel {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            provider_config_id: Uuid::new_v4(),
            model_id: "gpt-4o".to_string(),
            model_name: "GPT-4o".to_string(),
            aliases: vec!["gpt-4".to_string()].into(),
            input_cost_per_1k: DbDecimal::new(rust_decimal::Decimal::new(5, 3)),
            output_cost_per_1k: DbDecimal::new(rust_decimal::Decimal::new(15, 3)),
            context_window: Some(128000),
            max_tokens: Some(4096),
            supports_streaming: true,
            supports_tools: true,
            supports_vision: true,
            status: "active".to_string(),
            config: serde_json::json!({"json_mode": true}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let entry = row_to_entry(row, "openai".to_string());
        assert_eq!(entry.model_id, "gpt-4o");
        assert_eq!(
            entry.pricing.input_cost_per_1k.into_inner(),
            rust_decimal::Decimal::new(5, 3)
        );
    }
}
