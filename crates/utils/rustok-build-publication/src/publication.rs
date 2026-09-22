use std::time::{Duration, Instant};

use rustok_modules::{
    ArtifactAdmissionLimits, OciArtifactPublicationBundle, OciArtifactPublicationError,
    OciArtifactPublicationTarget, OciArtifactPublisher, OciArtifactReference,
    OciDistributionArtifactPublisher,
};
use thiserror::Error;

use crate::{
    CosignArtifactSigner, CosignAttestationPredicate, CosignSigningError, RegistryCredentialBroker,
    RegistryCredentialError,
};

/// Digest-pinned OCI identities created by the shared publication boundary.
/// The derived OCI write tags used during publication never escape this type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedOciArtifactPublicationReceipt {
    pub artifact: OciArtifactReference,
    pub signature_manifest: OciArtifactReference,
}

/// Terminal outcome of one bounded OCI publication and Cosign signing attempt.
/// Callers map these categories to their own durable protocol without exposing
/// credentials, registry output, or subprocess details.
#[derive(Debug, Error)]
pub enum SignedOciArtifactPublicationError {
    #[error("OCI publication input was rejected")]
    Rejected,
    #[error("OCI publication exceeded its execution deadline")]
    TimedOut,
    #[error("OCI publication infrastructure is unavailable: {0}")]
    Unavailable(String),
}

/// Publishes the descriptor-configured package, signs its digest-pinned
/// artifact with Cosign, creates the required SLSA and CycloneDX Cosign
/// attestations. This is the single credential/signing path shared by isolated
/// build workers and owner-receipted publication workers. The independent
/// verifier records the digest of its actual verified evidence output, so this
/// boundary does not assume a legacy Cosign attestation tag or one registry
/// referrer layout.
pub async fn publish_signed_oci_artifact(
    credentials: &dyn RegistryCredentialBroker,
    signer: &CosignArtifactSigner,
    target: &OciArtifactPublicationTarget,
    bundle: OciArtifactPublicationBundle,
    limits: ArtifactAdmissionLimits,
    execution_timeout: Duration,
    credential_lease_safety_margin: Duration,
) -> Result<SignedOciArtifactPublicationReceipt, SignedOciArtifactPublicationError> {
    if execution_timeout.is_zero() {
        return Err(SignedOciArtifactPublicationError::TimedOut);
    }
    target
        .validate()
        .map_err(|_| SignedOciArtifactPublicationError::Rejected)?;
    if !credentials.is_ready() || !signer.is_ready() {
        return Err(SignedOciArtifactPublicationError::Unavailable(
            "credential broker or Cosign deployment identity is not ready".to_string(),
        ));
    }
    let deadline = Instant::now() + execution_timeout;
    let acquire_timeout = remaining(deadline)?;
    let credential_lease = tokio::time::timeout(
        acquire_timeout,
        credentials.acquire(target, acquire_timeout + credential_lease_safety_margin),
    )
    .await
    .map_err(|_| SignedOciArtifactPublicationError::TimedOut)?
    .map_err(map_credential_error)?;
    credential_lease
        .ensure_valid()
        .map_err(|_| SignedOciArtifactPublicationError::Rejected)?;
    let publisher = OciDistributionArtifactPublisher::strict(credential_lease.registry_auth())
        .map_err(|error| SignedOciArtifactPublicationError::Unavailable(error.to_string()))?;
    let sbom = bundle.sbom.clone();
    let provenance = bundle.provenance.clone();
    let publication_timeout = remaining(deadline)?;
    let artifact = tokio::time::timeout(
        publication_timeout,
        publisher.publish(target.clone(), bundle, limits),
    )
    .await
    .map_err(|_| SignedOciArtifactPublicationError::TimedOut)?
    .map_err(map_oci_publication_error)?;
    let signing_timeout = remaining(deadline)?;
    signer
        .sign(&artifact, &credential_lease, signing_timeout)
        .await
        .map_err(map_signing_error)?;
    let provenance_timeout = remaining(deadline)?;
    signer
        .attest(
            &artifact,
            &credential_lease,
            CosignAttestationPredicate::SlsaProvenance,
            &provenance.bytes,
            provenance_timeout,
        )
        .await
        .map_err(map_signing_error)?;
    let sbom_timeout = remaining(deadline)?;
    signer
        .attest(
            &artifact,
            &credential_lease,
            CosignAttestationPredicate::CycloneDx,
            &sbom.bytes,
            sbom_timeout,
        )
        .await
        .map_err(map_signing_error)?;
    let signature_timeout = remaining(deadline)?;
    let signature_manifest = tokio::time::timeout(
        signature_timeout,
        publisher.resolve_cosign_signature(target, &artifact),
    )
    .await
    .map_err(|_| SignedOciArtifactPublicationError::TimedOut)?
    .map_err(map_oci_publication_error)?;
    Ok(SignedOciArtifactPublicationReceipt {
        artifact,
        signature_manifest,
    })
}

fn remaining(deadline: Instant) -> Result<Duration, SignedOciArtifactPublicationError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or(SignedOciArtifactPublicationError::TimedOut)
}

fn map_credential_error(error: RegistryCredentialError) -> SignedOciArtifactPublicationError {
    match error {
        RegistryCredentialError::Rejected => SignedOciArtifactPublicationError::Rejected,
        RegistryCredentialError::TimedOut => SignedOciArtifactPublicationError::TimedOut,
        RegistryCredentialError::Unavailable(error) => {
            SignedOciArtifactPublicationError::Unavailable(error)
        }
    }
}

fn map_signing_error(error: CosignSigningError) -> SignedOciArtifactPublicationError {
    match error {
        CosignSigningError::Rejected => SignedOciArtifactPublicationError::Rejected,
        CosignSigningError::TimedOut => SignedOciArtifactPublicationError::TimedOut,
        CosignSigningError::Unavailable(error) => {
            SignedOciArtifactPublicationError::Unavailable(error)
        }
        CosignSigningError::Credential(error) => map_credential_error(error),
    }
}

fn map_oci_publication_error(
    error: OciArtifactPublicationError,
) -> SignedOciArtifactPublicationError {
    match error {
        OciArtifactPublicationError::InvalidTarget(_)
        | OciArtifactPublicationError::InvalidBundle(_)
        | OciArtifactPublicationError::ManifestDigestMismatch { .. } => {
            SignedOciArtifactPublicationError::Rejected
        }
        OciArtifactPublicationError::Registry(error) => {
            SignedOciArtifactPublicationError::Unavailable(error)
        }
    }
}
