# rustok-secrets — Crate API

Runtime secret-reference contracts and the resolver registry used by capability and module integrations.

## Public surface

- `SecretRef` / `SecretRefStatus` — persisted references and their resolution status.
- `SecretResolver` trait and `SecretResolverRegistry` (+ builder) — process-owned resolution.
- `SecretAccessPolicy`, `SecretError`, `SecretResolverRegistryBuilder`.
- Cloud resolvers: `AwsSecretsManagerResolver`, `AzureKeyVaultResolver`, `GcpSecretManagerResolver`.
- Remote resolvers: `KubernetesSecretResolver`, `VaultResolver`, `VaultAuth`.
- Re-exported secrecy primitives: `ExposeSecret`, `SecretString`.

## Invariants

- The crate stores no credentials; callers persist only `SecretRef { resolver, key }` values.
- Resolver endpoints and workload identities belong to the process-owned registry, so a tenant cannot redirect secret resolution to an arbitrary endpoint.
- Secret values never appear in logs, errors, or Debug output; only reference identities and lengths are observable.
