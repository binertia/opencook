//! Test data generators (fixtures) for E2E tests.
#![allow(dead_code)]

use gateway_db::DbBackend;
use uuid::Uuid;

/// Create a default organization and return its ID.
pub async fn create_org(db: &DbBackend, name: &str) -> Uuid {
    let pool = db.sqlite();
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO organizations (id, name, slug, status, settings, billing_email, plan_tier)
        VALUES (?1, ?2, ?3, 'active', '{}', NULL, 'free')
        "#
    )
    .bind(id.as_bytes().as_slice())
    .bind(name)
    .bind(name.to_lowercase().replace(" ", "-"))
    .execute(pool)
    .await
    .expect("failed to insert org");

    id
}

/// Create a user and return its ID.
pub async fn create_user(db: &DbBackend, org_id: Uuid, email: &str, role: &str) -> Uuid {
    let pool = db.sqlite();
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
        VALUES (?1, ?2, ?3, 'hash', 'Test User', ?4, 'active')
        "#
    )
    .bind(id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind(email)
    .bind(role)
    .execute(pool)
    .await
    .expect("failed to insert user");

    id
}

/// Create an API key and return its ID.
pub async fn create_api_key(db: &DbBackend, org_id: Uuid, user_id: Uuid, name: &str, key_hash: &str) -> Uuid {
    let pool = db.sqlite();
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO api_keys (id, org_id, user_id, name, key_hash, key_prefix, scopes, rate_limit_rps, status)
        VALUES (?1, ?2, ?3, ?4, ?5, 'sk_test', '["chat:write"]', 100, 'active')
        "#
    )
    .bind(id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind(user_id.as_bytes().as_slice())
    .bind(name)
    .bind(key_hash)
    .execute(pool)
    .await
    .expect("failed to insert api key");

    id
}

/// Create a provider config and return its ID.
pub async fn create_provider_config(
    db: &DbBackend,
    org_id: Uuid,
    name: &str,
    kind: &str,
    api_base: &str,
) -> Uuid {
    let pool = db.sqlite();
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO provider_configs (id, org_id, name, kind, api_base, api_key_enc, default_headers, config, priority, status)
        VALUES (?1, ?2, ?3, ?4, ?5, X'00', '{}', '{}', 0, 'active')
        "#
    )
    .bind(id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind(name)
    .bind(kind)
    .bind(api_base)
    .execute(pool)
    .await
    .expect("failed to insert provider config");

    id
}

/// Create a provider model and return its ID.
pub async fn create_provider_model(
    db: &DbBackend,
    org_id: Uuid,
    provider_config_id: Uuid,
    model_id: &str,
    model_name: &str,
) -> Uuid {
    let pool = db.sqlite();
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO provider_models (id, org_id, provider_config_id, model_id, model_name, aliases, input_cost_per_1k, output_cost_per_1k, context_window, max_tokens, supports_streaming, supports_tools, supports_vision, status, config)
        VALUES (?1, ?2, ?3, ?4, ?5, '[]', '0', '0', 128000, 4096, 1, 0, 0, 'active', '{}')
        "#
    )
    .bind(id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind(provider_config_id.as_bytes().as_slice())
    .bind(model_id)
    .bind(model_name)
    .execute(pool)
    .await
    .expect("failed to insert provider model");

    id
}

/// Create a routing rule.
pub async fn create_routing_rule(
    db: &DbBackend,
    org_id: Uuid,
    name: &str,
    strategy: &str,
    match_model: Option<&str>,
) -> Uuid {
    let pool = db.sqlite();
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO routing_rules (id, org_id, name, strategy, priority, match_model, match_tags, conditions, targets, timeout_ms, retries, status)
        VALUES (?1, ?2, ?3, ?4, 0, ?5, '[]', '{}', '{}', 30000, 1, 'active')
        "#
    )
    .bind(id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind(name)
    .bind(strategy)
    .bind(match_model)
    .execute(pool)
    .await
    .expect("failed to insert routing rule");

    id
}

/// Create a quota and return its ID.
pub async fn create_quota(
    db: &DbBackend,
    org_id: Uuid,
    name: &str,
    metric: &str,
    period: &str,
    limit_value: &str,
) -> Uuid {
    let pool = db.sqlite();
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO quotas (id, org_id, name, metric, period, limit_value, warning_threshold, applies_to, scope_filter, action, status)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, '0.8', 'all', '{}', 'block', 'active')
        "#
    )
    .bind(id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind(name)
    .bind(metric)
    .bind(period)
    .bind(limit_value)
    .execute(pool)
    .await
    .expect("failed to insert quota");

    id
}

/// Count requests in the DB.
pub async fn count_requests(db: &DbBackend) -> i64 {
    let pool = db.sqlite();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM requests")
        .fetch_one(pool)
        .await
        .expect("failed to count requests");
    count.0
}
