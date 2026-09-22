# `commerce` as the root module of the ecommerce family and the matryoshka model of submodules

- Date: 2026-03-25
- Status: Accepted

## Context

After splitting `rustok-commerce` into `product`, `pricing`, and `inventory`, a second architectural question arose:

- how to keep the old `commerce` as the main module of the domain;
- how to avoid returning to a fat monolith;
- how to allow replacing default submodules with custom implementations in the future, similar to the Medusa approach
  and provider submodules like `payment -> stripe`.

Three levels needed to be separated:

- root family module;
- domain submodules;
- provider/submodule hierarchy within individual domain modules.

## Decision

Establish the `matryoshka` model for the ecommerce domain:

- `rustok-commerce` remains the root platform module of the `ecommerce` family;
- `rustok-product`, `rustok-pricing`, `rustok-inventory` are the default submodules of this family;
- subsequent domain modules (`cart`, `order`, `customer`, `payment`, `fulfillment`, ...) follow the same pattern;
- provider submodules live one level below, e.g. `payment -> stripe`, `payment -> custom-psp`.

Role of `rustok-commerce`:

- umbrella/root module of the family;
- orchestration and compatibility facade;
- top documentation and runtime entry point for the ecommerce family.

Role of child modules:

- own their domain and storage;
- act as the default implementation of a capability slot within the ecommerce family.

Important constraint:

- `rustok-commerce` is the root of the family in architectural and runtime terms;
- but it must not be a lower shared dependency for its own child modules;
- shared DTO/contracts/helpers remain in a separate support crate (`rustok-commerce-foundation`) to
  avoid creating dependency cycles.

## Consequences

Positive:

- a clear hierarchy `family -> submodules -> provider submodules` emerges;
- the old `commerce` retains its role as the main module of the domain;
- the path to replaceable submodules and provider model is preserved.

Negative:

- the root module must not be confused with a lower shared base crate;
- for true runtime replaceability, capability/provider selection in the manifest/runtime will need to be introduced later.

Follow-up:

- describe the capability slots of the `commerce` family;
- define how the manifest will select the default provider submodule;
- prevent direct collapsing of `rustok-commerce-foundation` back into the umbrella crate.
