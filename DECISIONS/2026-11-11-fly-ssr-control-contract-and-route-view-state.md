# Fly SSR control contract and route-carried view state

- Date: 2026-11-11
- Decision status: Proposed
- Implementation status: In progress
- Owners: fly-browser, fly-leptos, fly-dioxus, rustok-page-builder (admin)
- Extends: [Fly SSR-first browser runtime](./2026-07-16-fly-ssr-first-browser-runtime.md)
- Supersedes: None
- Superseded by: None

## Context

The SSR-first decision makes `ssr + browser-js` the default Page Builder admin runtime: the server
renders the editor, and `fly-browser.js` supplies the browser mechanics. A server-rendered page has
no Leptos event handlers, so a control works only when a Rust view emits a `data-fly-*` attribute
and the adapter binds that same attribute.

That pairing was a convention, and it broke without any failure. The admin panels that emitted
`data-fly-block-id`, `data-fly-action="insert-block"`, `"begin-block-drag"` and
`"select-component"` were replaced by the `fly-leptos` workspace, whose palette emitted
`data-fly-block` with an `on:click` handler. The adapter bound zero elements, the views rendered
inert controls, the adapter tests kept passing against hand-written markup, and every build stayed
green. The viewport switch had the opposite defect: the toolbar emitted
`data-fly-action="set-viewport"`, and the adapter never had a case for it.

The viewport switch also exposed a gap in the SSR-first decision itself. That decision gives the
adapter one channel to the server: a `BrowserIntentEnvelope` posted to the consumer endpoint. The
previewed viewport is not project state and not a command. The server renders the canvas frame
from it, so it must reach the server with the page request. The draft session is the wrong place:
a read-only caller has no draft session, by design.

## Decision

1. **The SSR control contract is a named, two-sided contract.** The consumer side is
   `bindSsrControls` in `crates/ui/fly/browser/assets/fly-browser.js`. The producer side is the
   Rust views of the default admin surface: `crates/ui/fly/leptos/src/editor.rs` and
   `crates/modules/rustok-page-builder/admin/src/editor/`. The vocabulary is:

   | Attribute | On | Meaning |
   | --- | --- | --- |
   | `data-fly-block-id` | palette item | block identity; drag source |
   | `data-fly-can-drag="false"` | palette item | the adapter must not offer a drag |
   | `data-fly-can-insert` | palette item | insert capability, for browser evidence runners |
   | `data-fly-action="insert-block"` | button in the item | posts `insert_block` |
   | `data-fly-action="begin-block-drag"` | button in the item | arms a block drag locally |
   | `data-fly-component-id` + `data-fly-action="select-component"` | layer row | selects locally |
   | `data-fly-action="intent:<name>"` | button | resolved locally or posted as `<name>` |
   | `data-fly-action="set-viewport"` + `data-fly-route-query` | `<select>` | route view state |

2. **The contract is derived, not listed.** `scripts/verify/verify-fly-ssr-control-contract.mjs`
   reads what the adapter binds and what the views emit and compares them in both directions. It
   keeps no allow list of its own. A control that renders and is not handled fails the check unless
   it is recorded as debt in `docs/modules/page-builder-backlog.md`; the debt ledger is empty.

3. **View state that the server renders travels in the route.** A control of this kind carries
   `data-fly-route-query="<key>"`. On `change` the adapter sets `<key>` to the control value in the
   current URL, keeps every other parameter, and navigates. It posts no intent. The view names the
   key through a constant, and the same constant is what the server resolves before it renders.
   The first such key is `fly_viewport`, resolved by `route_viewport` in
   `crates/modules/rustok-page-builder/admin/src/editor/toolbar.rs` and applied in `AdminCanvas`
   before any child renders.

4. **A control is not rendered without its transport.** The palette drag control renders only when
   the host passes `on_begin_drag`, which `FlyEditorWorkspace` does in intent mode only.

5. **Both framework adapters render the same contract markup.** `fly-dioxus` renders the palette
   item, the insert and drag controls and the layer row with the attributes of the table above.
   Its handlers are always live, so it does not depend on `fly-browser.js`; the markup is shared
   so that styles, browser evidence runners and a future server-rendered Dioxus host see one
   palette.

## Sources of truth and ownership

- The adapter source is the source of truth for what is bound. The views are the source of truth
  for what is emitted. Neither is copied into the verifier.
- `fly-browser` owns the adapter and the meaning of each attribute. `fly-leptos` owns the palette
  and layer markup. The Page Builder admin owns the toolbar markup and the route keys it resolves.
- Route view state is owned by the surface that renders from it. It is not persisted, not part of
  the project document, and not part of the draft session.
- Dependency direction is unchanged: views do not depend on the adapter; both depend on the
  attribute vocabulary above.

## Invariants

