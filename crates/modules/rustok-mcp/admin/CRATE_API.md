# rustok-mcp-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for the Model Context Protocol (MCP) server control plane.
- Integrates with `leptos`, `leptos-auth`, and `rustok-api`.

## Domain Collaborations
- Collaborates with `rustok-mcp` for registered MCP tools, resources, prompts, and server instances.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod core`: Framework-neutral presentation contracts and view models.
- `pub mod transport`: Native server functions and remote MCP facades.
- `pub mod ui`: Leptos presentation views (`McpAdmin`).
- `pub use ui::McpAdmin`: Root admin view component.

## Minimum Contract Set

### Input DTOs/Commands
- MCP server configuration, tool execution requests, and resource inspection parameters.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Native server functions consume host-provided `HostRuntimeContext`.
- Bounded tool execution and strict tenant permission checks.

### Events / Outbox Side Effects
- Admin UI adapter package; tool invocations delegate execution to `rustok-mcp`.

### Errors / Failure Codes
- Protocol errors and execution failures are surfaced through typed UI notices.
