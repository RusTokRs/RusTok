//! Database models for the server application

pub mod _entities;
pub mod build;
pub mod oauth_apps;
pub mod platform_settings;
pub mod product_field_definitions;
pub mod oauth_authorization_codes;
pub mod oauth_consents;
pub mod oauth_tokens;
pub mod release;
pub mod sessions;
pub mod tenant_modules;
pub mod tenants;
pub mod user_field_definitions;
pub mod users;

pub use build::Entity as Build;
pub use release::Entity as Release;
