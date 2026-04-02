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
mod storefront_channel;
mod storefront_shipping;

#[cfg(test)]
extern crate self as rustok_commerce;

#[cfg(test)]
mod state_machine_proptest;

pub use dto::*;
pub use error::{CommerceError, CommerceResult};
pub use graphql::{CommerceMutation, CommerceQuery};
pub use services::{
    CartService, CatalogService, CheckoutError, CheckoutResult, CheckoutService, CustomerService,
    FulfillmentService, InventoryService, OrderService, PaymentService, PricingService,
    RegionService, StoreContextError, StoreContextResult, StoreContextService,
};
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
        &[
            "cart",
            "customer",
            "product",
            "region",
            "pricing",
            "inventory",
            "order",
            "payment",
            "fulfillment",
        ]
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
            // Regions
            Permission::new(Resource::Regions, Action::Create),
            Permission::new(Resource::Regions, Action::Read),
            Permission::new(Resource::Regions, Action::Update),
            Permission::new(Resource::Regions, Action::Delete),
            Permission::new(Resource::Regions, Action::List),
            Permission::new(Resource::Regions, Action::Manage),
            // Payments
            Permission::new(Resource::Payments, Action::Create),
            Permission::new(Resource::Payments, Action::Read),
            Permission::new(Resource::Payments, Action::Update),
            Permission::new(Resource::Payments, Action::Delete),
            Permission::new(Resource::Payments, Action::List),
            Permission::new(Resource::Payments, Action::Manage),
            // Fulfillments
            Permission::new(Resource::Fulfillments, Action::Create),
            Permission::new(Resource::Fulfillments, Action::Read),
            Permission::new(Resource::Fulfillments, Action::Update),
            Permission::new(Resource::Fulfillments, Action::Delete),
            Permission::new(Resource::Fulfillments, Action::List),
            Permission::new(Resource::Fulfillments, Action::Manage),
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
