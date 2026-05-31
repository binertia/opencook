//! Role-based access control (RBAC) engine.

use serde::{Deserialize, Serialize};

/// Dashboard user roles within an organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Owner,
    Admin,
    Member,
    Viewer,
}

impl Role {
    /// Parse role from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "owner" => Some(Role::Owner),
            "admin" => Some(Role::Admin),
            "member" => Some(Role::Member),
            "viewer" => Some(Role::Viewer),
            _ => None,
        }
    }
}

/// Permissions that can be assigned to roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    OrganizationsRead,
    OrganizationsWrite,
    OrganizationsDelete,
    KeysRead,
    KeysWrite,
    KeysDelete,
    KeysRevoke,
    ProvidersRead,
    ProvidersWrite,
    ProvidersDelete,
    RoutingRead,
    RoutingWrite,
    RoutingDelete,
    QuotasRead,
    QuotasWrite,
    QuotasDelete,
    UsageRead,
    UsageReadAll,
    UsersRead,
    UsersWrite,
    UsersDelete,
    UsersInvite,
    WebhooksRead,
    WebhooksWrite,
    WebhooksDelete,
    SettingsRead,
    SettingsWrite,
    AuditRead,
    BillingRead,
    BillingWrite,
    Superadmin,
}

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::OrganizationsRead => "organizations:read",
            Permission::OrganizationsWrite => "organizations:write",
            Permission::OrganizationsDelete => "organizations:delete",
            Permission::KeysRead => "keys:read",
            Permission::KeysWrite => "keys:write",
            Permission::KeysDelete => "keys:delete",
            Permission::KeysRevoke => "keys:revoke",
            Permission::ProvidersRead => "providers:read",
            Permission::ProvidersWrite => "providers:write",
            Permission::ProvidersDelete => "providers:delete",
            Permission::RoutingRead => "routing:read",
            Permission::RoutingWrite => "routing:write",
            Permission::RoutingDelete => "routing:delete",
            Permission::QuotasRead => "quotas:read",
            Permission::QuotasWrite => "quotas:write",
            Permission::QuotasDelete => "quotas:delete",
            Permission::UsageRead => "usage:read",
            Permission::UsageReadAll => "usage:read:all",
            Permission::UsersRead => "users:read",
            Permission::UsersWrite => "users:write",
            Permission::UsersDelete => "users:delete",
            Permission::UsersInvite => "users:invite",
            Permission::WebhooksRead => "webhooks:read",
            Permission::WebhooksWrite => "webhooks:write",
            Permission::WebhooksDelete => "webhooks:delete",
            Permission::SettingsRead => "settings:read",
            Permission::SettingsWrite => "settings:write",
            Permission::AuditRead => "audit:read",
            Permission::BillingRead => "billing:read",
            Permission::BillingWrite => "billing:write",
            Permission::Superadmin => "superadmin",
        }
    }
}

/// Check if a role has a given permission.
pub fn check_permission(role: Role, permission: Permission) -> bool {
    match role {
        Role::Owner => true,
        Role::Admin => !matches!(permission, Permission::OrganizationsDelete | Permission::Superadmin),
        Role::Member => matches!(
            permission,
            Permission::KeysRead
                | Permission::KeysWrite
                | Permission::KeysRevoke
                | Permission::ProvidersRead
                | Permission::RoutingRead
                | Permission::QuotasRead
                | Permission::UsageRead
                | Permission::UsersRead
                | Permission::SettingsRead
                | Permission::WebhooksRead
        ),
        Role::Viewer => matches!(
            permission,
            Permission::OrganizationsRead
                | Permission::KeysRead
                | Permission::ProvidersRead
                | Permission::RoutingRead
                | Permission::QuotasRead
                | Permission::UsageRead
                | Permission::UsersRead
                | Permission::SettingsRead
                | Permission::WebhooksRead
                | Permission::AuditRead
                | Permission::BillingRead
        ),
    }
}

/// Return all permissions for a role.
pub fn permissions_for_role(role: Role) -> Vec<Permission> {
    use Permission::*;
    let all = vec![
        OrganizationsRead,
        OrganizationsWrite,
        OrganizationsDelete,
        KeysRead,
        KeysWrite,
        KeysDelete,
        KeysRevoke,
        ProvidersRead,
        ProvidersWrite,
        ProvidersDelete,
        RoutingRead,
        RoutingWrite,
        RoutingDelete,
        QuotasRead,
        QuotasWrite,
        QuotasDelete,
        UsageRead,
        UsageReadAll,
        UsersRead,
        UsersWrite,
        UsersDelete,
        UsersInvite,
        WebhooksRead,
        WebhooksWrite,
        WebhooksDelete,
        SettingsRead,
        SettingsWrite,
        AuditRead,
        BillingRead,
        BillingWrite,
        Superadmin,
    ];
    all.into_iter()
        .filter(|p| check_permission(role, *p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_owner_has_all_permissions() {
        let perms = permissions_for_role(Role::Owner);
        assert_eq!(perms.len(), 31);
    }

    #[test]
    fn test_admin_cannot_delete_org() {
        assert!(!check_permission(Role::Admin, Permission::OrganizationsDelete));
        assert!(check_permission(Role::Admin, Permission::KeysDelete));
    }

    #[test]
    fn test_member_read_only_limited() {
        assert!(check_permission(Role::Member, Permission::KeysRead));
        assert!(check_permission(Role::Member, Permission::KeysWrite));
        assert!(!check_permission(Role::Member, Permission::KeysDelete));
        assert!(!check_permission(Role::Member, Permission::UsersDelete));
    }

    #[test]
    fn test_viewer_read_only() {
        assert!(check_permission(Role::Viewer, Permission::UsageRead));
        assert!(!check_permission(Role::Viewer, Permission::KeysWrite));
        assert!(!check_permission(Role::Viewer, Permission::SettingsWrite));
    }
}
