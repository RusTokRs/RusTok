# `rustok-web` Documentation

`rustok-web` is the shared Axum boundary crate for RusToK.

It is not a web framework. It exists so that the Loco controller replacement does not
turn into many local copies of response envelopes, status mapping, and extractor glue.

Boundary rules:

- Domain errors stay in owner modules.
- Runtime access helpers belong in `rustok-runtime`.
- Public API contracts that are not Axum-specific stay in `rustok-api`.
- The crate must stay independent from Leptos and other UI frameworks.

