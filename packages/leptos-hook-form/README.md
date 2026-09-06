# Leptos Hook Form

Internal, minimal helpers for **form state management** across admin and storefront
frontends (Leptos + Next.js). This is not a full React Hook Form clone; it is a
small shared contract so both stacks handle form state and errors the same way.

Target apps:
- `apps/admin` (Leptos CSR)
- `apps/storefront` (Leptos SSR)
- `apps/next-frontend` (Next.js)

---

## Why this exists

- Next.js uses `react-hook-form` + `zod`.
- Leptos has no 1:1 alternative yet.
- We keep a tiny contract for form state + errors so UI behavior stays identical.

---

## Contract

- `FormState` (`is_submitting`, `form_error`, `field_errors`)
- `FieldError` (`field`, `message`)
- `ValidationIssue` to represent validation errors in a runtime-agnostic way.

---

## Runtime implementations

### 1) Leptos (Rust)
Rust helpers live in `crates/ui/leptos-hook-form`.

Exports:
- `FormState`, `FieldError`
- `ValidationIssue`
- `issues_to_field_errors(...)`

### 2) Next.js (TypeScript)
TypeScript helpers live in `packages/leptos-hook-form/next`.

Exports:
- `FormState`, `FieldError`, `ValidationIssue`
- `issuesToFieldErrors(...)`

---

## Usage (examples)

### Leptos
```rust
use leptos_hook_form::{FormState, ValidationIssue, issues_to_field_errors};

let issues = vec![ValidationIssue {
    path: vec!["email".to_string()],
    message: "Invalid email".to_string(),
}];

let field_errors = issues_to_field_errors(&issues);
let state = FormState::with_field_errors(field_errors);
```

### Next.js
```ts
import { issuesToFieldErrors, FormState } from "@/lib/leptos-hook-form";

const issues = [{ path: ["email"], message: "Invalid email" }];
const fieldErrors = issuesToFieldErrors(issues);
const state = FormState.withFieldErrors(fieldErrors);
```

---

## Status

**Minimal baseline** for shared form state + errors. Extend only when needed.

