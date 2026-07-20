# Module build dispatcher

`ModuleBuildDeliveryConsumer` defines the durable delivery boundary between an
external broker adapter and the owner-owned module build queue.
`IggyModuleBuildDeliverySource` uses the `rustok-module-build-dispatcher`
consumer group on the dedicated `module-build` topic. It holds a persistent
Iggy cursor across receive and acknowledgement, so `process_and_acknowledge`
commits an offset only after remote worker delivery and owner result persistence
succeed. Before it exposes a delivery, the adapter verifies every envelope
identity, event type/schema version, and queued-event payload, then requires
the payload tenant to equal the envelope tenant. A processing or acknowledgement failure leaves the offset uncommitted
for redelivery.

The platform's broker provisioning must create the `module-build` topic before
this dispatcher starts. There is no server-local polling or Cargo fallback.

## Deployment configuration

The `rustok-module-build-dispatcher` binary requires these deployment-owned
variables:

- `RUSTOK_MODULE_BUILD_DISPATCHER_DATABASE_URL`;
- `RUSTOK_MODULE_BUILD_DISPATCHER_WORKER_ENDPOINT` (`https://` only);
- `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_ADDRESSES`,
  `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_USERNAME`,
  `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_PASSWORD`, and
  `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_TLS_ENABLED=true`;
- client mTLS material under `RUSTOK_MODULE_BUILD_CLIENT_CERT_PEM`,
  `RUSTOK_MODULE_BUILD_CLIENT_KEY_PEM`, `RUSTOK_MODULE_BUILD_SERVER_CA_PEM`,
  and `RUSTOK_MODULE_BUILD_SERVER_DOMAIN`.

`RUSTOK_MODULE_BUILD_DISPATCHER_IDLE_POLL_DELAY_MS` is optional and bounded to
one minute. The external broker TLS setting is mandatory and cannot be disabled;
the dispatcher carries broker credentials and has no plaintext transport mode.
The binary validates worker readiness before it begins consuming.
Processing, worker, owner-persistence, or acknowledgement errors leave the
current broker offset uncommitted for redelivery. The dispatcher then exits
instead of retaining a pending delivery in memory; its deployment supervisor
must restart it with bounded backoff so the persistent consumer cursor can
redeliver that exact offset.
