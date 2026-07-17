//! Isolated process boundary for module-build execution.

pub mod artifact;
pub mod credentials;
pub mod materializer;
pub mod policy;
pub mod runner;
pub mod signing;
pub mod source;

pub use artifact::{
    BuildEvidenceError, BuildEvidenceInspector, ComponentArtifactError, ComponentArtifactInspector,
    PublicationBundleCollector, PublicationBundleError, WitContractError, WitContractInspector,
};
pub use credentials::{
    CommandRegistryCredentialBroker, RegistryCredentialBroker, RegistryCredentialError,
    RegistryCredentialLease,
};
pub use materializer::{DependencyMaterializationError, OciScopedDependencyMaterializer};
pub use policy::{
    CargoMetadataError, CargoMetadataInspector, SourcePolicyError, SourcePolicyPreflight,
};
pub use runner::{OciJobBuildWorker, OciJobRuntime};
pub use signing::{CosignArtifactSigner, CosignSigningError};
pub use source::{SourceMaterializationError, SourceMaterializer};
