//! Shared secret-minimizing publication adapters for isolated build workers.

mod credentials;
mod publication;
mod signing;

pub use credentials::{
    CommandRegistryCredentialBroker, RegistryCredentialBroker, RegistryCredentialError,
    RegistryCredentialLease, validate_fixed_program,
};
pub use publication::{
    SignedOciArtifactPublicationError, SignedOciArtifactPublicationReceipt,
    publish_signed_oci_artifact,
};
pub use signing::{CosignArtifactSigner, CosignAttestationPredicate, CosignSigningError};
