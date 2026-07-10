# rustok-secrets

`rustok-secrets` owns runtime secret-reference contracts and the resolver
registry used by capability and module integrations.

It stores no credentials. Callers persist `SecretRef { resolver, key }`, while
the process composes trusted resolver aliases and workload identities. Secret
values remain redacted through `secrecy::SecretString`, are cached for at most
60 seconds, and can be invalidated after profile changes or rotation.

Built-in resolvers cover environment variables and mounted files. Vault,
Kubernetes, AWS Secrets Manager, GCP Secret Manager and Azure Key Vault attach
through the same `SecretResolver` contract from distribution-owned integration
crates; tenant-controlled data never supplies resolver endpoints.
