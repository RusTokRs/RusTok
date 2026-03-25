use async_trait::async_trait;
use rustok_core::permissions::{Action, Permission, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod migrations;
mod search;
pub mod services;
pub mod state_machine;

#[cfg(test)]
mod state_machine_proptest;

pub use dto::*;
pub use error::{CommerceError, CommerceResult};
pub use graphql::{CommerceMutation, CommerceQuery};
pub use services::{CatalogService, InventoryService, PricingService};
pub use state_machine::{
    Cancelled, Confirmed, Delivered, Order, OrderError, Paid, Pending, Shipped,
};

pub struct CommerceModule;

#[async_trait]
impl RusToKModule for CommerceModule {
    fn slug(&self) -> &'static str {
        "commerce"
    }

    fn name(&self) -> &'static str {
        "Ecommerce"
    }

    fn description(&self) -> &'static str {
        "Ecommerce umbrella/root module for the commerce family, compatibility facade, and orchestration surface"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["product", "pricing", "inventory"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            // Products
            Permission::new(Resource::Products, Action::Create),
            Permission::new(Resource::Products, Action::Read),
            Permission::new(Resource::Products, Action::Update),
            Permission::new(Resource::Products, Action::Delete),
            Permission::new(Resource::Products, Action::List),
            Permission::new(Resource::Products, Action::Manage),
            // Orders
            Permission::new(Resource::Orders, Action::Create),
            Permission::new(Resource::Orders, Action::Read),
            Permission::new(Resource::Orders, Action::Update),
            Permission::new(Resource::Orders, Action::Delete),
            Permission::new(Resource::Orders, Action::List),
            Permission::new(Resource::Orders, Action::Manage),
            // Customers
            Permission::new(Resource::Customers, Action::Create),
            Permission::new(Resource::Customers, Action::Read),
            Permission::new(Resource::Customers, Action::Update),
            Permission::new(Resource::Customers, Action::Delete),
            Permission::new(Resource::Customers, Action::List),
            Permission::new(Resource::Customers, Action::Manage),
            // Inventory
            Permission::new(Resource::Inventory, Action::Create),
            Permission::new(Resource::Inventory, Action::Read),
            Permission::new(Resource::Inventory, Action::Update),
            Permission::new(Resource::Inventory, Action::Delete),
            Permission::new(Resource::Inventory, Action::List),
            Permission::new(Resource::Inventory, Action::Manage),
            // Discounts
            Permission::new(Resource::Discounts, Action::Create),
            Permission::new(Resource::Discounts, Action::Read),
            Permission::new(Resource::Discounts, Action::Update),
            Permission::new(Resource::Discounts, Action::Delete),
            Permission::new(Resource::Discounts, Action::List),
            Permission::new(Resource::Discounts, Action::Manage),
        ]
    }
}

impl MigrationSource for CommerceModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod contract_tests;
