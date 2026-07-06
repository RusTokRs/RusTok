---
id: doc://docs/templates/module_contract.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Module Documentation Template

This template is needed for new platform modules, as well as for support/capability crates that want to comply with the current RusToK documentation contract.

The normative path for module-level documentation is as follows:

- root `README.md` next to the code;
- local `docs/README.md`;
- local `docs/implementation-plan.md`;
- `rustok-module.toml` when necessary.

Do not create a separate central doc for each module in `docs/modules/`. Central docs should reference local documentation, not duplicate it.

## 1. Minimum file set

For a new path-module, the following set is expected:

```text
crates/rustok-<slug>/
  Cargo.toml
  README.md
  rustok-module.toml
  docs/
    README.md
    implementation-plan.md
```

For support/capability crate, `rustok-module.toml` is not mandatory if the crate is not included in `modules.toml`.

## 2. Root `README.md`

The root README must be in English and contain this framework:

```md
# rustok-<slug>

## Purpose

One short paragraph explaining what this crate owns.

## Responsibilities

- Responsibility 1
- Responsibility 2
- Responsibility 3

## Entry points

- `MainType`
- `MainService`
- `controllers::routes`

## Interactions

- Interaction with `apps/server`
- Interaction with other modules/crates
- Notes about UI packages or runtime wiring

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
```

Rules:

- one file — one language;
- `README.md` does not replace local docs;
- `Docs` section is mandatory;
- section names must match the contract form:
  - `## Purpose`
  - `## Responsibilities`
  - `## Entry points`
  - `## Interactions`

## 3. Local `docs/README.md`

Local docs README is written in English and describes the live module contract.

Minimum framework:

```md
# <Module name>

## Purpose

Briefly: what the module does and why it exists.

## Responsibility scope

- What the module owns
- What the module consciously does not own

## Integration

- GraphQL / REST / background tasks / UI surfaces
- host wiring and runtime boundaries
- dependencies on other modules and crates
- especially important cross-module contracts

## Verification

- `cargo xtask module validate <slug>`
- `cargo xtask module test <slug>`
- other targeted commands when needed

## Related documents

- `implementation-plan.md`
- central docs
- neighboring host/module docs
```

Additional sections are allowed if they are really needed for the module:

- `## Settings and configuration`
- `## Health and observability`
- `## Limitations`
- `## UI contract`

But the minimum sections above should remain in place.

## 4. Local `docs/implementation-plan.md`

This file captures the live plan to bring the module to the target state, not a detailed work history.

Minimum framework:

```md
# <Module> Development Plan

## Scope of work

Briefly: what the current plan is focused on.

## Current state

Briefly: what is already stabilized and what invariants the module already maintains.

## Stages

### 1. Nearest slice

- ...

## Verification

- `cargo xtask module validate <slug>`
- `cargo xtask module test <slug>`

## Update rules

1. When changing runtime/module contract, update this file first.
2. When changing public surface, synchronize `README.md` and `docs/README.md`.
3. When changing manifest metadata, synchronize `rustok-module.toml`.
```

Additional sections are allowed:

- `## Risks and open questions`
- `## Priorities`
- `## Readiness criteria`

But `## Scope of work`, `## Current state`, `## Stages`, `## Verification` and `## Update rules` must be present as a minimum standard.

## 5. `rustok-module.toml`

For a path-module from `modules.toml`, a local manifest is mandatory.

Minimum framework:

```toml
[module]
slug = "<slug>"
name = "<Name>"
version = "0.1.0"
description = "At least one publish-ready sentence."
ownership = "first_party"
trust_level = "verified"
ui_classification = "dual_surface"

[crate]
entry_type = "<PascalSlug>Module"
```

For a core module that is added to `modules.toml` with `required = true`, use `trust_level = "core"`.

If the crate implements `RusToKModule`, `entry_type` is mandatory and must match the actual runtime entry type in `src/lib.rs`.
If the crate does not implement `RusToKModule` and is used as a capability-only layer, `entry_type` can be omitted.

Then add as needed:

- `[provides.graphql]`
- `[provides.http]`
- `[provides.admin_ui]`
- `[provides.storefront_ui]`
- `[settings]`
- `[marketplace]`

The detailed contract layer is described in [docs/modules/manifest.md](../modules/manifest.md).

## 6. Mandatory local verification

For a new or significantly changed platform module:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

If the composition of `modules.toml` changes, add:

```powershell
cargo xtask validate-manifest
```

The minimum Windows verification path is described in [docs/verification/README.md](../verification/README.md).

## 7. What not to do

- do not write root `README.md` in Russian;
- do not store the module's only documentation in `docs/modules/`;
- do not add a path-module to `modules.toml` without `rustok-module.toml`;
- do not consider `admin/` and `storefront/` subfolders proof of integration without manifest wiring;
- do not turn local docs into a historical changelog if you need a live contract.

## 8. Related documents

- [Documentation map](../index.md)
- [Modular platform overview](../modules/overview.md)
- [Manifest layer contract](../modules/manifest.md)
- [Index of local module documentation](../modules/_index.md)
