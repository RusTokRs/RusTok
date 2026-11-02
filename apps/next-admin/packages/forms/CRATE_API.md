# apps/next-admin/packages/forms / CRATE_API

## Purpose

`@rustok/forms-admin` (`apps/next-admin/packages/forms`) is the Next.js administration UI package for the `rustok-forms` module. It provides lead triage inbox interfaces for reviewing, reading, handling, and flagging spam form submissions collected by storefronts and page-builder forms.

## Architecture & Responsibilities

- **Module Registration:** Registers the forms module with the host admin registry via `registerAdminModule` and supplies structured navigation items (`formsNavItems`) under the `modulePlugins` group.
- **Triage Inbox:** Implements `FormsPage` / `FormsAdminPage` displaying paginated form submissions with submission timestamp, form identifier, locale, state, and formatted JSON payload.
- **State Machine Transitions:** Exposes `SubmissionActions` enabling operators to advance submissions across the 4 canonical lifecycle states: `new` → `read` → `handled` or `spam`.
- **GraphQL Integration:** Directly dispatches typed queries (`FORM_SUBMISSIONS_QUERY`) and mutations (`UPDATE_FORM_SUBMISSION_STATE_MUTATION`) against the backend `rustok-forms` GraphQL schema.

## Public Surface

- `formsNavItems`: Navigation configuration for the Next.js admin sidebar under `modulePlugins` (`title: 'Forms'`, `url: '/dashboard/forms'`).
- `FormsPage` / `FormsAdminPage`: Server-rendered inbox container for listing form submissions.
- `SubmissionActions`: Client component providing actionable state transition buttons with optimistic transition states.
- `FORM_SUBMISSIONS_QUERY`, `UPDATE_FORM_SUBMISSION_STATE_MUTATION`: Exported GraphQL documents.
- `listFormSubmissions(filter, opts)`: Fetches paginated form submissions filtered by form ID or state.
- `updateFormSubmissionState(id, state, opts)`: Transitions a submission's lifecycle state.
- `FormSubmission`, `FormSubmissionState`, `FormSubmissionsFilter`, `GqlOpts`: TypeScript domain types.

## Invariants & Guarantees

1. **GraphQL Contract Hygiene:** Query and mutation documents match the backend `rustok-forms` schema (`ListGqlFormSubmissionsFilter`, `UpdateGqlFormSubmissionStateInput`) exactly.
2. **State Machine Invariants:** State transitions strictly adhere to the 4 valid states: `'new'`, `'read'`, `'handled'`, `'spam'`.
3. **Safe JSON Rendering:** Payloads are serialized cleanly inside `<code>{JSON.stringify(payload)}</code>` without risking XSS or runtime rendering crashes.
4. **Parity with Leptos Admin:** State indicators, transition actions, and inbox concepts maintain full functional parity with `crates/modules/rustok-forms/admin`.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19, `next/navigation`, and `@/modules/registry` from `apps/next-admin`.
- **Backend API:** Communicates with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Table Access:** Never queries `form_submissions` database tables directly; all access is mediated through GraphQL.

