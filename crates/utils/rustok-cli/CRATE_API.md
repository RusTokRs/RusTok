# rustok-cli / CRATE_API

## Role

`rustok-cli` is the terminal-facing platform CLI runner for RusToK maintenance, distribution, and module commands.
It parses command-line arguments, enforces namespace dispatch policies, and coordinates command execution across registered providers.

## Public surface

### Primary Public Types

- `CliExit`: Process exit representation containing status code, stdout, and stderr output buffers.
- `CommandRegistry`: Aggregator maintaining registered command providers and namespaced dispatch routes.

### Primary Functions

- `run_with_args`: Execute a CLI command with caller-supplied argument vectors.
- `run_with_environment`: Build the runtime from environment configuration before dispatching arguments.
- `collect_commands`: Collect command descriptors from all registered providers.
- `render_command_list`: Render human-readable help and command listing text.
- `render_command_list_json`: Output structured JSON inventory of registered commands for tooling integration.

## Invariants

- **Namespace Uniqueness**: Duplicate `namespace/name` registrations are rejected before command execution.
- **Normalized Arguments**: Raw terminal arguments are normalized into typed `CommandRequest.args.options` and `CommandRequest.args.positionals`.
- **Runtime Isolation**: The CLI runner operates outside the production HTTP server and does not absorb domain business rules.
- **Structured Exit Codes**: All commands produce a deterministic `CliExit` code (`0` for success, non-zero for handled failures or syntax errors).

