# ADR: Alloy module scaffold as the first real MCP product slice

## Status

Accepted

## Context

RusToK already had:

- `rustok-mcp` as an MCP transport/tool adapter over `rmcp`;
- `alloy` as the Alloy runtime/engine capability;
- `rustok-alloy` as GraphQL/REST transport for Alloy runtime management.

But this stack still looked like foundation without a clear first vertical product slice. We needed a
real `AI -> MCP -> Alloy -> Platform` capability that:

- is not just script CRUD;
- creates a tangible platform artifact;
- stays governed and reviewable;
- does not pretend to be full autonomous module generation.

## Decision

We introduce `alloy_scaffold_module` in `rustok-mcp` as the first real Alloy product slice.

The tool:

- accepts structured `ScaffoldModuleRequest`;
- can preview a draft `crates/modules/rustok-<slug>` module skeleton;
- can write that skeleton to disk when `write_files=true`;
- generates only a draft scaffold aligned with RusToK crate conventions;
- does not register the module in runtime;
- does not generate the final permission/resource surface in `rustok-core`;
- does not bypass review/apply boundaries.

## Consequences

Positive:

- Alloy now has a concrete platform-building action instead of only runtime/script management.
- MCP becomes a more honest AI-to-platform boundary for creation scenarios.
- We get a safe stepping stone toward richer module generation and code review flows.

Constraints:

- This is not a full script-to-native-module compiler.
- Generated crates remain drafts until a human or later pipeline finishes permissions, domain logic,
  runtime wiring, and review.
- Local docs must continue to treat official MCP/rmcp docs as the source of truth for protocol and
  SDK behavior.

## Next steps

1. Add review/apply boundaries above scaffolded code.
2. Extend generated output with richer RusToK hints and capability metadata.
3. Move from draft scaffolding toward a broader Alloy codegen pipeline.

