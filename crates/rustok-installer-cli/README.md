# rustok-installer-cli

`rustok-installer-cli` is the selected `rustok-cli` provider for installer
operator commands. It owns terminal-adapter mapping only and delegates plan,
preflight and seed semantics to `rustok-installer`.

## Commands

Every `install plan`, `install preflight`, and `install apply` invocation
requires an explicit `--root <path>`. Relative paths resolve against the
invocation directory; all local runtime paths are derived from that root.

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

Distributed plan, preflight, and apply additionally require a platform-signed
base-distribution receipt and its Ed25519 public key through
`--base-distribution-receipt` plus `--base-distribution-public-key`, or the
matching `RUSTOK_INSTALL_BASE_DISTRIBUTION_*` environment variables. The CLI
verifies the bounded regular file, signer-key digest, validity interval, exact
bundle identity, and executable-composition match before producing a plan.
The receipt is supply authority for a fresh target, not caller-selected module
metadata; its owner-ledger import and distributed rollout remain fail-closed
until their canonical adapters are connected.
