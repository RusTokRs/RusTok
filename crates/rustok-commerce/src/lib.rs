use async_trait::async_trait;
use rustok_core::permissions::{Action, Permission, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod services;
pub mod state_machine;

#[cfg(test)]
mod state_machine_proptest;

pub use dto::*;
pub use error::{CommerceError, CommerceResult};
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
        "Commerce"
    }

    fn description(&self) -> &'static str {
        "Products, Orders, Cart, Checkout"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            // Products
            Permission::new(Resource::Products, Action::Create),
            Permission::new(Resource::Products, Action::Read),
            Permission::new(Resource::Products, Action::Update),
            Permission::new(Resource::Products, Action::Delete),
            Permission::new(Resource::Products, Action::List),
            // Orders
            Permission::new(Resource::Orders, Action::Create),
            Permission::new(Resource::Orders, Action::Read),
            Permission::new(Resource::Orders, Action::Update),
            Permission::new(Resource::Orders, Action::List),
            // Customers
            Permission::new(Resource::Customers, Action::Read),
            Permission::new(Resource::Customers, Action::Update),
            Permission::new(Resource::Customers, Action::List),
            // Inventory
            Permission::new(Resource::Inventory, Action::Read),
            Permission::new(Resource::Inventory, Action::Update),
            Permission::new(Resource::Inventory, Action::List),
            // Discounts
            Permission::new(Resource::Discounts, Action::Create),
            Permission::new(Resource::Discounts, Action::Read),
            Permission::new(Resource::Discounts, Action::Update),
            Permission::new(Resource::Discounts, Action::Delete),
            Permission::new(Resource::Discounts, Action::List),
        ]
    }
}

impl MigrationSource for CommerceModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[cfg(test)]
mod contract_tests;
