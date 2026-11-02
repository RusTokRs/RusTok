# Bounded sparse SKU combination browsing

Status (2026-10-29): source implementation, **not runtime-certified**. This is ordinary feature
work, not an audit/readiness advancement. The owner remains the
[unified variant-axis contract](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md).
Existing saved-axis changes still require the separate
[preview, full-tuple correction and CAS workflow](./variant-axis-reconfiguration.md).

## One ontology and one writer

`admin/src/core/combinations.rs` and Next Product `src/combinations.ts` are mirrored **local
projections**, not an option dictionary, SKU identity authority, stock service or command provider.
Initial candidates come only from explicitly configured axes/allowed IDs and the effective form,
using the same required-axis/eligibility validation as ordinary save. Saved candidates come from
`ProductDetail.variant_axes` / `variantAxes.allowedValues`, never new schema defaults.

Saved membership is the complete Product detail `variants` projection, not a table page or a
filtered list. The current Product projection queries all tenant/Product variants and reads
assignments by joining the configured canonical axes. Native/GraphQL adapters of the selected
owner must preserve that complete-list contract; a future paged provider must not pass its first
page to this picker as a complete snapshot. The local membership index checks complete tuples,
known allowed IDs, distinct SKU identities and distinct tuples, independent of axis/assignment
order and display labels. A saved Product without axes must already have exactly one default SKU; a
missing default is invalid source, not an automatically repairable empty catalog. Invalid or oversized saved membership refuses the entire candidate
view, even when the bad row falls outside a filter. It never means an empty catalog.

Both admin surfaces also display saved axis assignments beside their actual SKU rows, independently
of candidate-space cardinality. Available dictionary labels are presentation only; retained
assignments without labels remain visible by UUID rather than being dropped or replaced by a
default tuple. An empty assignment list with configured axes is explicitly missing, not default.

The mounted Leptos Variable-kind section no longer duplicates the SKU grid, displays a fabricated
one-variant count before loading, or promises a full generator after saving. It points to the
actual initial picker/saved SKU section instead. This retires only inline read presentation,
not persisted data or any command/caller contract.

No backend command, table, receipt format, identity algorithm or transport fallback was added.
The picker does not issue the existing-axis CAS preview, produce a CAS token or claim admission.
The existing Product/SKU owner commands validate the current configuration and uniqueness again
at actual save under their existing transaction/receipt boundary.

## Bounds and local filters

| Dimension                     | Local bound / behavior                                                  |
| ----------------------------- | ----------------------------------------------------------------------- |
| Configured axes               | At most 32 distinct axes                                                |
| Configured values             | 1–256 distinct known allowed IDs per axis                               |
| Fixed local filters           | At most one known allowed value per axis; Any leaves it unrestricted    |
| Candidate space after filters | At most 4096; refuse larger spaces before enumerating rows              |
| Candidate page                | 12 rows; mixed-radix random access, not a full Cartesian allocation     |
| Saved membership              | At most 256 complete SKU tuples; no first-N availability claim          |
| Initial selected drafts       | At most 128; existing client save bound, not an owner-wide import limit |
| No configured axes            | Exactly one empty/default candidate tuple                               |

The 4096 bound is for browsing, not for the unfiltered allowed space of a Product. Appending a
known complete tuple validates its one-candidate space; a tuple picked after narrowing must not
fail merely because the original unfiltered matrix was larger. This does not narrow the saved
configuration or the operator's filters.

