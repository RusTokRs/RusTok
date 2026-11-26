# RusTok Page Builder Admin

This package is the optional full-authoring surface for the Page Builder module.

## Current responsibilities

It owns:

- the full Fly presentation shell;
- an isolated iframe canvas with an instrumented, CSP-restricted document renderer;
- source/origin/protocol/instance/sequence validation for iframe messages;
- viewport, component geometry, pointer, hover and selection synchronization with `FlyUiStateMachine`;
- selected and hovered overlays in parent canvas coordinates;
- the `AdminCanvasController` that applies `fly-ui` intents to `FlyEditor`;
- the admin FFA facade contract over canonical Page Builder capability envelopes;
- editor-side diagnostics, dirty state, undo/redo, save lifecycle and revision-conflict handling.

## Consumer contract

The package does not own consumer document persistence, tenant/auth loading, GraphQL selection,
native server-function selection, domain widget data, or authoritative backend sanitization.

A consumer provides `PageBuilderAdminHostContext` with an `AdminCanvasController` and an
`Arc<dyn PageBuilderAdminFacade>`. The facade receives canonical capability requests and returns
canonical responses. The editor owns save-start, save-failure and acknowledgement state, including
acknowledgement against the exact project hash that was dispatched.

For Dioxus hosts, configure
`DioxusPageBuilderAdminHostContext::with_draft_recovery_scope` (or the
`draft_recovery_scope` prop) with a stable, opaque value derived from the
authenticated tenant and actor. Do not pass an email address or other direct
identifier. This enables browser-local recovery in `localStorage`: each tab
writes immutable per-save records under its own writer identity and records the
server base revision/document plus the unsaved document. Recovery selects only
the latest sequence from each writer, while retaining older record references
for safe cleanup if a tab was interrupted between writing and pruning. Before selecting active records, the reader validates the complete persisted supersession graph; any cycle is a fatal recovery error and leaves every record untouched, even when unrelated writers have valid records. The host's
`on_reload_requested` remains the authoritative
refresh action; after reload, records from all tabs are reconciled against the
latest server document using a conservative three-way merge. Object fields are
merged recursively and arrays of uniquely identified objects are merged by
stable `id`; simultaneous edits to the same value, ambiguous arrays and
incompatible reorderings stop in a read-only conflict state. The local copies
and conflicting paths remain visible until the author copies or explicitly
discards them. Records are retired only after a confirmed save. Each save uses a fresh immutable
storage key, so compare-before-delete cannot race a peer replacing the bytes of
a record that was already read.

Browser recovery is disabled when the host does not supply a scope. A single
record is limited to 2 MiB to stay within common browser storage quotas; quota,
serialization and storage-security errors are surfaced and the current project
JSON remains available for manual copying. The browser store is same-origin
storage, not encrypted storage. Cross-tab draft edits are not live collaboration:
they are merged on a reload/reconciliation boundary, while BroadcastChannel
notices remain advisory and server revision checks are authoritative.

When the Pages facade returns `PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN`, Dioxus
keeps the editor read-only and retains the exact submitted page, revision and payload.
The mounted retry is admitted only if the current draft still matches that request;
edits made during the original save are preserved and require an authoritative reload
instead of being submitted as a different retry. A generic retry failure or malformed
response does not clear the unknown-outcome fence. The durable local recovery record
also makes a full reload recoverable: if the
first request committed, the latest server document already contains the
submitted draft; if it did not, the local draft can be merged against the
unchanged server base. The server receipt remains the authority for whether a
write committed.

`frames[0].component` data into canonical `pages[].component`, keeps a synchronized frame snapshot
for the existing JSON fallback, performs optimistic `updated_at` revision checks, and persists
through the existing Pages transport facade.

## Browser boundary

The iframe is sandboxed with `allow-scripts` and without `allow-same-origin`. A `srcdoc` iframe has
an opaque `null` origin, so the parent validates both that origin and the exact `contentWindow`
identity before decoding the feature protocol. Browser listener, observer, pointer-capture and
postMessage lifecycle primitives live in `fly-leptos` and are compiled only for `wasm32`.
The admin package's direct DOM bindings are owned by its `browser-js` feature;
`csr` and `hydrate` enable that feature through `wasm-client`, while `ssr`
keeps the native rendering path independent from browser APIs.

