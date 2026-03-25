use async_trait::async_trait;
use rustok_core::permissions::{Action, Permission, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod migrations;
pub mod services;

pub use services::PricingService;

pub struct PricingModule;

#[async_trait]
impl RusToKModule for PricingModule {
    fn slug(&self) -> &'static str {
        "pricing"
    }

    fn name(&self) -> &'static str {
        "Pricing"
    }

    fn description(&self) -> &'static str {
        "Variant pricing, price lists, regions, and discount calculations"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["product"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PRODUCTS_READ,
            Permission::PRODUCTS_UPDATE,
            Permission::new(Resource::Discounts, Action::Create),
            Permission::new(Resource::Discounts, Action::Read),
            Permission::new(Resource::Discounts, Action::Update),
            Permission::new(Resource::Discounts, Action::Delete),
            Permission::new(Resource::Discounts, Action::List),
            Permission::new(Resource::Discounts, Action::Manage),
        ]
    }
}

impl MigrationSource for PricingModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
