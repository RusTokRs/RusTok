# rustok-installer-cli

`rustok-installer-cli` is the selected `rustok-cli` provider for installer
operator commands. It owns terminal-adapter mapping only and delegates plan,
preflight and seed semantics to `rustok-installer`.

## Commands

- `rustok-cli install plan` renders a validated, redacted plan without DB access.
- `rustok-cli install preflight` evaluates installer policy without mutation.
- `rustok-cli install status` reads the latest durable session through
  `rustok-installer-persistence`.
- `rustok-cli seed apply` applies the typed seed workflow through owner-owned
  database writers.

`install apply` is intentionally not exposed until the shared executor-port
layer replaces the current HTTP-host composition.