The same feature decides whether the server-rendered editor emits the `fly-browser` adapter.
A consumer sets `default-features = false`, so a consumer feature that enables `ssr` must also
enable `browser-js`; without it the editor renders `<span data-fly-browser-adapter="disabled">`
and no control works. Under a host Content Security Policy with a nonce, the host provides
`rustok_ui_core::UiCspNonce` as render context: a server host from the request, a browser host
from the `nonce` property of a script of its own document. The editor writes it on the adapter
`<script>`, on the `<style>` and `<script>` of the canvas `srcdoc` document, and on the
stylesheet of the server preview document (`fly::document_html_with_style_nonce`). Both
documents inherit the host policy. The adapter workspace reaches the same nonce from the same
context: `AdapterWorkspacePanel` passes it to `fly_leptos::FlyEditorWorkspace`, whose canvas
renders the project stylesheet as an inline `<style>` that the policy would otherwise drop.

The default file-backed SSR editor draft store uses a persistent per-user state directory
(`XDG_STATE_HOME`, `LOCALAPPDATA`, or the platform home state path) and a 30-day
sliding editor retention. Set `RUSTOK_DRAFT_SESSION_DIR` to an explicitly mounted
persistent directory in containers; multi-replica deployments must use a shared
KV backend rather than relying on a node-local filesystem.

The file backend uses native OS file locks (`std::fs::File::try_lock`; `flock` on Unix, `LockFileEx` on Windows) and holds
the token lock descriptor for the full read-check-write operation. A stable
`_locks/.guard.lock` serializes token-lock opening with cleanup: waiters close a
contended descriptor before releasing the guard, and `prune` unlinks only idle
session and owner-cap lock files while holding it. The OS releases held locks when a process exits;
there is no age-based stale-lock breaking. Per-owner capacity checks also take a
stable owner-key lock held across the capacity scan and new-record write, while
candidate records are revalidated under their own token locks before eviction. All writers sharing a directory must use the same
protocol, so drain old-version writers before rolling out this change.
The underlying filesystem must propagate OS file locks; use the shared KV backend
when that guarantee is unavailable.

A consumer that restores SSR draft sessions provides `SsrDraftRenderBinding`, including the
exact token and generation that produced the rendered controller (or an explicit saved-page
render with no session). Pages supplies this binding. The browser uses the rendered generation,
not a potentially newer sessionStorage value. If a saved-page render finds a cached draft, or the
route and render disagree, editing stops and a same-origin recovery link preserves the token.
Out-of-order responses cannot rewrite the route, and a history-write failure stops editing and
offers recovery instead of reloading the wrong document. For this binding, the host checks the
pathname, `page_id`, draft token and connected root before sending an intent or applying its route
response; an old canvas cannot navigate the newly selected page. This does not make navigation
transactional: stopping can abort a request already committed by the server before its token is
received, and concurrent adapters for the same page can still overwrite the single per-page
sessionStorage pointer. Durable discovery of all in-flight draft sessions remains open. Hosts
without this binding retain their existing browser contract. The Node DOM-boundary test and Chromium host-bootstrap fixture pass;
they use stubbed endpoints and do not replace a running Pages-server integration test.

The current canvas supports rendering, geometry, hover, selection, overlays and persistence. It
also provides the block palette, drag-and-drop insertion, resize handles, keyboard editing,
traits/styles editors and asset management. The iframe forwards only admitted editor shortcuts;
ordinary text input, IME composition, AltGraph, repeated save/duplicate and native text editing
are not dispatched as authoring intents. The nearest contenteditable boundary controls text admission.
Storefront real-DOM editing remains outside this
authoring package.

The canvas is a preview surface, not a navigable document: the runtime cancels authored link
activation and form submission inside the iframe, so an authored `<a href>` or form cannot replace
the `srcdoc` document with a page the bridge no longer instruments. The server preview panel is
where a rendered page — including its links and forms — is inspected.

## Verification

```bash
node scripts/verify/verify-fly-admin-browser-runtime.mjs
node scripts/verify/verify-pages-ui-boundary.mjs
cargo check -p rustok-page-builder-admin --no-default-features --features hydrate --target wasm32-unknown-unknown
cargo check -p rustok-page-builder-admin --no-default-features --features ssr
cargo check -p rustok-page-builder-admin --no-default-features --features dioxus-web --target wasm32-unknown-unknown
```

