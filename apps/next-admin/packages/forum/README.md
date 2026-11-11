# @rustok/forum-admin

Next.js admin UI package for community forum management, category hierarchies, topic moderation, and reply restructuring.

## Responsibilities

- **Category Hierarchy Management**: Full administrative tree workspace for forum categories with reordering, nesting, topic policy toggles, and archival/restoration (`CategoryTreeAdmin`, `CategoryEditorDialog`).
- **Topic & Reply Composition**: Rich-text authoring interfaces for editorial forum topics and discussion replies with draft protection and validation (`ForumTopicEditor`, `ForumReplyEditor`).
- **Topic Lifecycle Moderation**: Pinned status toggles, locking, closing, reopening, deletion, and restoration actions.
- **Advanced Discussion Restructuring**:
  - **Topic Merge**: Merge replies and metadata from a source topic into a target topic (`ForumTopicMerge`).
  - **Branch Fork**: Extract a reply subtree into an independent new topic (`ForumTopicFork`).
  - **Range Move**: Transfer a contiguous range of replies from one topic to another (`ForumTopicReplyRange`).
  - **Topic Split**: Split an existing thread into multiple topic branches (`ForumTopicSplit`).
  - **Slug Rename**: Safely rename topic routing slugs with redirect preservation (`ForumTopicSlugRename`).
- **Fluid Frontend Architecture (FFA)**: Domain validation and command builders isolated in `src/core/` with zero UI framework dependencies.
