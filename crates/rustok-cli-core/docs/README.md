# `rustok-cli-core` Documentation

`rustok-cli-core` is the contract layer for platform CLI command providers.

It is intentionally not a parser crate and not a central catalog of all commands. A future
`rustok-cli` binary can use these contracts to aggregate command providers from generated
distribution registries and module-local `cli/` adapter packages.

Boundary rules:

- Domain crates do not depend on CLI contracts.
- Module-local `cli/` adapter packages may depend on the domain crate and on
  `rustok-cli-core`.
- `apps/server` does not depend on CLI code.
- Parser/output/terminal UX can live in the binary crate, while command metadata and
  machine-readable outcomes live here.