Rust unit and browser interaction tests must also be run in a checkout with the repository Rust
and WASM toolchains installed.

## Known Limitations / Pending Implementation

Server preview (`src/editor/server_preview.rs`):

- **Active-page preview error handling is source-only.** Both the browser intent and the Pages
  preview route now call `AdminCanvasController::active_page_project`, which returns a Fly error
  instead of silently producing an empty project when the active page is absent or encoding
  fails. The change still requires crate compilation and regression coverage for both paths.
- **The preview cannot be requested in the default runtime.** "Refresh preview" acts through
  hydration. No intent or SSR form requests a preview, and the draft session holds none.
- **The preview is shown under the policy of the admin.** A `srcdoc` document inherits the
  policy of the page that embeds it, so a `style` attribute, a provider frame of an embed block
  and a font or media file from another origin are refused there although the published page
  shows them. The canvas document has the same limits.
- **Only the Fly document envelope receives the nonce.** A consumer whose preview port returns
  other markup gets it embedded as it arrived, and its inline styles are dropped under a nonce
  policy.

Pages of a document in the default runtime (`src/editor/ssr_inspector.rs`):

- **The page the editor is on is changed through the "Open page" form of the SSR inspector.**
  The page tabs of the workspace act through hydration and do nothing without it.
- **A caller without the edit capability cannot change the page.** The open page is kept in the
  draft session, which the host keeps for a caller that may edit, so the form is not rendered for
  anyone else. Such a caller sees the page the document opens on.
- **Undoing a created page does not return to the page the author came from.** The page is
  removed and the editor shows the last page that is left.

SSR draft session storage (`src/draft_session.rs`):

- **`prune` has one host caller, and it is the standalone admin.** Expired sessions are also
  dropped when they are read (the file store deletes the file under the token lock, the key-value
  backend deletes the key only while it still holds the generation that was read), and the
  per-owner cap is enforced when a session is created. Both are lazy: a session nobody reads again
  would never be collected, and the global cap (`maximum_entries`) would never apply. The
  standalone admin now runs `apps/admin/src/app/draft_retention.rs`
  (`DRAFT_SESSION_PRUNE_INTERVAL`, five minutes) over the store the intent endpoint resolves, so
  a store installed through `set_pages_browser_draft_store` is pruned too. A host that embeds the
  editor but not that binary still owes the same call.
- **Drafts of the pre-partition file layout are not migrated.** Files written directly into the
  storage root by earlier builds are no longer read; `prune` deletes them once they are older than
  one TTL.
- **A damaged record fails its session for up to one TTL.** Records carry a format version
  (`PersistentDraftSessionRecord::format`, now `2`) and both stores read them through
  `PersistentDraftSessionRecord::decode`, so the two causes are distinguishable:
  - A record of a **newer** format than this build understands parses, and is answered with
    `UnsupportedFormat` — a deployment that has to be finished (or rolled back), not damage. The
    record is not aged out and not deleted: it belongs to a build that is still writing the store.
  - A record that does not parse at all is damage: the file store reports `Serialization` for that
    token until the file has not been written for a whole TTL, then treats it as expired and
    deletes it; the key-value backend reports `Serialization` and leaves the record to the TTL the
    driver stored it with.
  Records written before the field existed have format `0` and are read as-is; the next commit
  rewrites them with the current format. An editor that holds the token of a damaged session
  still gets an error for every action until the TTL passes or it opens the page in a new tab.
- **Ownership and mode checks are Unix-only.** On other platforms the storage root is not checked
  for a foreign owner and files get no private mode; access control is whatever the operator set on
  the parent directory.
- **The file lock is advisory and breaks stale locks by age.** A crashed holder blocks its token for
  up to 30 seconds, and two waiters breaking the same stale lock can briefly both hold it (see
  `with_token_lock`). A stale lock that cannot be deleted fails the operation with an I/O error.
- **`DurableSsrDraftSessionStore` extends a session's lifetime on commit only.** Loading a draft
  through the durable store does not slide its expiry; a draft that is only viewed expires one TTL
  after its last edit.
