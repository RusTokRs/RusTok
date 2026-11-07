# rustok-media-transport

> **FBA microservice extraction** — typed gRPC transport adapter supporting the proposed whole-module extraction of `rustok-media`, with current scope bounded to implemented owner ports. Part of the Media domain FBA extraction; MUST NOT be collapsed into `rustok-media`. See `AGENTS.md` §5 and `docs/modules/registry.md`.

## Purpose

`rustok-media-transport` maps the Media-owned read, public-image presentation,
and write ports onto tonic gRPC as prerequisites for the proposed extraction. The canonical
DTOs, policies, URL selection, and typed errors remain owned by `rustok-media`.

## Responsibilities

- provide consumer and provider gRPC adapters for Media metadata/control operations;
- expose `MediaPublicImageReadPort` remotely so consumers receive the same
  owner-selected direct or capability descriptor as embedded deployments;
- provide `GrpcMediaPublicImageConnectionConfig` for validated HTTPS connection,
  bounded startup timeout, optional TLS domain override, and public-origin routing;
- allow plaintext only for an explicitly enabled loopback endpoint/origin;
- rebase only root-relative Media descriptors when an extracted Media HTTP origin
  differs from the consumer host;
- require an explicit public-image provider attachment and a separate
  `GetPublicImageAsset` trusted-authority grant on the server;
- propagate `PortContext` deadlines into gRPC timeouts;
- preserve serialized `PortError` details across the remote boundary;
- keep binary media bodies, HTTP cache headers, conditional GET behavior, and
  object-store access on Media-owned HTTP or presigned storage transports.

## Interactions

Consumers may use the broad `GrpcMediaProvider` through `MediaAssetReadPort`,
`MediaPublicImageReadPort`, and `MediaAssetWritePort`. Public profile presentation
uses the narrower `GrpcMediaPublicImageProvider`, which exposes only
`MediaPublicImageReadPort` and may resolve root-relative owner descriptors against a
validated deployment-owned Media public origin.

A deployment providing an isolated Media process must serve `MediaGrpcService` around its canonical
metadata/write provider and explicitly attaches the owner public-image provider with
`with_public_image_provider(...)`.

The public-image RPC returns only `MediaPublicImageAsset`: the canonical asset
metadata needed for consumer relation validation and the descriptor selected by
Media. The descriptor may point to the Media HTTP capability endpoint; the RPC never
reads or returns the image body.

## Entry points

- `GrpcMediaProvider`
- `GrpcMediaPublicImageConnectionConfig`
- `GrpcMediaPublicImageProvider`
- `MediaGrpcService`
- `MediaGrpcOperation::GetPublicImageAsset`
- generated `proto::media_service_client` and `proto::media_service_server`

## Documentation

- [CRATE_API.md](CRATE_API.md) — Public API contract, transport framing, capability bindings, and domain invariants.
- [Transport Documentation](docs/README.md)

## Known Limitations / Pending Implementation

This is an adapter library, not a standalone Media provider host. No
`rustok-media-service` package currently exists. The consumer host's
`RUSTOK_PROFILE_MEDIA_PROVIDER=grpc` selector is limited to Profiles public-image
presentation. A provider must separately install authenticated `TrustedMediaAuthority`
with exact operation grants and expose the Media-owned HTTP byte endpoint. TLS connection
configuration alone does not establish caller authority. See the
[deployment contract](docs/README.md#deployment-boundary) and
[Media owner plan](../rustok-media/docs/implementation-plan.md#ffafba-boundary).
