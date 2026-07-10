# rustok-ai-media

## Purpose

`rustok-ai-media` owns media-specific AI descriptors and pure validation for
the `image_asset` vertical. It is a consumer support adapter, not the image
provider runtime or the media transport implementation.

## Responsibilities

- Define the stable task and tool identity for image generation.
- Register media-owned descriptor metadata for `rustok-ai` composition.
- Normalize and validate requested image dimensions.

## Interactions

`rustok-ai` owns provider routing and direct execution. `rustok-media` owns
`MediaAssetReadPort`; this crate records the consumer contract and its
degraded behavior without duplicating either owner.

## Entry points

- `register_media_ai_vertical_handlers`
- `normalize_image_size`

## Documentation

- [Module documentation](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform documentation map](../../docs/index.md)
