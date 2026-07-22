# Media gRPC transport

The transport is deliberately narrower than the Media HTTP/object interfaces.
It carries asset metadata, descriptors, translations, upload-session control,
deletion commands, and reconciliation commands. Upload/download bytes never
enter a JSON or protobuf envelope.

## Contract ownership

`rustok-media` owns `MediaAssetReadPort`, `MediaAssetWritePort`, `PortContext`
usage, DTO validation, lifecycle behavior, and typed errors. This crate only
owns tonic framing. Owner errors are serialized into gRPC status details so the
client reconstructs the exact `PortError`; unstructured network failures use a
small deterministic gRPC-to-port fallback mapping.

## Deployment

Embedded deployments use `MediaService` directly. Extracted deployments wrap
the same provider in `MediaGrpcService` and inject `GrpcMediaProvider` into
consumers. Production listeners must use host-owned mutual TLS and an
authentication/authorization interceptor that inserts `TrustedMediaAuthority`
with an explicit allow-list of `MediaGrpcOperation` values into tonic request
extensions. The server rejects requests without that trusted authority or
without an allow-listed operation, and replaces caller-supplied
tenant/principal claims before invoking the provider. Host-owned
health/readiness and observability configuration remain required.

## Verification

`cargo test -p rustok-media-transport` runs one owner-port conformance suite
against both an embedded provider and a real loopback tonic server/client.
