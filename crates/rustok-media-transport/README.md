# rustok-media-transport

## Purpose

`rustok-media-transport` maps the Media-owned read and write ports onto tonic
gRPC for whole-module extraction. The canonical DTOs, policies, and typed
errors remain owned by `rustok-media`.

## Responsibilities

- provide consumer and provider gRPC adapters for every Media port operation;
- propagate `PortContext` deadlines into gRPC timeouts;
- preserve serialized `PortError` details across the remote boundary;
- keep binary media bodies on Media-owned streaming REST or presigned object
  storage transports.

## Interactions

Consumers depend on `GrpcMediaProvider` through `MediaAssetReadPort` and
`MediaAssetWritePort`. An isolated Media process serves `MediaGrpcService`
around its canonical Media provider implementation.

## Entry points

- `GrpcMediaProvider`
- `MediaGrpcService`
- generated `proto::media_service_client` and `proto::media_service_server`

See [transport documentation](docs/README.md).
