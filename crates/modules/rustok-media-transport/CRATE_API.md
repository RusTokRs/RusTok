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
- `MediaGrpcOperation`: Typed operations (`GetPublicImageAsset`).
- `TrustedMediaAuthority`: Service authority authorization trait.

## Platform Capability Bindings
- **Transport Extraction**: Encapsulates gRPC communications for extracted microservice deployments of `rustok-media`.
- **Zero-Binary Framing**: gRPC frames contain metadata and descriptors only; image binaries remain on HTTP/S3 presigned channels.
- **Tls & Security**: Enforces HTTPS/TLS by default, allowing plaintext loopback only for local testing.

## Domain Collaborations
- **`rustok-media`**: Sovereign owner of media entities, storage engines, image variant processing, and ports.
- **Consumers (`rustok-profiles`, `rustok-blog`, etc.)**: Call media ports remotely without depending on SeaORM entities or storage engines.

## Minimum Contract Set

### Input DTOs/Commands
- `GrpcMediaPublicImageConnectionConfig`: `endpoint`, `connect_timeout`, `tls_domain`, `public_origin`.
- Protobuf RPC payloads for asset inspection and descriptor retrieval.

### Domain Invariants
- **Metadata Framing Only**: Media binary bodies never traverse the gRPC channel.
- **PortContext Propagation**: Caller deadlines and correlation metadata are propagated into gRPC request headers.
- **Typed Error Roundtripping**: Serialized `PortError` structures are preserved and mapped back accurately across process boundaries.
- **Strict TLS**: Remote endpoints require valid TLS certificates unless explicitly marked loopback.

### Events / Outbox Side Effects
- Transport adapter itself emits no domain events; side effects are executed by the underlying `rustok-media` service.

### Errors / Failure Codes
- `GrpcMediaPublicImageConnectionError`: Invalid endpoint, missing TLS configuration, or timeout.
- Mapped `PortError` failure codes:
  - `media.not_found`
  - `media.validation`
  - `media.unavailable`
  - `media.access_denied`
