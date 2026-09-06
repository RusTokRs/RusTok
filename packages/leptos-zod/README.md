# Leptos Zod

Internal, minimal validation helpers that mirror the **zod-style** contract used
in Next.js, so Leptos frontends can share the same error structure.

Target apps:
- `apps/admin` (Leptos CSR)
- `apps/storefront` (Leptos SSR)
- `apps/next-frontend` (Next.js)

---

## Why this exists

- Next.js uses `zod` for schema validation and error reporting.
- Leptos needs an equivalent error shape to keep parity in UX.
- We keep a tiny error contract (`ZodIssue`/`ZodError`) and mapping helpers.

---

## Runtime implementations

### 1) Leptos (Rust)
Rust helpers live in `crates/ui/leptos-zod`.

Exports:
- `ZodIssue`, `ZodError`

### 2) Next.js (TypeScript)
TypeScript helpers live in `packages/leptos-zod/next`.

Exports:
- `ZodIssue`, `ZodError`
- `mapZodError(...)`

---

## Usage (examples)

### Leptos
```rust
use leptos_zod::{ZodError, ZodIssue};

let error = ZodError::from_api(vec![ZodIssue {
    path: vec!["email".to_string()],
    message: "Invalid email".to_string(),
}]);
```

### Next.js
```ts
import { mapZodError } from "@/lib/leptos-zod";

const error = mapZodError({
  issues: [{ path: ["email"], message: "Invalid email" }],
});
```

---

## Status

**Minimal baseline** for validation error parity. Extend only when needed.

