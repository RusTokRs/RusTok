# Leptos Zustand

Internal, minimal store helpers to mirror zustand-like patterns across admin and
storefront frontends (Leptos + Next.js).

Target apps:
- `apps/admin` (Leptos CSR)
- `apps/storefront` (Leptos SSR)
- `apps/next-frontend` (Next.js)

---

## Why this exists

- Next.js uses `zustand` for lightweight state management.
- Leptos uses signals, but we want a shared store contract for parity.
- This kit defines minimal store snapshot/update shapes.

---

## Contract

- `StoreSnapshot<T>`
- `StoreUpdate<T>`

---

## Runtime implementations

### 1) Leptos (Rust)
Rust helpers live in `crates/ui/leptos-zustand`.

Exports:
- `StoreSnapshot`, `StoreUpdate`

### 2) Next.js (TypeScript)
TypeScript helpers live in `packages/leptos-zustand/next`.

Exports:
- `StoreSnapshot`, `StoreUpdate`

---

## Usage (examples)

### Leptos
```rust
use leptos_zustand::{StoreSnapshot, StoreUpdate};

let snapshot = StoreSnapshot::new("ready");
let update = StoreUpdate {
    previous: "idle",
    next: "ready",
};
```

### Next.js
```ts
import { StoreSnapshot } from "@/lib/leptos-zustand";

const snapshot: StoreSnapshot<string> = { state: "ready" };
```

---

## Status

**Minimal baseline** for shared store shapes. Extend only when needed.

