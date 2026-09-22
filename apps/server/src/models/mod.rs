//! Database models for the server application

pub mod _entities;
pub mod auth_invite_consumptions;
pub mod flex_attached_localized_values;
pub mod flex_entries;
pub mod flex_entry_localized_values;
pub mod flex_schema_translations;
pub mod flex_schemas;
pub mod mcp_audit_logs;
pub mod mcp_clients;
pub mod mcp_policies;
pub mod mcp_scaffold_drafts;
pub mod mcp_tokens;
pub mod oauth_app_translations;
pub mod oauth_apps;
pub mod oauth_authorization_codes;
pub mod oauth_consents;
pub mod oauth_tokens;
pub mod order_field_definitions;
pub mod platform_settings;
pub mod platform_state;
pub mod product_field_definitions;
pub mod sessions;
pub mod tenants;
pub mod topic_field_definitions;
pub mod user_field_definitions;
pub mod users;

pub use auth_invite_consumptions::Entity as AuthInviteConsumptions;
pub use flex_attached_localized_values::Entity as FlexAttachedLocalizedValues;
pub use flex_entry_localized_values::Entity as FlexEntryLocalizedValues;
pub use oauth_app_translations::Entity as OAuthAppTranslations;
