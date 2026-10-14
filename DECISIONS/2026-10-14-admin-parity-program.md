---
id: decision://2026-10-14-admin-parity-program
kind: architecture_decision
status: accepted
date: 2026-10-14
documented_language: en
---

# Both admins stay: functional parity over the shared GraphQL

## Decision

RustTok keeps two admins — the Rust FFA admin (single Leptos connector,
`apps/admin`) and the classic Next admin (`apps/next-admin`). Neither replaces
the other. Functional consistency is defined as: every user-facing module
capability is reachable in both admins through the **module-owned** UI of that
admin (Rust FFA packages in `crates/modules/*/admin`, Next packages in
`apps/next-admin/packages/*`), over the **same GraphQL schema and permissions**.
`docs/modules/admin-parity.md` is the parity matrix and update contract.

## First steps (2026-10-14)

After `@rustok/pages-admin`, the same pattern lands `@rustok/media-admin`
(library list/usage/delete — the assets builder pages reference) and
`@rustok/forms-admin` (the submission inbox with the `new/read/handled/spam`
triage machine), wired identically: host dependency, module registration in
`src/modules/index.ts`, and a session-fed `/dashboard/<module>` route. Both were
verified against the Rust GraphQL sources (`media(tenantId: UUID!, …)`,
`formSubmissions(filter)`, `updateFormSubmissionState(id, input)`) and
syntax-checked with TypeScript 5.

## Boundaries

- Deep-linked asymmetry is allowed and documented per surface: the visual
  builder is Rust-FFA-only; Next manages pages and links into it. Internal-only
  modules (outbox/index) may stay one-sided with the reason recorded in the
  matrix.
- No admin-private endpoints: a missing capability first becomes a module
  GraphQL operation both admins can use.

## Verification

`next build`/`tsc --noEmit` (no `node_modules` here), `cargo test` for the FFA
side, and one `npm install` to confirm the hand-maintained lock entries remain
release blockers for the maintainer/CI (backlog item 1 and 7).
