# rustok-media-transport / CRATE_API

## Public surface

### Public modules
- `client`: Client-side gRPC adapter `GrpcMediaProvider`.
- `public_image_client`: Narrow presentation client `GrpcMediaPublicImageProvider` and configuration.
- `server`: gRPC server adapter `MediaGrpcService` with operation authorization.
- `proto`: Generated protobuf service definitions (`media_service_client`, `media_service_server`).

### Primary Public Types & Contracts
- `GrpcMediaProvider`: Transport adapter implementing `MediaAssetReadPort`, `MediaPublicImageReadPort`, and `MediaAssetWritePort`.
- `GrpcMediaPublicImageProvider`: Narrow presentation adapter implementing `MediaPublicImageReadPort`.
- `GrpcMediaPublicImageConnectionConfig`: Connection configuration for HTTPS, timeouts, and public origins.
- `MediaGrpcService`: Server-side gRPC service wrapping canonical media providers.
- `MediaGrpcOperation`: Explicit authorization variants for the declared metadata/reference/upload/delete/translation/reconciliation operations, including the distinct `GetPublicImageAsset` grant.
- `TrustedMediaAuthority`: Server-installed authority struct carrying tenant, actor, claims/roles and explicit allowed operations.

## Platform Capability Bindings
- **gRPC adapters**: Supplies client and server transport mechanics for the declared Media operations.
- **Zero-Binary Framing**: gRPC frames contain metadata and descriptors only; image binaries remain on HTTP/S3 presigned channels.
- **Connection policy**: The narrow `GrpcMediaPublicImageConnectionConfig` requires HTTPS unless explicit loopback HTTP is allowed. Generic providers receive an already constructed channel. The provider listener must install authenticated authority separately.

## Domain Collaborations
- **`rustok-media`**: Sovereign owner of media entities, storage engines, image variant processing, and ports.
- **Consumers**: `rustok-profiles` can select the descriptor client through host composition. Other consumers use their declared Media port; this adapter does not switch them to gRPC.

## Minimum Contract Set

### Input DTOs/Commands
- `GrpcMediaPublicImageConnectionConfig`: endpoint, connect timeout, TLS domain, public origin and explicit insecure-loopback permission.
- Protobuf RPC payloads for asset inspection and descriptor retrieval.

### Domain Invariants
- **Metadata Framing Only**: Media binary bodies never traverse the gRPC channel.
- **PortContext Propagation**: Typed call context is serialized in the request; deadlines are also enforced through gRPC timeouts. Server-installed authority replaces untrusted tenant/principal claims.
- **Typed Error Roundtripping**: Serialized `PortError` structures are preserved and mapped back accurately across process boundaries.
- **Trusted authority**: `MediaGrpcService` rejects requests without authenticated `TrustedMediaAuthority` and the exact operation grant. The public-image provider also requires explicit attachment.

### Events / Outbox Side Effects
- Transport adapter itself emits no domain events; side effects are executed by the underlying `rustok-media` service.

### Errors / Failure Codes
- `GrpcMediaPublicImageConnectionError`: Endpoint/public-origin/timeout/TLS-domain validation and connection failures.
- Owner `PortError` details are preserved; unstructured network failures use the adapter mapping.

## Deployment scope

The adapter package and its conformance sources do not establish a standalone Media
service or all-consumer remote readiness. The current host selector changes Profiles'
public-image descriptor provider only. See [deployment limits](README.md#known-limitations--pending-implementation).
