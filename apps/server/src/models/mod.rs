//! Database models for the server application

pub mod _entities;
pub mod auth_invite_consumptions;
pub mod auth_invites;
pub mod auth_email_change_requests;
pub mod flex_attached_localized_values;
pub mod flex_entries;
pub mod flex_entry_localized_values;
pub mod flex_schema_translations;
pub mod flex_schemas;
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
pub use auth_invites::Entity as AuthInvites;
pub use auth_email_change_requests::Entity as AuthEmailChangeRequests;
pub use flex_attached_localized_values::Entity as FlexAttachedLocalizedValues;
pub use flex_entry_localized_values::Entity as FlexEntryLocalizedValues;
pub use oauth_app_translations::Entity as OAuthAppTranslations;
