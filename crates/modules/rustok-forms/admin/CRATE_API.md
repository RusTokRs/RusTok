# Crate API: rustok-forms-admin

## Minimum Contract Set
- **Fluid Frontend Architecture (FFA)**: Strict separation of framework-agnostic models, submission summarizers, state machine transition validators, and localized label resolvers in `core.rs` from Leptos template rendering.
- **Pure Rust Domain Invariants**: Verification of allowed triage transitions (`New` -> `Read`, `Handled`, `Spam`; `Read` -> `Handled`, `Spam`; `Spam`/`Handled` -> `Read`) and payload formatting via pure unit tests without Leptos or DOM runtime.
- **Headless GraphQL Transport**: Type-safe queries (`formSubmissions`) and mutations (`updateFormSubmissionState`) with tenant scoping and nil-UUID prevention.
- **Zero Fallbacks**: Transparent error display without silent data masks.

## Platform Capability Bindings
- **`rustok-ui-i18n`**: Bilingual support (`en`, `ru`) across all 10 triage action, state, and header keys.
- **`rustok-graphql`**: GraphQL request execution (`execute_graphql`).
- **`leptos-auth`**: Authentication session token (`use_token`) and tenant context (`use_tenant`).

## Domain Collaborations
- **`rustok-forms`**: Consumes backend form submission entities, triage state machines, and honeypot indicators.
- **`rustok-page-builder`**: Manages incoming form leads captured by page-builder interaction forms.

## Public surface

### Components
- `FormsAdmin`: Standalone Leptos admin view component hosting the triage inbox, state transitions, and error alert banners.

### Core Domain APIs
- `FormsAdminLabels`: Localized labels for inbox header, subtitle, and empty states.
- `SubmissionAction`: Actionable triage transition descriptor containing target state and localized button label.
- `allowed_transitions`: Resolves valid next triage transitions based on the current submission state.
- `state_label`: Resolves localized human-readable label for a submission state string.
- `summarize_submission`: Formats human-readable summary of page ID and up to 6 scalar payload fields.

### Transport
- `fetch_form_submissions`: Asynchronously loads form submissions for the authenticated tenant.
- `set_form_submission_state`: Asynchronously mutates submission triage state with nil-UUID validation.

### Input DTOs/Commands
- `FormSubmission`: Admin model projection for form submissions (`id`, `form_id`, `locale`, `page_id`, `payload`, `state`, `created_at`, `handled_at`, `handled_by`).

### Domain Invariants
- **Strict FFA Separation**: `src/core.rs` contains zero imports of UI framework types (`leptos::*`).
- **Triage State Machine**: State transitions strictly follow platform rules (`new -> read | handled | spam`, `read -> handled | spam`, `handled | spam -> read`).
- **Nil UUID Prevention**: Submission state updates reject nil UUIDs prior to dispatch.

### Events / Outbox Side Effects
- Form triage inbox actions are client-side UI mutations dispatched via GraphQL; outbox events are managed by the backend host.

### Errors / Failure Codes
- `ApiError`: GraphQL query or network error wrapper.
