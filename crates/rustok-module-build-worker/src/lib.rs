//! Isolated process boundary for module-build execution.

pub mod artifact;
pub mod materializer;
pub mod policy;
pub mod runner;
pub mod source;

pub use artifact::{
    BuildEvidenceError, BuildEvidenceInspector, ComponentArtifactError, ComponentArtifactInspector,
    PublicationBundleCollector, PublicationBundleError, WitContractError, WitContractInspector,
};
pub use materializer::{DependencyMaterializationError, OciScopedDependencyMaterializer};
pub use policy::{
    CargoMetadataError, CargoMetadataInspector, SourcePolicyError, SourcePolicyPreflight,
};
pub use runner::CommandBuildWorker;
pub use source::{SourceMaterializationError, SourceMaterializer};
