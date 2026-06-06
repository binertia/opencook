//! SSO configuration repository.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;

/// SSO provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "lowercase")]
pub enum SsoProviderType {
    Saml,
    Oidc,
}

/// SSO configuration record.
#[derive(Debug, Clone)]
pub struct SsoConfig {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider_type: SsoProviderType,
    pub metadata_url: Option<String>,
    pub entity_id: Option<String>,
    pub certificate: Option<String>,
    pub sso_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret_enc: Option<String>,
    pub idp_issuer: Option<String>,
    pub role_attribute: String,
    pub enabled: bool,
}

/// Repository for SSO configurations.
#[derive(Clone)]
pub struct SsoConfigRepo {
    pool: PgPool,
}

impl SsoConfigRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_by_org(&self, org_id: Uuid) -> Result<Vec<SsoConfig>, DbError> {
        let rows = sqlx::query_as::<_, SsoConfigRow>(
            r#"
            SELECT id, org_id, provider_type, metadata_url, entity_id, certificate,
                   sso_url, client_id, client_secret_enc, idp_issuer, role_attribute, enabled
            FROM sso_configs
            WHERE org_id = $1
            ORDER BY provider_type
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn get_by_org_and_type(
        &self,
        org_id: Uuid,
        provider_type: SsoProviderType,
    ) -> Result<Option<SsoConfig>, DbError> {
        let row = sqlx::query_as::<_, SsoConfigRow>(
            r#"
            SELECT id, org_id, provider_type, metadata_url, entity_id, certificate,
                   sso_url, client_id, client_secret_enc, idp_issuer, role_attribute, enabled
            FROM sso_configs
            WHERE org_id = $1 AND provider_type = $2
            "#,
        )
        .bind(org_id)
        .bind(provider_type)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn upsert(&self, config: &SsoConfig) -> Result<SsoConfig, DbError> {
        let row = sqlx::query_as::<_, SsoConfigRow>(
            r#"
            INSERT INTO sso_configs (org_id, provider_type, metadata_url, entity_id, certificate,
                                     sso_url, client_id, client_secret_enc, idp_issuer, role_attribute, enabled)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (org_id, provider_type) DO UPDATE SET
                metadata_url = EXCLUDED.metadata_url,
                entity_id = EXCLUDED.entity_id,
                certificate = EXCLUDED.certificate,
                sso_url = EXCLUDED.sso_url,
                client_id = EXCLUDED.client_id,
                client_secret_enc = EXCLUDED.client_secret_enc,
                idp_issuer = EXCLUDED.idp_issuer,
                role_attribute = EXCLUDED.role_attribute,
                enabled = EXCLUDED.enabled,
                updated_at = NOW()
            RETURNING id, org_id, provider_type, metadata_url, entity_id, certificate,
                      sso_url, client_id, client_secret_enc, idp_issuer, role_attribute, enabled
            "#,
        )
        .bind(config.org_id)
        .bind(config.provider_type)
        .bind(&config.metadata_url)
        .bind(&config.entity_id)
        .bind(&config.certificate)
        .bind(&config.sso_url)
        .bind(&config.client_id)
        .bind(&config.client_secret_enc)
        .bind(&config.idp_issuer)
        .bind(&config.role_attribute)
        .bind(config.enabled)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    pub async fn delete(
        &self,
        org_id: Uuid,
        provider_type: SsoProviderType,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            DELETE FROM sso_configs
            WHERE org_id = $1 AND provider_type = $2
            "#,
        )
        .bind(org_id)
        .bind(provider_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct SsoConfigRow {
    id: Uuid,
    org_id: Uuid,
    provider_type: SsoProviderType,
    metadata_url: Option<String>,
    entity_id: Option<String>,
    certificate: Option<String>,
    sso_url: Option<String>,
    client_id: Option<String>,
    client_secret_enc: Option<String>,
    idp_issuer: Option<String>,
    role_attribute: String,
    enabled: bool,
}

impl From<SsoConfigRow> for SsoConfig {
    fn from(r: SsoConfigRow) -> Self {
        Self {
            id: r.id,
            org_id: r.org_id,
            provider_type: r.provider_type,
            metadata_url: r.metadata_url,
            entity_id: r.entity_id,
            certificate: r.certificate,
            sso_url: r.sso_url,
            client_id: r.client_id,
            client_secret_enc: r.client_secret_enc,
            idp_issuer: r.idp_issuer,
            role_attribute: r.role_attribute,
            enabled: r.enabled,
        }
    }
}