- **An unusable storage root is reported with its path.** The error raised when the file store
  cannot be created names the configured directory, and the host returns that text to the editor
  that hit it.
- **A lookup that misses scans every tenant partition** of the file store before answering.
- **The per-owner cap groups sessions by `tenant_id:user_id`.** Two owners whose identifiers
  concatenate to the same string share one cap.

For the formal Rust API contract and guarantees, see [`CRATE_API.md`](CRATE_API.md).

## Snapshot-bound save acknowledgements

Before invoking any persistence transport, SSR Pages, Leptos and Dioxus use
`AdminCanvasController::begin_save_request` to bind the emitted save effect to
the live page, CAS revision, command sequence, project hash and exact encoded
project data. Stale or mismatched effects are rejected before I/O and cannot
start an attempt. Session reducer acknowledgements for inactive attempts do not
replace current error feedback.

A successful Page Builder publish response acknowledges exactly the hash captured when the
request began. Newer local edits remain dirty, while the persisted revision becomes the base for
the next save. A local edit during an in-flight request is **not** a remote revision conflict.
Duplicate save starts, acknowledgements without an active request, and acknowledgements for a
nonmatching submitted snapshot are rejected. Browser/host transports must still correlate the
actual request and response; the controller cannot recover that identity from an HTTP status.

In-flight save flags are transient and reset when an editor session is deserialized, because
network requests cannot be resumed from serialized state. The reactive admin runtime now starts
save tracking for both the facade and callback transport paths and requires a submitted hash for
publish requests; an implicit read of the later working-tree hash is not an acknowledgement.

All mutating browser intents must now carry the current persisted revision. Unversioned
read-only selections remain allowed. The trusted Leptos adapter callback stamps the revision
and project hash from its controller before invoking the same checked dispatch path; remote
browser clients must send their observed revision and cannot obtain a fresh one implicitly.

Both Leptos and Dioxus trusted in-process editor workspaces attach their current revision and
project hash to typed Fly intents before checked dispatch. The SSR page-form contract tests
simulate that same host binding; no HTTP input is silently upgraded to a current revision.

The browser intent dispatcher now applies selection hints, composed commands and response
construction as one transactional gesture. A failed insertion or later step rolls the controller
back instead of leaving a changed selection/clipboard or half-finished page operation. A snapshot
is taken for mutating intents and intents with selection hints; pure read-only hover/select without
hints retain the lightweight path. This adds a state snapshot to mutating browser dispatches;
profile the cost with large projects before certifying performance.

The controller now creates a `SaveAttempt` containing both a local revision sequence and a
fresh UUID request ID. Acknowledgement, failure and conflict require the exact active attempt;
the hash-only acknowledgement path has been removed. The active request ID is transient and
is **not** a server idempotency key. A restored controller cannot accept a response from an
attempt that existed before serialization. Known `REVISION_CONFLICT` and
`PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN` codes survive the Leptos server-function error boundary;
a stale-revision response retires only its matching attempt and fences the editor read-only.
Fly UI owns the effective capability fence, so host permission refreshes cannot unlock it.
Recovered merges are applied to a fresh/reloaded controller, not a conflicted revision.

The Leptos facade runtime and callback transport now carry the same `SaveAttempt`. The public
`PageBuilderCapabilityExecution` callback receives the request and its optional attempt together,
so external hosts can acknowledge or fail only the request they actually executed. Late
responses from an older identical retry cannot corrupt the active attempt.

The framework-neutral `EditorSessionState` carries `SaveAttempt` through save start,
acknowledgement and failure actions. There is no hash-only acknowledgement fallback: Dioxus and
external session adapters must return the exact attempt token.

Toolbar and browser previews use the controller's canonical active-page projection. A missing
page fails before an attempt is started. Before dispatching a browser preview (or a prepared
Dioxus effect), the controller checks the page identity, command sequence and exact active-page
payload, since navigating between pages does not change the project hash. Adopting a different
browser runtime context or scenario clears the previous preview and runtime publish-gate result.

