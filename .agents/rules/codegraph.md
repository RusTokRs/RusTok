# CodeGraph Exploration Policy (Conditional)

## Pre-condition: Tool Availability
CodeGraph is an environment-specific code intelligence tool present in this local development environment, but it is NOT used or required in other environments or CI.
- The agent MUST check if CodeGraph is available before attempting to use it:
  - As an MCP tool: `codegraph_explore` is present in the available toolset.
  - Or as a CLI tool: `codegraph` command is available (or accessible at `C:\Users\user\AppData\Local\codegraph\current\bin\codegraph.exe`).
- If CodeGraph is **NOT** available in the environment:
  - Do NOT fail, warn, or attempt to install/initialize it.
  - Gracefully fall back to standard repository search and inspection tools (`grep_search`, `view_file`).

## Usage Guidelines (When Available)
When CodeGraph is available in the environment:
- **Prioritize CodeGraph for architectural exploration:** When surveying architecture, tracing call chains, investigating "how does X work", or discovering relationships across modules, call `codegraph_explore` (or CLI `codegraph explore <query>`) FIRST.
- **Blast radius analysis:** Before modifying shared symbols, functions, or types, use CodeGraph to analyze who calls them and what depends on them (`codegraph_explore` or CLI `codegraph impact <symbol>` / `codegraph callers <symbol>`).
- **Avoid redundant grep loops:** Do not run broad manual `grep_search` or sequential `view_file` loops over code that is already indexed in `.codegraph/` (~7 950+ files). Source returned by CodeGraph should be treated as already read.
- **Fallback boundary:** Use `grep_search` and `view_file` primarily for unindexed content (e.g. documentation, configs, temporary scratch files) or to verify specific immediate local edits.
