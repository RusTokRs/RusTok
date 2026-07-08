# `rustok-cli-core` Documentation

`rustok-cli-core` is the contract layer for platform CLI command providers.

It is intentionally not a parser crate and not a central catalog of all commands. The
`rustok-cli` binary uses these contracts to aggregate command providers from generated
distribution registries and module-local `cli/` adapter packages.

Boundary rules:

- Domain crates do not depend on CLI contracts.
- Module-local `cli/` adapter packages may depend on the domain crate and on
  `rustok-cli-core`.
- `apps/server` does not depend on CLI code.
- Parser/output/terminal UX can live in the binary crate, while command metadata and
  machine-readable outcomes live here.
- Providers expose command discovery and typed execution through the same
  `CommandProvider` contract; discovery-only providers can rely on the default
  unknown-command execution result until their command body is implemented.
- The runner passes normalized command input in `CommandRequest.args` as an
  object with `options` and `positionals`; provider crates should not parse raw
  terminal tokens themselves.

Current entry points:

- `CommandDescriptor`
- `CommandRequest`
- `CommandOutcome`
- `CommandProvider`
- `CommandProvider::execute`
- `CliCoreError`

Use this crate from module-local `cli/` adapter packages, generated CLI registries or the
`rustok-cli` binary.
Do not add CLI command implementations to module domain crates or to the production HTTP
server runtime.

Related guide: [Backend Module Architecture](../../../docs/backend/module-backend-architecture.md).
