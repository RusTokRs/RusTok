# rustok-media-transport

## Purpose

`rustok-media-transport` maps the Media-owned read, durable-reference admission,
public-image presentation, and write ports onto tonic gRPC for whole-module
extraction. The canonical DTOs, policies, lifecycle decisions, URL selection,
and typed errors remain owned by `rustok-media`.

## Responsibilities

- provide consumer and provider gRPC adapters for Media metadata/control operations;
- expose `MediaReferenceAdmissionPort` remotely so durable cross-module references
  receive the same bounded owner decision in embedded and extracted deployments;
- expose `MediaPublicImageReadPort` remotely so consumers receive the same
  owner-selected direct or capability descriptor as embedded deployments;
- provide `GrpcMediaPublicImageConnectionConfig` for validated HTTPS connection,
  bounded startup timeout, optional TLS domain override, and public-origin routing;
- allow plaintext only for an explicitly enabled loopback endpoint/origin;
- rebase only root-relative Media descriptors when an extracted Media HTTP origin
  differs from the consumer host;
- require an explicit public-image provider attachment and a separate
  `GetPublicImageAsset` trusted-authority grant on the server;
- require a separate `AdmitReferences` trusted-authority grant for durable-reference
  admission; generic asset-read authority does not imply this capability;
- propagate `PortContext` deadlines into gRPC timeouts;
- preserve serialized `PortError` details across the remote boundary;
- keep binary media bodies, HTTP cache headers, conditional GET behavior, and
  object-store access on Media-owned HTTP or presigned storage transports.

## Interactions

Consumers may use the broad `GrpcMediaProvider` through `MediaAssetReadPort`,
`MediaReferenceAdmissionPort`, `MediaPublicImageReadPort`, and
`MediaAssetWritePort`. Public profile presentation uses the narrower
`GrpcMediaPublicImageProvider`, which exposes only `MediaPublicImageReadPort` and
may resolve root-relative owner descriptors against a validated deployment-owned
Media public origin.

An isolated Media process serves `MediaGrpcService` around its canonical
metadata/reference-admission/write provider and explicitly attaches the owner
public-image provider with `with_public_image_provider(...)`.

The reference-admission RPC returns only bounded owner decisions
`{ media_id, tenant_id, referenceable }`. Media lifecycle strings, deletion
timestamps, storage state, quarantine implementation details, and object keys do
not cross the consumer boundary. Missing and non-referenceable assets fail closed.

The public-image RPC returns only `MediaPublicImageAsset`: the canonical asset
metadata needed for consumer relation validation and the descriptor selected by
Media. The descriptor may point to the Media HTTP capability endpoint; the RPC never
reads or returns the image body.

## Entry points

- `GrpcMediaProvider`
- `GrpcMediaPublicImageConnectionConfig`
- `GrpcMediaPublicImageProvider`
- `MediaGrpcService`
- `MediaGrpcOperation::AdmitReferences`
- `MediaGrpcOperation::GetPublicImageAsset`
- generated `proto::media_service_client` and `proto::media_service_server`

See [transport documentation](docs/README.md).
