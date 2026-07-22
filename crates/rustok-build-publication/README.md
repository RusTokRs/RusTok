# rustok-build-publication

## Purpose

`rustok-build-publication` owns the shared secret-minimizing credential and
signing adapters used by isolated RusToK build publishers.

## Responsibilities

- Invoke one digest-pinned deployment credential broker with a bounded
  current-only request/response contract.
- Keep short-lived registry credentials in memory and expose only scoped OCI
  authentication or a private temporary Cosign configuration.
- Invoke one digest-pinned Cosign executable with an approved KMS reference,
  cleared environment, closed standard streams, and a bounded deadline.
- Re-hash fixed broker and signer executables before every use.

## Non-responsibilities

- It does not own build commands, registry destinations, OCI artifact shapes,
  promotion policy, source materialization, databases, or long-lived secrets.
- It does not accept raw private keys, file-backed signing keys, caller-selected
  programs, plaintext credentials in requests, or version-suffixed fallback
  contracts.

## Interactions and entry points

- `CommandRegistryCredentialBroker` acquires one repository-scoped lease.
- `RegistryCredentialLease` supplies scoped OCI/Cosign authentication.
- `CosignArtifactSigner` signs one exact digest-pinned OCI subject.
- See [local documentation](docs/README.md).
