# rustok-ai-media documentation

This support adapter owns the `image_asset` descriptor and image-size
validation. It does not own media storage, provider routing, or an admin UI.

`rustok-ai` consumes its registered descriptor; `rustok-media` provides the
`MediaAssetReadPort` contract. The active integration work is recorded in the
[implementation plan](./implementation-plan.md).
