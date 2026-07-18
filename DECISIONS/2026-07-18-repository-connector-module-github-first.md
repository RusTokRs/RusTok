# Repository connector module with GitHub as the first provider

- Status: Accepted
- Date: 2026-07-18

## Context

RusToK needs a multi-connector for external source repositories. The first
workflow stores a user's Rhai module and its Rust counterpart in two linked
repositories. The initial provider is GitHub. Additional Git forges may be
added later, but introducing a separate crate or module for every provider
would split one lifecycle and make the marketplace integration harder to
reason about.

## Decision

Create one Alloy capability submodule, named `alloy-repository`, that
owns external repository connections and repository bindings. GitHub is the
first provider and is implemented inside this module. Future providers are
added behind the same module-owned connector contract; they do not become
independent RusToK modules unless they acquire a genuinely separate domain
or lifecycle.

The module exposes provider-neutral records for:

- an authenticated repository connection (provider, account, scopes and
  secret reference);
- a repository binding (owner, repository, revision/branch and path);
- a logical module's linked source bindings, including `rhai_source` and
  `rust_source`;
- normalized repository metadata that marketplace and AI integrations can
  consume without reading provider internals.

GitHub-specific OAuth, API, webhook and repository mapping code stays inside
the module's provider implementation. Other modules use the repository
module's typed ports/events rather than importing GitHub client types.

## Boundaries

- `alloy-repository` owns connection credentials by reference, provider API
  calls, repository binding state and synchronization status.
- `rustok-modules` remains the owner of module artifact identity, admission,
  installation and release lifecycle.
- Marketplace owns catalog and publication projections; it may display the
  Rhai and Rust source bindings but does not own provider credentials.
- AI/RAG consumes an explicit normalized repository/document projection. It
  does not crawl repositories or inspect connector tables directly.
- Storage remains the physical file/object owner. Repository checkouts or
  exported archives use storage through its public contract.

## Consequences

The first implementation can be GitHub-focused without locking the public
model to GitHub names. Adding another provider later means adding an internal
adapter and mapping tests, while the marketplace and AI contracts remain
unchanged. Provider-specific feature differences must be represented as
capabilities or explicit unsupported operations, not by leaking provider
types across module boundaries.

The module is planned as an Alloy capability module; this ADR does not yet
claim that its crate, migrations or transports are implemented.
