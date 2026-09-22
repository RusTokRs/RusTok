# rustok-notifications-api / CRATE_API

## Public contract

### Keys

- `NotificationSourceSlug`
- `NotificationTypeKey`
- `NotificationTemplateKey`
- `NotificationTargetKind`
- `NotificationAudienceCursor`
- `NotificationTargetRoute`

All keys validate during construction and deserialization. Source slugs are
bounded lowercase module identifiers. Semantic type/template/target keys may use
bounded dot-separated lowercase segments. Target routes are validated internal
root-relative paths. A single query component is allowed only as bounded
lowercase `key=value` pairs with safe ASCII values; absolute URLs, fragments,
percent encoding, traversal, whitespace, backslashes, and malformed queries fail
closed.

### Semantic records

- `NotificationSourceEventRef`
- `NotificationTargetRef`
- `NotificationSemanticDescriptor`
- `NotificationTemplateData`
- `NotificationPriority`

Descriptors are recipient-neutral. They contain semantic template keys and a
bounded string map, not rendered HTML, contact data, complete source payloads, or
arbitrary JSON. Source-event identity includes tenant, event UUID, source/type,
and a positive owner revision. Invariant-bearing fields remain private and are
validated by constructors and deserialization.

### Audience and open authorization

- `NotificationAudienceCandidate`
- `NotificationAudiencePage`
- `NotificationOpenAuthorization`

An audience page contains at most 256 unique recipient UUIDs and an optional
bounded cursor. Its internal collection is private so Rust callers cannot bypass
the constructor. Target-open authorization returns either an authorized safe
internal route or an indistinguishable unavailable state.

### Provider boundary (`server` feature)

- `NotificationSourceProvider`
- `NotificationSourceProviderFactory`
- `NotificationSourceRegistry`
- `NotificationSourceFactoryRegistry`
- `NotificationSourceRegistryEntry`
- `register_notification_source_provider`
- `register_notification_source_provider_factory`
- `materialize_notification_source_registry`
- `notification_source_registry_from_extensions`
- `notification_source_factory_registry_from_extensions`
- `ensure_notification_source_registry`
- `ensure_notification_source_factory_registry`
- `NotificationProviderError`
- `NotificationSourceRegistryError`

Producer modules register factories during module composition, before
DB-backed host services exist. The executable host later materializes those
factories with `rustok_api::HostRuntimeContext`. The API never exposes
`DatabaseConnection` or source persistence types. Duplicate source/factory
slugs, factory/provider slug mismatches, and factory build failures fail closed.
Provider errors carry stable retryability and do not expose source-private error
messages.

Direct provider registration remains available for tests and already-built
neutral providers. Factory materialization is the production path for sources
that require host capabilities.

## Ownership rules

The API crate owns contracts only. `rustok-notifications` owns inbox,
preferences, fan-out, grouping, digests, retention, and delivery attempts.
Producer modules own subscriptions, source visibility, semantic event journals,
and target authorization. Email/push/SMS modules own channel transport.
