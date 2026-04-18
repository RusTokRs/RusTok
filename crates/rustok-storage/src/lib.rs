pub mod backend;
pub mod error;
pub mod local;
#[cfg(feature = "s3")]
pub mod s3;
pub mod service;

pub use backend::{StorageBackend, UploadedObject};
pub use error::{Result, StorageError};
pub use local::LocalStorageConfig;
#[cfg(feature = "s3")]
pub use s3::S3StorageConfig;
pub use service::{StorageConfig, StorageDriver, StorageService};
