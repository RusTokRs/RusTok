//! Shared secret-minimizing publication adapters for isolated build workers.

mod credentials;
mod signing;

pub use credentials::{
    CommandRegistryCredentialBroker, RegistryCredentialBroker, RegistryCredentialError,
    RegistryCredentialLease,
};
pub use signing::{CosignArtifactSigner, CosignSigningError};
