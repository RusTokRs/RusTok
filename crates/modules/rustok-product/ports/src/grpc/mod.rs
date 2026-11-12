pub mod auth;
#[cfg(feature = "grpc-client")]
pub mod client;
#[cfg(feature = "grpc-client")]
pub mod connection;
#[cfg(any(feature = "grpc-client", feature = "grpc-server"))]
pub mod payloads;
#[cfg(feature = "grpc-server")]
pub mod server;

pub mod proto {
    tonic::include_proto!("rustok.product");
}

pub use auth::{ProductCatalogGrpcAuthenticationError, ProductCatalogGrpcBearerToken};
#[cfg(feature = "grpc-client")]
pub use client::{GrpcProductCatalogCommandProvider, GrpcProductCatalogReadProvider};
#[cfg(feature = "grpc-client")]
pub use connection::{
    GrpcProductCatalogConnectionConfig, GrpcProductCatalogReadConnectionConfig,
    GrpcProductCatalogReadConnectionError, ValidatedGrpcProductCatalogReadConnection,
};
#[cfg(feature = "grpc-server")]
pub use server::{
    ProductCatalogCommandGrpcService, ProductCatalogGrpcBearerAuthenticator,
    ProductCatalogGrpcBearerInterceptor, ProductCatalogGrpcOperation, ProductCatalogGrpcService,
    TrustedProductCatalogAuthority,
};
pub use tonic::transport::ClientTlsConfig;
