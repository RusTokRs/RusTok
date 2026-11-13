pub mod auth;
pub mod connection;

#[cfg(feature = "grpc-client")]
pub mod client;

#[cfg(feature = "grpc-client")]
pub mod public_image_client;

#[cfg(feature = "grpc-server")]
pub mod server;

pub mod proto {
    tonic::include_proto!("rustok.media");
}

pub use auth::{
    MediaGrpcAuthenticationError, MediaGrpcBearerToken, MediaGrpcOperation, TrustedMediaAuthority,
};
#[cfg(feature = "grpc-server")]
pub use auth::{MediaGrpcBearerAuthenticator, MediaGrpcBearerInterceptor};
#[cfg(feature = "grpc-client")]
pub use client::GrpcMediaProvider;
pub use connection::{
    GrpcMediaConnectionConfig, GrpcMediaPublicImageConnectionConfig,
    GrpcMediaPublicImageConnectionError, ValidatedGrpcMediaPublicImageConnection,
};
#[cfg(feature = "grpc-client")]
pub use public_image_client::GrpcMediaPublicImageProvider;
#[cfg(feature = "grpc-server")]
pub use server::MediaGrpcService;
pub use tonic::transport::ClientTlsConfig;

