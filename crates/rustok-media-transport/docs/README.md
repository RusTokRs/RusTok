# Media gRPC transport

The transport is deliberately narrower than the Media HTTP/object interfaces.
It carries asset metadata, owner-selected public image descriptors,
translations, upload-session control, deletion commands, and reconciliation
commands. Upload/download bytes never enter a JSON or protobuf envelope.

## Contract ownership

`rustok-media` owns `MediaAssetReadPort`, `MediaPublicImageReadPort`,
`MediaAssetWritePort`, `PortContext` usage, DTO validation, lifecycle behavior,
public URL policy, and typed errors. This crate owns tonic framing and validated
consumer connection policy. Owner errors are serialized into gRPC status details so
the client reconstructs the exact `PortError`; unstructured network failures use a
small deterministic gRPC-to-port fallback mapping.

`GetPublicImageAsset` returns `MediaPublicImageAsset`: canonical asset metadata
plus the descriptor selected by Media. Storage-relative image references may
therefore become a Media capability URL before crossing the consumer boundary.
The image body, checksum validation, immutable cache headers, conditional GET,
and object-store read remain on the Media-owned HTTP endpoint.

## Provider configuration

`MediaGrpcService::new(provider)` preserves the existing metadata/write adapter
and does not infer access to Media database or storage state. A deployment must
explicitly call `with_public_image_provider(...)` to enable public descriptor
selection. Calls without that attachment return typed unavailable semantics.

The trusted server interceptor must separately allow
`MediaGrpcOperation::GetPublicImageAsset`; a generic asset-read grant does not
implicitly authorize public URL selection.

## Consumer connection policy

`GrpcMediaPublicImageConnectionConfig` builds the public-image-only remote adapter.
The configuration requires an absolute origin-style gRPC endpoint without URL
credentials, query, fragment, or path. External connections require HTTPS and use
webpki roots. Plain HTTP is accepted only when the deployment explicitly enables
insecure loopback and the endpoint host is `localhost` or a loopback IP.

Connection establishment has a bounded 1–30,000 ms timeout. A TLS domain override
may be supplied for service discovery names that differ from certificate identity.
Custom root stores and client certificates are not part of this source slice;
production mutual TLS remains a deployment gate.

An optional Media public origin may be configured for extracted deployments. It is
validated with the same HTTPS/explicit-loopback policy. The adapter rebases only
root-relative owner descriptors, such as
`/api/media/public/images/{asset}/{checksum}`. Absolute direct/CDN URLs remain
unchanged. Profiles does not see the endpoint, TLS settings, or rebasing policy.

## Server deployment wiring

The RusToK server supports these environment variables for profile image
presentation:

- `RUSTOK_PROFILE_MEDIA_PROVIDER=embedded|grpc`;
- `RUSTOK_PROFILE_MEDIA_GRPC_ENDPOINT`;
- `RUSTOK_PROFILE_MEDIA_PUBLIC_ORIGIN`;
- `RUSTOK_PROFILE_MEDIA_GRPC_TLS_DOMAIN`;
- `RUSTOK_PROFILE_MEDIA_GRPC_CONNECT_TIMEOUT_MS`;
- `RUSTOK_PROFILE_MEDIA_GRPC_ALLOW_INSECURE_LOOPBACK`.

`embedded` is the default. Remote-only variables are rejected in embedded mode so a
misspelled provider selector cannot silently fall back. `grpc` requires an endpoint,
validates the transport configuration, establishes the connection before app runtime
bootstrap, and pre-seeds the transport-neutral `ProfileMediaPublicImageProvider`.
Invalid configuration or an unavailable remote service stops startup instead of
silently selecting embedded Media.

Endpoint and public-origin values are not logged. The server logs only the selected
provider class and non-sensitive configuration booleans.

## Deployment boundary

Embedded deployments use the Media providers directly. Extracted deployments wrap
the metadata/write provider in `MediaGrpcService`, attach the public-image provider
explicitly, and expose the Media capability HTTP route through deployment-owned
ingress.

Production provider listeners still require host-owned mutual TLS and an
authentication/authorization interceptor that inserts `TrustedMediaAuthority` with an
explicit allow-list of `MediaGrpcOperation` values into tonic request extensions. The
server rejects requests without that trusted authority or without an allow-listed
operation and replaces caller-supplied tenant/principal claims before invoking the
provider.

The returned capability URL must be reachable through the same tenant routing model.
Health/readiness, client certificate distribution, public ingress, Local/S3 byte
runtime evidence, cache behavior, rollback, and observability remain deployment
gates.

## Verification

`cargo test -p rustok-media-transport` contains owner-port conformance and connection
configuration source scenarios. Retained source covers public capability descriptor
selection, deadline propagation, typed deleted state, explicit trusted-operation
authorization, HTTPS/loopback policy, bounded connection timeout, public-origin
rebasing, and the rule that binary image bodies never cross gRPC.

These commands were not run while publishing this source slice.
