# Page Builder media asset provider and srcset-safe static path

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-page-builder` (Page Builder), `rustok-media` (media storage and API)
- Extends: [Multilingual content contract](./2026-03-28-multilingual-content-contract.md)
- Supersedes: None
- Superseded by: None

## Context

The pages/page-builder functional audit (2026-10-09) recorded gap G-6: there is no
media library or file upload in the builder. The editor `AssetSection` exposes manual
`id` + `URL` fields, `AssetProviderDefinition` in the Fly registry is a declaration
that nothing registers or consumes, and there is no integration with `rustok-media`.
Authors cannot upload, browse a gallery, or pick an existing asset. On the publish
side the static policy forbids `srcset` (and therefore usable `picture`/`source`
pairs) outright, so responsive images cannot be authored even by hand.

The supporting pieces already exist. The Fly asset document model carries
`provider`/`provider_asset_id` and stamps `data-fly-asset-provider` patches; the SSR
asset surface already accepts `srcset` as an asset source attribute; `rustok-media`
owns upload, listing and public image URLs (`/api/media/public/images/...`); and the
media admin package already transports a paginated library query and multipart
uploads with the repository-standard UI transport selection. Wave 2 of the audit
roadmap therefore asks for an `AssetProvider` over `rustok-media` (upload, gallery,
selection) plus a srcset-safe path for responsive images.

## Decision

1. **`rustok.media` provider binding.** The Page Builder admin package owns the
   editor-side binding of the media module as an asset provider. It registers a real
   `AssetProviderDefinition { id: "rustok.media", supported_kinds: [...] }` into the
   canvas `RegistrySet`, and the Assets panel enumerates
   `RegistrySet::asset_providers` to decide which provider panels to render. A
   registered provider without a bound runtime shows no upload UI: the declaration is
   consumed, never decorative.
2. **Host-supplied port.** The Page Builder admin package defines an
   `AssetProviderPort` (`library_page`, `upload`) with canonical DTOs. The editor
   stays transport-free (the FFA property); the pages host composition binds an
   implementation over the `rustok-media-admin` transport (GraphQL `media` list and
   `POST /api/media` multipart upload). Other hosts may bind the same port later.
3. **Selection flow.** Choosing a media item builds a provider-tagged asset value
   (`provider: "rustok.media"`, `providerAssetId`, `src` = `public_url`, plus name/
   mime/width/height) and enters the existing `AssetCommand::Upsert` /
   apply-to-selected mechanics, so `data-fly-asset-provider` references keep working
   unchanged. Upload inserts the new item and selects it through the same path.
4. **srcset-safe static path.** The default static publish policy drops `srcset`
   from `forbidden_attributes` and validates it instead: every candidate URL must
   pass the same resource-image rule as `src` (relative or `https:`), `data:` URIs
   stay banned inside `srcset` (their base64 payload collides with the candidate
   comma grammar), descriptors are restricted to `<N>w` / `<N>x`, candidate count is
   bounded, and duplicate descriptors are rejected. `sizes` gains a strict grammar
   (media-condition groups plus one length per entry, bounded entries) instead of
   passing through unvalidated. An operator who re-adds `srcset` to the configured
   `forbidden_attributes` restores the blanket ban — explicit configuration outranks
   the default, matching the existing `url(` precedent.

## Sources of truth and ownership

- `rustok-media` remains the only owner of media storage, upload semantics and
  public image URLs. This decision adds no media endpoint and no media table.
- `rustok-page-builder` owns the static publish policy (including the srcset/sizes
  grammar) and the Fly asset document contract remains owned by `fly`.
- `rustok-page-builder/admin` owns the `AssetProviderPort` shape, the provider
  registry binding and the Assets panel. The pages host owns its port implementation
  over media transport.

## Invariants

- Every `srcset` candidate URL is validated exactly like an image `src`, minus
  `data:` payloads; a `srcset` value with one bad candidate fails the whole publish.
- `sizes` accepts only bounded media-condition/length entries; anything else is
  rejected fail-closed with a `landing_*` diagnostic path.
- Provider-tagged assets preserve `provider` and `providerAssetId` through every
  document command; the media item id never becomes document-internal authority —
  the published `src` remains a verifiable URL.
- The Assets panel renders a provider only when both the registry declaration and a
  bound `AssetProviderPort` runtime exist.
- The editor package performs no raw media transport; hosts supply the port.

## Non-goals

- Cropping, rendition authoring or derivative generation in the builder (the media
  module owns image recipes; exposing them is a later slice).
- Automatic `srcset` generation from renditions; authors compose candidates.
- `data:`-URI candidates inside `srcset`, `image-set()`, or CSS-based responsiveness.
- Binding the provider in the forum host composition or the Next admin.
- Changes to `rustok-media` endpoints, storage or lifecycle events.

## Data, transaction, and concurrency boundary

No new storage. Provider metadata lives in the page document (asset catalog +
`data-fly-asset-*` attributes) and is covered by the existing body revision history.
Media items stay owned by `rustok-media` rows; deleting a media item does not rewrite
documents (the published URL simply stops resolving — same model as any external
asset source). The static publish policy stays code-defined; its `policy_hash`
remains tamper-evidence for sanitized envelopes, not persisted configuration.

## Context dimensions

- **Tenant**: library and upload calls carry the authenticated token and tenant slug
  exactly like the media admin surface.
- **Channel**: assets are channel-agnostic document state.
- **Locale**: panel labels only; asset metadata is locale-free.

## Events and projections

None. This slice emits no contract events; media module lifecycle events are
unchanged. No search or cache projection covers document assets beyond the published
artifact itself.

## Failure semantics

- Port failures (library or upload) surface as panel error state and never mutate the
  document; an upload failure keeps the chosen file for retry.
- Static publish validation fails closed: one invalid `srcset`/`sizes` value rejects
  the document with a precise diagnostic path, like every other policy rule.
- **Policy-hash compatibility**: changing the default policy changes the sanitized
  envelope hash. Exact rebuild of sources published under an earlier policy fails
  closed with `retained sanitized source hash drifted`; the remediation is a fresh
  reviewed publish, which re-binds the rebuild source under the current policy.
  Serving existing published artifacts is unaffected: compiled artifact identity
  (`source_hash`/`build_hash`/registry/render-policy hashes) is policy-independent,
  and stored artifact integrity revalidation does not consult the policy hash. The
  wave-1 exact-rebuild invariant is deliberately left fail-closed rather than
  relaxed in this slice.

## Migration and cutover

No migrations and no backfill contract (no schema change). The policy change takes
effect at the next publish. Pre-change published pages keep serving; their explicit
rebuild requires one re-publish to re-bind (see Failure semantics).

## Alternatives considered

- **Raw media transport inside the editor UI** — rejected: the editor package is
  deliberately transport-free behind the FFA facade; hosts own persistence and
  transport.
- **Allowing `srcset` unvalidated** — rejected: that would let arbitrary candidate
  URLs bypass the single-URL `src` rule, the exact incoherence the policy's `url(`
  commentary rejects in the opposite direction.
- **Parsing `data:` candidates out of `srcset`** — rejected: base64 payloads contain
  commas; a grammar-tolerant parser would become the security boundary. Image `src`
  keeps supporting `data:`; `srcset` does not.
- **Relaxing the exact-rebuild drift check to compare canonical project bytes** —
  deferred: it would silently change the wave-1 rebuild invariant. If operational
  pressure appears, it needs its own ADR with the exactness definition re-derived.
- **Extending `PageBuilderCapabilityRequest` with an Assets capability** — rejected:
  the FFA envelope is a pinned transport contract; a narrow optional port avoids
  churning every facade implementation.

## Verification

- Unit tests: srcset/sizes grammar accept/reject vectors, operator blanket-ban
  override, provider definition registration, provider-tagged asset mapping.
- Existing SSR asset browser tests and `verify-fly-ssr-assets` keep passing
  unchanged (`srcset` remains an allowed asset source attribute).
- Source-marker guards over the sanitization boundary
  (`verify-page-builder-static-publish-resource-limits`,
  `verify-page-builder-publish-runtime-review`,
  `verify-page-builder-static-sanitization-execution`) keep passing; the sanitization
  hash payload is unchanged.
- Scoped `cargo` checks are not runnable in the authoring environment (no toolchain);
  the compile/test evidence for this slice is CI-owned and reported as such.

## Consequences

- Authors upload, browse and pick media inside the builder, and can publish
  responsive `srcset`/`sizes` images that survive the static policy.
- The Fly `asset_providers` registry becomes a live contract instead of a dormant
  declaration.
- Policy hash changes across this upgrade; operators must re-publish before an
  explicit rebuild of older pages (documented in the Page Builder README Known
  Limitations).
- Cropping/renditions and automatic `srcset` generation remain future work.
