//! Tonic gRPC framing for the Product-owned catalog read port.
//!
//! Canonical DTOs and policy remain in `rustok-product`. This crate supplies a
//! replaceable external adapter and does not own catalog storage or semantics.

pub mod auth;
pub mod client;
pub mod connection;
pub mod server;

pub mod proto {
    tonic::include_proto!("rustok.product");
}

pub use auth::{ProductCatalogGrpcAuthenticationError, ProductCatalogGrpcBearerToken};
pub use client::GrpcProductCatalogReadProvider;
pub use connection::{
    GrpcProductCatalogReadConnectionConfig, GrpcProductCatalogReadConnectionError,
    ValidatedGrpcProductCatalogReadConnection,
};
pub use server::{
    ProductCatalogGrpcBearerAuthenticator, ProductCatalogGrpcBearerInterceptor,
    ProductCatalogGrpcOperation, ProductCatalogGrpcService, TrustedProductCatalogAuthority,
};
pub use tonic::transport::ClientTlsConfig;
