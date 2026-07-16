# Module build dispatcher

`ModuleBuildDeliveryConsumer` defines the durable delivery boundary between an
external broker adapter and the owner-owned module build queue.
`IggyModuleBuildDeliverySource` uses the `rustok-module-build-dispatcher`
consumer group on the dedicated `module-build` topic. It holds a persistent
Iggy cursor across receive and acknowledgement, so `process_and_acknowledge`
commits an offset only after remote worker delivery and owner result persistence
succeed. A processing or acknowledgement failure leaves the offset uncommitted
for redelivery.

The platform's broker provisioning must create the `module-build` topic before
this dispatcher starts. There is no server-local polling or Cargo fallback.

## Deployment configuration

The `rustok-module-build-dispatcher` binary requires these deployment-owned
variables:

- `RUSTOK_MODULE_BUILD_DISPATCHER_DATABASE_URL`;
- `RUSTOK_MODULE_BUILD_DISPATCHER_WORKER_ENDPOINT` (`https://` only);
- `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_ADDRESSES`,
  `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_USERNAME`, and
  `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_PASSWORD`;
- client mTLS material under `RUSTOK_MODULE_BUILD_CLIENT_CERT_PEM`,
  `RUSTOK_MODULE_BUILD_CLIENT_KEY_PEM`, `RUSTOK_MODULE_BUILD_SERVER_CA_PEM`,
  and `RUSTOK_MODULE_BUILD_SERVER_DOMAIN`.

`RUSTOK_MODULE_BUILD_DISPATCHER_IDLE_POLL_DELAY_MS` is optional and bounded to
one minute. The binary validates worker readiness before it begins consuming.
Processing, worker, owner-persistence, or acknowledgement errors leave the
current broker offset uncommitted for redelivery.
