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
bounded dot-separated lowercase segments. Target routes are root-relative and
control-character free.

### Semantic records

- `NotificationSourceEventRef`
- `NotificationTargetRef`
- `NotificationSemanticDescriptor`
- `NotificationTemplateData`
- `NotificationPriority`

Descriptors are recipient-neutral. They contain semantic template keys and a
bounded string map, not rendered HTML, contact data, complete source payloads, or
arbitrary JSON.

### Audience and open authorization

- `NotificationAudienceCandidate`
- `NotificationAudiencePage`
- `NotificationOpenAuthorization`

An audience page contains at most 256 unique recipient UUIDs and an optional
bounded cursor. Target-open authorization returns either an authorized safe
root-relative route or an indistinguishable unavailable state.

### Provider boundary (`server` feature)

- `NotificationSourceProvider`
- `NotificationSourceRegistry`
- `NotificationSourceRegistryEntry`
- `register_notification_source_provider`
- `notification_source_registry_from_extensions`
- `ensure_notification_source_registry`
- `NotificationProviderError`

Provider registration is optional for producer modules and stored in
`ModuleRuntimeExtensions`. Duplicate source slugs fail closed. Provider errors
carry stable retryability and do not expose source-private error messages.

## Ownership rules

The API crate owns contracts only. `rustok-notifications` owns inbox,
preferences, fan-out, grouping, digests, retention, and delivery attempts.
Producer modules own subscriptions, source visibility, semantic event journals,
and target authorization. Email/push/SMS modules own channel transport.
