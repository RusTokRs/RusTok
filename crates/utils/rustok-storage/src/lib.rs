//! Runtime construction and canonical object-key policy for RusToK.
//!
//! Domain owners use [`object_store::ObjectStore`] directly. This crate does
//! not wrap object CRUD or own any domain object's lifecycle.

mod key;
mod runtime;

pub use key::{DigestObjectKey, KeyError, ObjectKey, ObjectScope, ObjectZone};
pub use object_store;
pub use runtime::{
    LocalStorageConfig, S3StorageConfig, StorageConfig, StorageDriver, StorageKind, StorageRuntime,
};
