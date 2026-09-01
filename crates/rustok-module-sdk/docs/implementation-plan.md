# Implementation Plan for `rustok-module-sdk`

## Scope

Own the canonical external WIT package and generated Rust guest bindings used
by author templates and the isolated module build pipeline. The SDK does not
own marketplace, build orchestration, runtime policy, capability authorization,
or host infrastructure.

## Current State

- [x] Package the single canonical `module-runtime` WIT world.
- [x] Generate guest imports, `Guest`, and the public export macro with
  Bytecode Alliance `wit-bindgen`.
- [x] Generate Wasmtime host bindings from the same canonical WIT file.
- [x] Keep the independently versioned project renderer in
  `rustok-module-template` while this crate remains the sole guest binding
  owner.
- [ ] Compile the exact rendered fixture with the pinned native Rust component
  target and toolchain in isolated CI.
- [x] Publish SDK/template identity into the canonical module init/build CLI
  flow. `rustok module init` invokes the template renderer, returns exact SDK
  and template versions in its owner-local outcome, and produces the pinned
  source manifest consumed by the isolated build request.

## Completion Condition

The SDK slice is complete when the canonical CLI can initialize a buildable
component from the current template, the isolated worker validates its produced
WIT surface, and no repository-owned caller maintains a duplicate ABI model.