Filters only change this browser's candidate space. They do not modify axis configuration,
allowed-value subsets, EAV, existing SKUs or the author's sparse selection. Within a mounted
picker, reactive metadata/configuration changes retain stale filters and draft rows. A stale filter explicitly refuses until corrected or
reset; Reset filters clears only local filters. The viewport may clamp to a valid last page, but
no source rows or configuration are pruned.
Filters are local, not durable browser state. Dialog/route remounts start a new local browser;
unsubmitted forms are not durable commands. Once standalone creation is admitted, its separate
[shared recovery workflow](./variant-axis-reconfiguration.md#standalone-sku-creation-and-recovery)
retains the full submitted request across reload. That durability is not supplied by the picker.

Incomplete/stale **initial drafts** remain in the editor with a warning and are not indexed as
complete identities. Duplicate complete drafts refuse candidate browsing. Final ordinary save
still validates every retained row, including incomplete manual rows that existed before a pick.
This is not a shortcut around required-axis or duplicate validation.

## Explicit operator actions

### New Product

One click on Add this draft tuple adds exactly **one** local row with the chosen full assignments.
It does not copy SKU code, barcode, price, quantity or policy from a neighboring row or from the
editor's other fields. Next lets the operator edit the new row's scalar controls. Leptos also
provides Fill manual tuple (not added yet): it fills only the existing pending assignment controls,
leaving authored SKU/price/quantity fields untouched, and the existing Add selected SKU action
adds that authored row explicitly. Operators may instead configure an unpriced SKU after saving.

The pending assignment vector is the manual controls' source of truth. Leptos seeds each control
from it and reflects subsequent picks/clears one way; only a user DOM change publishes an
assignment. Mounting a Select never sends its empty initial value back. Unavailable retained
assignments remain visible as disabled choices (by UUID if metadata is missing), not a silently
selected default. Local filter controls use the same one-way reflection so a reset cannot feed
an old control value back into the filter vector.

Shared scalar inventory fields are validated only when the no-axis/no-selected-row path actually
consumes them. Explicit selected rows are validated on their own; an unused invalid manual input
must not prevent saving those valid rows. Adding a manually authored row still validates its
current inventory input before appending.

The existing atomic Product create receives precisely the selected sparse rows; the candidate
count is not a creation count. There is no grid materialization, bulk POST, intermediate default
SKU or subsequent hidden per-SKU write loop.

### Saved Product

The picker is inside the existing new-SKU form/dialog. Use tuple replaces **only** its entire
assignment tuple. It preserves SKU, barcode, shipping/default context, price and stock fields;
the disclosure states this replacement before the action. Native manual axis selectors are
controlled so the visible tuple follows a programmatic pick. The operator still explicitly
submits the existing form; selecting a candidate sends no request.

Next binds the intended create or update callback before saving. If that callback is unavailable,
the draft stays open with an error; editing cannot fall through to creation and missing callbacks
cannot produce a successful close without a write. Scalar/price editing submits no assignments
and does not validate the dialog's old tuple after an axis refresh. Creation still validates the
complete current tuple. Neither path bypasses the owner's live validation.

Selection rechecks current source configuration, filters and full membership. Busy/blocked and
capacity gates are checked again on click, not only when rendering a button. At 256 saved SKUs
this bounded picker no longer prefills new rows; manual saved-SKU creation remains available.
Above the membership bound it refuses instead of trusting a partial source. These are UI bounds,
not a new global restriction on imports or other Product owner commands.

## Money, inventory and identity

The states are SKU exists, Draft selected and No SKU in this snapshot. None means in stock,
reserved, purchasable, priced, channel-listed or publishable. A loaded membership snapshot is
not an availability lease; a concurrent creator can still win, and the owner must refuse the
duplicate at save. Existing Price/Inventory read snapshots are not used to generate candidates.
Pricing and Inventory remain authoritative for their own data.

New initial rows and newly opened saved-SKU forms have **no implicit price row**. A blank scalar
initial/native base amount, or absence of Next standalone currency rows, means no initial price,
not a free price; an explicit `0` still creates a price. An explicitly added Next currency row
starts blank and is incomplete until authored or removed; creation can remove its last row to
return to unpriced. Existing price-update clearing semantics are unchanged. The mounted Leptos
new Product's scalar/manual initial path uses the same absence rule. A compare-at amount without a base amount is invalid and must not be silently discarded.
No positive stock, warehouse assignment, reservation or stock movement is derived from a tuple.
The existing create defaults still apply when an unpriced draft omits inventory initialization.
Historical scalar/pending request framing is not rewritten, and existing Variant price-update
semantics are unchanged.

Local tuple keys and `draft-N` row references are presentation/draft identifiers only. Actual
Variant UUIDs remain the owner's canonical identity. No existing Variant, price, assignment,
stock link or applied migration is deleted by browsing or picking.

## Verification and remaining boundaries

No tests were executed, at the maintainer's request. Added, unrun Rust and MJS scenarios cover
bounded paging, the exact 4096 bound, large-space refusal, local narrowing, order-independent
membership, stale/duplicate/oversized inputs, current-state rechecks, empty/default uniqueness,
explicit single-row append and absent versus zero prices. Review regressions also cover pending
assignment projections/retention, unused scalar inputs, stale edit tuples, create-only tuple
validation, missing writers/no create fallback and disabled/busy guards. The existing price-save
VM harness supplies the current busy/disabled bindings and localized copy callback; these scenarios
remain unrun. The existing package test glob includes
the new MJS source; Rust scenarios are normal unit-test source, not a separate fake application.

Scoped TypeScript core checking, Rust source formatting/parsing, MJS syntax and Fluent generation
are source checks, not runtime evidence. Expanded UI/host typing still fails on the incomplete
shared dependency graph and errors outside Product; it must not be reported as a successful app
build. Rust compilation, all test execution, native/GraphQL mounted behavior, browser-controlled
select/retained-filter behavior, owner races and real save/retry/rollback remain unverified.

Attribute/option update/archive/restore, paged dictionary authoring, stock-safe SKU retirement,
storefront axis selectors, cross-owner Pricing/Inventory/Channel/Index/Cart/Order/Media/Fulfillment
verification and installed-data retirement remain separate work. Axes and standalone SKU
creation now have [durable recovery in source](./variant-axis-reconfiguration.md#standalone-sku-creation-and-recovery),
with native/GraphQL standalone adapters and Next parity. Product create and other Leptos lifecycle
writers still have their documented memory-only retry limitations. Unknown submissions never
retarget when a tuple changes; their full saved request/key/locale must be replayed first.
The picker itself adds no recovery protocol or replacement writer. See the
[active Product plan](./implementation-plan.md#variant-axis-continuation-2026-10-29).