- Every attribute and action the adapter binds is emitted by a view of the default admin surface.
- Every action a view emits is handled: by the adapter, or by the server through `intent:<name>`
  where `<name>` is a `BrowserIntentKind` with an arm in `dispatch_named_intent`.
- Every contract part has a render test on the composed admin surface, a render test in
  `fly-leptos` when that crate emits it, and a browser test that loads the shipped adapter.
- Every contract part that the `fly-leptos` workspace emits is emitted by the `fly-dioxus`
  workspace.
- A `data-fly-route-query` value is a constant that a server resolver reads and that is applied
  before render.

### Allowed states

- A control that needs a transport the host did not provide is absent.
- A control whose capability is denied renders disabled; the server still refuses the intent.
- An unknown, empty or stale route value renders the current view state.

### Forbidden states

- A control that renders enabled and does nothing in the default runtime.
- An adapter binding with no producer.
- View state stored in the draft session, or sent as a mutating intent, in order to reach the
  server renderer.
- A route key written by the adapter and not resolved by the server.

## Non-goals

- The hydrated `wasm-client` profile. It attaches Leptos handlers next to the adapter; what the
  adapter must do there is not decided here and is recorded as open in the backlog.
- Panel parity of the Dioxus workspace beyond the palette and the layer tree. It has no page
  tabs, property, style or asset panels; that is the adapter's own parity work.
- Workspace controls that still have no SSR path (page tabs, page and component properties, style
  rules, assets, theme tokens inside `FlyEditorWorkspace`). They are recorded as open.
- Localization of viewport preset labels.

## Data, transaction, and concurrency boundary

Not applicable. Route view state is not persisted and takes part in no transaction. Intents posted
by the bound controls keep the draft-session and revision rules of the consumer endpoint.

## Context dimensions

- Principal and policy: capabilities decide whether a control is enabled. Route view state applies
  to every caller that may open the editor, including a read-only one.
- Tenant, channel, locale, timezone, currency: not applicable to the contract. Control labels come
  from the host message catalog for the request locale.

## Events and projections

Not applicable. The contract emits no domain event and owns no projection.

## Failure semantics

- A drifted contract fails `verify-fly-ssr-control-contract.mjs` in CI through
  `verify-fly-ssr-first.mjs`; it is not detected at run time.
- An unknown route value is ignored. It does not fail the page.
- A denied capability is enforced by the server endpoint. The disabled attribute and
  `data-fly-can-drag="false"` only stop the browser from offering a refused gesture.

## Migration and cutover

- `data-fly-block` is removed from the Leptos and Dioxus palettes; nothing read it.
- `EditorMessages` gains `insert_block` and `drag_block` with serde defaults, so message sets
  serialized before this change still deserialize.
- No data migration. No external contract changes: the intent names and payloads are those the
  server already dispatched.

## Alternatives considered

- **Post a `set_viewport` intent and keep the viewport in the draft session.** Rejected. It needs a
  new `BrowserIntentKind`, a capability rule and a reload rule in the Pages endpoint, and it still
  does nothing for a read-only caller, who has no session.
- **Resize the canvas frame in the adapter only.** Rejected. Every mutating intent reloads the
  page, so the frame would jump back to the server default after each edit, and the server-side
  geometry would disagree with the frame.
- **Remove `data-fly-action="set-viewport"` and leave the select to hydration.** Rejected. It keeps
  a visible control that does nothing in the default runtime.
- **Keep a hand-written list of contract attributes in the verifier.** Rejected. The list becomes
  a third copy that drifts; this is the failure the decision removes.

## Verification

- `node scripts/verify/verify-fly-ssr-control-contract.mjs`, imported by
  `scripts/verify/verify-fly-ssr-first.mjs`, which the "Fly Page Builder" workflow runs.
- Render tests: `crates/ui/fly/leptos/src/editor.rs` (`mod tests`) and
  `crates/modules/rustok-page-builder/admin/src/ssr_control_contract_tests.rs`.
- Browser tests: `apps/next-admin/tests/e2e/fly-browser-ssr-controls.spec.ts`, which loads the
  shipped adapter.
- Unit tests of `route_viewport` in `toolbar.rs`.

## Consequences

- The palette, the layer tree and the viewport switch work in the default runtime.
- Adding an SSR control means adding both sides, a render test and a browser test; the check names
  the missing part.
- A viewport change is a navigation. This matches the reload that already follows every mutating
  intent, and it makes the previewed viewport shareable through the URL.
- The `fly-leptos` render tests are compiled but not executed by the "Fly Page Builder" workflow
  (`cargo check --all-targets`). The admin render tests, which mount the same workspace, are
  executed (`cargo test -p rustok-page-builder-admin --lib`). Running the `fly-leptos` tests in CI
  needs a workflow change and is recorded in the backlog.
