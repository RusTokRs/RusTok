# rustok-installer-cli

`rustok-installer-cli` is the selected `rustok-cli` provider for installer
operator commands. It owns terminal-adapter mapping only and delegates plan,
preflight and seed semantics to `rustok-installer`.

## Commands

- `rustok-cli install plan` renders a validated, redacted plan without DB access.
- `rustok-cli install preflight` evaluates installer policy without mutation.
- `rustok-cli install apply` runs the typed plan through the same installer
  state machine used by HTTP, including database readiness, schema, seed,
  admin provisioning, verification, and durable receipts.
- `rustok-cli install status` reads the latest durable session through
  `rustok-installer-persistence`.
- `rustok-cli seed apply` applies the typed seed workflow through owner-owned
  database writers.

`install apply --dry-run` validates and renders preflight evidence without
mutating the target database.