The Dioxus admin Save toolbar and FlyEditorWorkspace consume reducer request effects and execute
the supplied consumer facade. Save completion is attempt-aware; browser previews retain their
scenario/context and lifecycle flags in the same session. The Page Builder Dioxus host now mounts
the isolated iframe canvas, forwards geometry/hover/focus/keyboard events, completes palette and
selected-layer drag/drop, commits bounded resize patches and participates in peer-write fencing.
Both framework adapters use the same Page Builder resize policy for pointer and keyboard
input: a 4px grid, 24px/20px minimum width/height and a 100,000px maximum on either axis.
The Leptos Rust runtime and JS adapter share a document nonce plus editor-root scope so their
own confirmed writes advance one baseline; other tabs or editor roots fence stale editors.
These BroadcastChannel notices are advisory and do not replace server revision CAS or receipts.
Mounted browser/CSP verification remains outstanding.

Dioxus host-context/prop equality now considers the full draft controller and every relevant
facade, contribution, runtime and workspace toggle. Previously only page ID and persisted
revision were compared, so local edits and changes in injected providers could be treated as
unchanged props and suppress a needed rerender. Regression coverage authored, not run.

The Leptos FlyEditorWorkspace's typed `on_intent` callback forwards Save/Preview and
announcement effects to `AdminEditorRuntime`. The runtime compares the command sequence and
submitted project hash before dispatch, captures the exact `SaveAttempt`, and retains the
browser-provided preview context for response correlation. Server preview attempts also have
a local UUID independent of their project hash, page index and runtime context. A superseded
success, failure or duplicate callback cannot consume a newer preview's state, even when its
inputs are identical. The Dioxus effect dispatcher distinguishes a prepared preview from a
browser-origin request and retires only the matching attempt on transport/provider failure.
Removing a host-supplied trait registry restores the builtin registry rather than retaining
the previous host's schemas. Missing Leptos facades retire only the matching preview attempt,
so the preview indicator cannot remain busy and stale dispatch failures cannot clear a newer one.

Browser intent admission also rejects a supplied document page ID that disagrees with the
controller's own page ID, including read-only intents. Leptos validates the completed workspace
result's page, revision, hash and command sequence before dispatching any facade effects. This
prevents a late/switching-document callback from executing effects against a different draft.

Working-draft Save is gated by edit permission, not by the runtime release/publish gate. The
gate remains a separate release-readiness evaluation and must not prevent persistence of a
working draft. The Pages host maps the Page Builder Save capability to its draft-update
authorization; actual release remains a distinct host operation.

Dioxus workspace effect handling now rejects a mismatched owning document, server revision,
project hash, or command sequence before calling a facade. Previously a mismatched effect hash
was silently turned into `None` and the request sequence was ignored. Concurrent preview requests
are blocked instead of replacing one another's request snapshot. All of these are source changes
awaiting the final user-led checks.

The Dioxus PageBuilderAdmin host now keys its child session by owning page ID and server
revision. A route/document switch or an authoritative revision replacement remounts the editor
instead of reusing `use_signal` state from the prior document. Same-revision local edits remain
inside the existing session; consumers mounting the inner WithController component directly
must provide their own stable instance key for owner changes. Source regression authored, not run.

Dioxus now applies the effective tenant/RBAC capability profile on session creation, narrowed
by the facade's provider status exactly as in the Leptos AdminCanvas, instead of always
starting with Fly's default full editing permissions. Undo/Redo also require history capability;
Save is disabled without edit permission, during an active/conflicted save, or with blocking
validation diagnostics. Dispatch remains the authoritative guard against forged UI events.

Dioxus editor capabilities are also updated reactively when a host prop or provider-limited
profile changes on an already mounted session. The live Fly controller is narrowed without
resetting draft content, using an untracked state read to avoid rerunning on every edit. If the
host withdraws its profile, the base controller policy (not an old restricted snapshot) is used
as the fallback. This is not an authorization substitute for the backend.

Direct Fly workspace preview requests now use the same consumer facade provider-health and
rollout `preview_enabled()` gate as the normal Leptos preview panel. Both the Leptos and
Dioxus dispatch paths reject disabled previews before calling the facade and clear pending
preview state rather than leaving an in-progress indicator stranded.

The shared `FlyEditorWorkspace` already accepts a `capabilities` prop to disable palette, style and authoring controls. Dioxus now forwards the controller's effective permissions rather than omitting that prop and allowing the workspace to default to full editing affordances.

