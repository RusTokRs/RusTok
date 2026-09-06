# rustok-media-transport

## Purpose

`rustok-media-transport` maps the Media-owned read, public-image presentation,
and write ports onto tonic gRPC for whole-module extraction. The canonical
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

An isolated Media process serves `MediaGrpcService` around its canonical
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

See [transport documentation](docs/README.md).
