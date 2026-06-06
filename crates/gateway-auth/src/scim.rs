//! SCIM 2.0 types and authentication.

use serde::{Deserialize, Serialize};

// ── SCIM Resource Types ──────────────────────────────────────────────

/// SCIM User resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimUser {
    pub schemas: Vec<String>,
    pub id: String,
    pub user_name: String,
    pub name: Option<ScimName>,
    pub display_name: Option<String>,
    pub emails: Option<Vec<ScimEmail>>,
    pub active: bool,
    pub groups: Option<Vec<ScimGroupRef>>,
    pub meta: ScimMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimName {
    pub formatted: Option<String>,
    pub family_name: Option<String>,
    pub given_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimEmail {
    pub value: String,
    #[serde(rename = "type")]
    pub email_type: Option<String>,
    pub primary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimGroupRef {
    pub value: String,
    pub display: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimMeta {
    pub resource_type: String,
    pub created: Option<String>,
    pub last_modified: Option<String>,
    pub location: Option<String>,
}

/// SCIM ListResponse for paginated results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimListResponse<T> {
    pub schemas: Vec<String>,
    pub total_results: usize,
    pub start_index: usize,
    pub items_per_page: usize,
    pub resources: Vec<T>,
}

/// SCIM ServiceProviderConfig.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimServiceProviderConfig {
    pub schemas: Vec<String>,
    pub documentation_uri: Option<String>,
    pub patch: ScimSupported,
    pub bulk: ScimBulkSupport,
    pub filter: ScimSupported,
    pub change_password: ScimSupported,
    pub sort: ScimSupported,
    pub etag: ScimSupported,
    pub authentication_schemes: Vec<ScimAuthScheme>,
    pub meta: ScimMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimSupported {
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimBulkSupport {
    pub supported: bool,
    pub max_operations: usize,
    pub max_payload_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimAuthScheme {
    pub name: String,
    pub description: String,
    pub spec_uri: Option<String>,
    pub documentation_uri: Option<String>,
    #[serde(rename = "type")]
    pub auth_type: String,
}

/// SCIM ResourceType.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimResourceType {
    pub schemas: Vec<String>,
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub description: String,
    pub schema: String,
    pub schema_extensions: Vec<ScimSchemaExtension>,
    pub meta: ScimMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimSchemaExtension {
    pub schema: String,
    pub required: bool,
}

/// SCIM Schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimSchema {
    pub schemas: Vec<String>,
    pub id: String,
    pub name: String,
    pub description: String,
    pub attributes: Vec<ScimAttribute>,
    pub meta: ScimMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimAttribute {
    pub name: String,
    #[serde(rename = "type")]
    pub attr_type: String,
    pub multi_valued: bool,
    pub required: bool,
    pub case_exact: bool,
    pub mutability: String,
    pub returned: String,
    pub uniqueness: String,
}

/// SCIM Group resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimGroup {
    pub schemas: Vec<String>,
    pub id: String,
    pub display_name: String,
    pub members: Option<Vec<ScimMember>>,
    pub meta: ScimMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScimMember {
    pub value: String,
    pub display: Option<String>,
}

/// SCIM Error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimError {
    pub schemas: Vec<String>,
    pub status: String,
    pub detail: Option<String>,
}

// ── Request / Response Helpers ───────────────────────────────────────

impl ScimUser {
    pub fn new(id: &str, user_name: &str, active: bool) -> Self {
        Self {
            schemas: vec!["urn:ietf:params:scim:schemas:core:2.0:User".to_string()],
            id: id.to_string(),
            user_name: user_name.to_string(),
            name: None,
            display_name: None,
            emails: None,
            active,
            groups: None,
            meta: ScimMeta {
                resource_type: "User".to_string(),
                created: None,
                last_modified: None,
                location: Some(format!("/scim/v2/Users/{}", id)),
            },
        }
    }
}

impl<T> ScimListResponse<T> {
    pub fn new(resources: Vec<T>, total: usize, start_index: usize, count: usize) -> Self {
        Self {
            schemas: vec!["urn:ietf:params:scim:api:messages:2.0:ListResponse".to_string()],
            total_results: total,
            start_index,
            items_per_page: count,
            resources,
        }
    }
}

impl ScimError {
    pub fn new(status: u16, detail: &str) -> Self {
        Self {
            schemas: vec!["urn:ietf:params:scim:api:messages:2.0:Error".to_string()],
            status: status.to_string(),
            detail: Some(detail.to_string()),
        }
    }
}

/// Build the ServiceProviderConfig response.
pub fn service_provider_config(base_url: &str) -> ScimServiceProviderConfig {
    ScimServiceProviderConfig {
        schemas: vec!["urn:ietf:params:scim:schemas:core:2.0:ServiceProviderConfig".to_string()],
        documentation_uri: Some(format!("{}/docs/scim", base_url)),
        patch: ScimSupported { supported: true },
        bulk: ScimBulkSupport {
            supported: false,
            max_operations: 0,
            max_payload_size: 0,
        },
        filter: ScimSupported { supported: true },
        change_password: ScimSupported { supported: false },
        sort: ScimSupported { supported: false },
        etag: ScimSupported { supported: false },
        authentication_schemes: vec![ScimAuthScheme {
            name: "Bearer".to_string(),
            description: "Authentication scheme using the Bearer standard".to_string(),
            spec_uri: Some("https://www.rfc-editor.org/info/rfc6750".to_string()),
            documentation_uri: None,
            auth_type: "oauthbearertoken".to_string(),
        }],
        meta: ScimMeta {
            resource_type: "ServiceProviderConfig".to_string(),
            created: None,
            last_modified: None,
            location: Some("/scim/v2/ServiceProviderConfig".to_string()),
        },
    }
}

/// Build the ResourceTypes response.
pub fn resource_types() -> Vec<ScimResourceType> {
    vec![
        ScimResourceType {
            schemas: vec!["urn:ietf:params:scim:schemas:core:2.0:ResourceType".to_string()],
            id: "User".to_string(),
            name: "User".to_string(),
            endpoint: "/scim/v2/Users".to_string(),
            description: "User Account".to_string(),
            schema: "urn:ietf:params:scim:schemas:core:2.0:User".to_string(),
            schema_extensions: vec![],
            meta: ScimMeta {
                resource_type: "ResourceType".to_string(),
                created: None,
                last_modified: None,
                location: Some("/scim/v2/ResourceTypes/User".to_string()),
            },
        },
        ScimResourceType {
            schemas: vec!["urn:ietf:params:scim:schemas:core:2.0:ResourceType".to_string()],
            id: "Group".to_string(),
            name: "Group".to_string(),
            endpoint: "/scim/v2/Groups".to_string(),
            description: "Group".to_string(),
            schema: "urn:ietf:params:scim:schemas:core:2.0:Group".to_string(),
            schema_extensions: vec![],
            meta: ScimMeta {
                resource_type: "ResourceType".to_string(),
                created: None,
                last_modified: None,
                location: Some("/scim/v2/ResourceTypes/Group".to_string()),
            },
        },
    ]
}

/// Build the Schemas response.
pub fn schemas() -> Vec<ScimSchema> {
    vec![ScimSchema {
        schemas: vec!["urn:ietf:params:scim:schemas:core:2.0:Schema".to_string()],
        id: "urn:ietf:params:scim:schemas:core:2.0:User".to_string(),
        name: "User".to_string(),
        description: "User Account".to_string(),
        attributes: vec![
            ScimAttribute {
                name: "userName".to_string(),
                attr_type: "string".to_string(),
                multi_valued: false,
                required: true,
                case_exact: false,
                mutability: "readWrite".to_string(),
                returned: "default".to_string(),
                uniqueness: "server".to_string(),
            },
            ScimAttribute {
                name: "name".to_string(),
                attr_type: "complex".to_string(),
                multi_valued: false,
                required: false,
                case_exact: false,
                mutability: "readWrite".to_string(),
                returned: "default".to_string(),
                uniqueness: "none".to_string(),
            },
            ScimAttribute {
                name: "emails".to_string(),
                attr_type: "complex".to_string(),
                multi_valued: true,
                required: false,
                case_exact: false,
                mutability: "readWrite".to_string(),
                returned: "default".to_string(),
                uniqueness: "none".to_string(),
            },
            ScimAttribute {
                name: "active".to_string(),
                attr_type: "boolean".to_string(),
                multi_valued: false,
                required: false,
                case_exact: false,
                mutability: "readWrite".to_string(),
                returned: "default".to_string(),
                uniqueness: "none".to_string(),
            },
        ],
        meta: ScimMeta {
            resource_type: "Schema".to_string(),
            created: None,
            last_modified: None,
            location: Some(
                "/scim/v2/Schemas/urn:ietf:params:scim:schemas:core:2.0:User".to_string(),
            ),
        },
    }]
}
