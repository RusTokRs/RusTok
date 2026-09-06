# rustok-secrets

`rustok-secrets` owns runtime secret-reference contracts and the resolver
registry used by capability and module integrations.

It stores no credentials. Callers persist `SecretRef { resolver, key }`, while
the process composes trusted resolver aliases and workload identities. Secret
values remain redacted through `secrecy::SecretString`, are cached for at most
60 seconds, and can be invalidated after profile changes or rotation.
`SecretString` is re-exported for trusted host-consumer ports so downstream
owners do not need to unwrap secret material into an ordinary `String`.

Built-in resolvers cover environment variables, mounted files, Vault (token or
Kubernetes auth), Kubernetes Secrets, AWS Secrets Manager via the default AWS
credential chain, GCP Secret Manager via ADC, and Azure Key Vault via workload,
managed, or developer identity selection. Resolver endpoints, namespaces,
projects, vault names, identities, and access policies are constructor-only
server configuration.

Every resolver registration has an exact-key, prefix, or tenant-prefix policy.
`resolve_for_tenant(...)` checks that policy before reading or returning a cached
value, so a persisted tenant reference cannot become an arbitrary environment or
cross-tenant secret read.
