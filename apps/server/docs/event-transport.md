# Event delivery in `apps/server`

## Purpose

The server uses one global event-delivery profile for the whole process. It is
not a tenant setting: events and the outbox cross tenant boundaries, so a
tenant must never select its own broker or delivery semantics.

The profile is persisted in `event_delivery_settings` and takes effect only at
the next controlled server restart. This deliberately prevents a live swap of
the event transport while an outbox relay is processing records.

## Profiles

| Profile | Delivery path | Intended use |
| --- | --- | --- |
| `memory` | process-local memory bus | development, tests, and simple local blogs; no durable event record |
| `outbox_local` | transactional database outbox → process-local listeners | lightweight single-node production deployments, including stores |
| `outbox_iggy` | transactional database outbox → Iggy → local listeners | high-throughput or multi-process production deployments |

`outbox_local` is fully independent of Iggy. It is the production default for
a single server process and retains retries, backoff, DLQ handling, and
transactional event persistence.

There is no direct-Iggy profile and no relay fallback. If `outbox_iggy` cannot
initialize Iggy, startup fails with an actionable error instead of silently
delivering events locally.

## Operator workflow

1. Open the global Events and Outbox operator screen.
2. Select `memory`, `outbox_local`, or `outbox_iggy`.
3. If `outbox_iggy` is selected, the server validates its deployment Iggy
   configuration. When it is absent or invalid, the UI shows a configuration
   dialog and the API rejects the change.
4. Save the desired profile and restart the server through the deployment's
   normal controlled-restart mechanism.
5. Confirm the active profile and relay health on the same screen or through
   `eventsStatus`.

The screen never accepts Iggy passwords or endpoints. Those are deployment
secrets and remain in the server configuration or secret store.

## Deployment configuration

Bootstrap defaults and Iggy deployment details are under
`settings.rustok.events`:

```yaml
settings:
  rustok:
    events:
      delivery_profile: outbox_local
      relay_interval_ms: 1000
      iggy:
        mode: external # bundled or external
        external:
          addresses: ["iggy.example.net:8090"]
          protocol: tcp
          username: "iggy"
          password: "${IGGY_PASSWORD}"
```

`RUSTOK_EVENT_DELIVERY_PROFILE` may provide the bootstrap profile using one of
`memory`, `outbox_local`, or `outbox_iggy`. The persisted global setting has
priority when the server starts.

For `outbox_iggy`, `external` requires a TCP endpoint, a username, and a
password secret reference. The reference is persisted as resolver/key metadata;
the value is resolved through `rustok-secrets` only inside the server process.
`bundled` starts the module-installed native `iggy-server` on loopback with
durable data storage. Iggy upstream does not support Windows server hosts, so
Windows must use an external Iggy deployment. See the
[Iggy integration reference](../../../docs/references/iggy/README.md).

## Runtime path

1. A module writes its business state and outbox event in one transaction.
2. `rustok-outbox` claims and retries pending records.
3. The selected relay target receives the event.
4. Local module listeners receive an idempotent local fan-out after the primary
   target accepts the event.

Settings and runtime locations:

- bootstrap settings: `apps/server/src/common/settings.rs`
- global profile storage and validation:
  `apps/server/src/services/event_delivery_settings_service.rs`
- connector-owned settings and readiness:
  `crates/rustok-iggy-connector` with the server adapter in
  `apps/server/src/services/iggy_connector_settings_service.rs`
- transport assembly: `apps/server/src/services/event_transport_factory.rs`
- GraphQL control plane: `apps/server/src/graphql/settings/`

## Module-owned listeners

Modules publish their event handlers through
`RusToKModule::register_event_listeners(...)`. The host collects and registers
them into one shared dispatcher. Cron jobs and other long-running maintenance
workers are separate runtime paths.
