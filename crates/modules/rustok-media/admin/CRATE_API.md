# rustok-media-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for media library administration, file uploads, and asset details.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, `rustok-ui-i18n`, and `rustok-graphql`.

## Domain Collaborations
- Collaborates with `rustok-media` and `rustok-storage` for asset metadata, storage runtimes, and file transformations.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub use model::{MediaListItem, MediaListPayload}`
- `pub use transport::{MediaTransportError, fetch_media_library, upload_media}`
- `pub use ui::leptos::MediaAdmin`

## Minimum Contract Set

### Input DTOs/Commands
- Media upload request parameters (multipart file, MIME type, folder/collection tags).
- Media library query filters and pagination limits (`MediaListPayload`).

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Native server functions consume host-provided `HostRuntimeContext` and `StorageRuntime`.
- Fallback preservation: GraphQL fallback for queries and REST for uploads.

### Events / Outbox Side Effects
- Uploads and deletions trigger media outbox lifecycle events via `rustok-media`.

### Errors / Failure Codes
- Network, validation, and storage errors are typed through `MediaTransportError`.
