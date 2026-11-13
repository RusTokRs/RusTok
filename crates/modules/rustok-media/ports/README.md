# rustok-media-ports

Canonical Fluid Backend Architecture (FBA) ports, DTOs, errors, and optional gRPC transport adapters for the RusTok Media module.

## Roles

- Transport-neutral owner ports: `MediaAssetReadPort`, `MediaAssetWritePort`, `MediaPublicImageReadPort`.
- Pure DTOs, descriptors, and admission contracts with zero database or ORM dependencies.
- Optional gRPC client (`grpc-client`) and server (`grpc-server`) transport adapters backed by Protobuf wire contracts.

