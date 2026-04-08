# leptos-forms

## Purpose

`leptos-forms` owns generic form-state and validation primitives for Leptos applications in RusToK.

## Responsibilities

- Provide reusable form context state and submit lifecycle helpers.
- Provide field-level bindings and validation composition.
- Keep generic client-side form handling separate from domain-specific UI packages.

## Entry points

- `use_form`
- `FormContext`
- `Field`
- `Validator`
- `FormError`

## Interactions

- Can be used by Leptos applications and UI packages that need generic form handling.
- Complements validation adapters such as `leptos-zod` and state wrappers such as `leptos-hook-form`.
- Stays independent from domain modules and transport-specific API clients.

## Docs

- [Platform docs index](../../docs/index.md)
