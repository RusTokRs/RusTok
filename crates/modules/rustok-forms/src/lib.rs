//! Forms module: form submission intake, abuse controls and lead triage.

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod http;
pub mod migrations;
pub mod openapi;
pub mod services;

pub use dto::*;
pub use entities::FormSubmission;
pub use error::{FormsError, FormsResult};
pub use graphql::{FormsMutation, FormsQuery};
pub use services::{
    FORM_HONEYPOT_FIELD, FORM_SUBMIT_RATE_LIMIT, FORM_SUBMIT_RATE_WINDOW_MINUTES,
    FormsNotification, FormsService, MAX_FORM_PAYLOAD_BYTES, MAX_FORM_PAYLOAD_FIELDS,
    notification_from_shared,
};

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub struct FormsModule;

#[async_trait]
impl RusToKModule for FormsModule {
    fn slug(&self) -> &'static str {
        "forms"
    }
    fn name(&self) -> &'static str {
        "Forms"
    }
    fn description(&self) -> &'static str {
        "Form submission intake, abuse controls and lead triage inbox"
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn dependencies(&self) -> &[&'static str] {
        &["outbox", "email"]
    }
    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Forms, Action::Read),
            Permission::new(Resource::Forms, Action::List),
            Permission::new(Resource::Forms, Action::Update),
            Permission::new(Resource::Forms, Action::Manage),
        ]
    }
}

impl MigrationSource for FormsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