Dioxus Page Builder now passes host selection, active page and undo/redo availability to the
controlled Fly workspace, whose intent-mode canvas/layers/tokens render the latest controller
document prop rather than a locally cached first-mount FlyEditor. This fixes stale visual state
after edits without resetting the standalone editor's local history.

The outer Dioxus host's session key also incorporates the incoming controller's document
hash, command sequence, page index and selection. A host-side draft or selection replacement
that does not change the server revision now remounts the child instead of continuing to display
stale state from the first `use_signal` initialization. The editor's own internal changes do not
alter the incoming host key. Direct use of the inner component still requires a stable owning
component key or a caller-controlled remount when changing documents.

Both Leptos and Dioxus FlyEditorWorkspace adapters now receive the RegistrySet cloned from
the active AdminCanvasController. Previously they constructed fresh built-in-only registries,
hiding blocks installed by the consumer and contribution host, even though the authoritative
controller could already insert those blocks. The controller remains the registry owner; these
are read-only workspace projections, not independent registration paths.

Dioxus now clears a pending preview's prepared lifecycle state when its consumer facade is
missing. Without this transition the request was never dispatched and could not receive a
response, leaving the editor stuck reporting a preview in progress. A missing Publish facade
still fails visibly without claiming a successful save.

The Leptos AdapterWorkspacePanel now passes derived controller signals for the canonical
project document, selected component, active page and undo/redo availability. The Fly workspace
uses these signals in intent mode to refresh its internal view model after an edit or selection
without remounting the entire widget (and without resetting its UI mode/viewport). The source
of truth is still AdminCanvasController, never a parallel editor instance.

The Leptos adapter passes live effective capabilities to the shared Fly workspace. Host RBAC
and provider-health changes now affect page controls, palette, tokens, property/style editors,
assets and history buttons without a full remount. Back-end authorization remains mandatory.

Dioxus PageBuilderAdmin also forwards FlyEditorWorkspace's own validation/runtime errors through
its `on_error` callback into the session's accessible Page Builder error alert. Previously
invalid token input and editor-local errors could be silently lost because `on_error` was unset.

Contribution registry installation is atomic at both the host and controller boundaries:
all installers must succeed on a candidate registry before it replaces the active registry.
Undo and redo also reject disconnected or currently invalid history snapshots without
changing the controller's document, revision, or history.

A successful document mutation, including undo, redo and recovered merges,
invalidates active drag/drop candidates and all measured selection, hover and insertion
geometry. Surviving component identities are preserved, but hover IDs outside the active page
are cleared. Failed and idempotent commands preserve the existing interaction so it can be retried.

Receipt-bearing SSR operations preflight the owner-scoped receipt ledger,
expected generation and receipt capacity before consumer I/O. This read-only
admission is not a reservation; the final commit still checks the same fences
atomically. It does not make external Pages persistence and draft storage one
transaction.

Strict Pages saves prepare the edited draft and reserve its generation in the
existing operation ledger before external I/O. Only the same operation can
complete the reservation; other writes are fenced. Receipt lookup stays read-only.
Record format 2 protects prepared operations from older readers; deploy upgraded
readers before using the new protocol. Persistent readers reject oversized records,
duplicate operation identities, malformed receipt hashes, out-of-order or future
receipt generations, and prepared reservations inconsistent with the current
generation or record format. File and KV reads also verify the embedded token
against the requested storage key before serving the record. Restored controllers
must match the document fingerprint and core/UI revision projections, including
dirty and conflict flags. These consistency checks are not a cryptographic
authentication mechanism for stored drafts.

Durable draft commits stage candidates outside the memory cache and publish
only after persistent CAS. Loads return the owner-scoped backend snapshot,
never a speculative or divergent cached generation. Ordinary and receipt-bearing commits
reject generation exhaustion instead of accepting two writes at the same generation.
Replacing an existing
cache entry does not evict an unrelated draft.

Live prepared saves are protected from capacity eviction in memory, file and
KV stores. The memory and file owner-admission paths reject new sessions when
only protected slots remain; periodic pruning may temporarily exceed the cap
rather than discard recovery state. Normal TTL expiry and explicit removal
still apply; no indefinite retention is implied.
