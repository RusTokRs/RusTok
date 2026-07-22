# Build publication foundation

This support crate is shared by the untrusted module-build publication path and
the trusted static-distribution evidence publisher. It prevents credential
broker and Cosign process handling from diverging between workers.

The credential broker executable and Cosign executable are absolute,
non-symlink regular files bound to lowercase SHA-256 digests. Their bytes are
re-hashed at construction, readiness, and every invocation. Neither executable
is selected by a build request.

The broker receives one bounded JSON request with contract
`rustok.registry_credential.request`, registry, repository, and minimum lease
duration. It returns only `rustok.registry_credential.response` for the same
destination, a bounded username/password pair, and an expiry no more than
fifteen minutes in the future. Unknown fields, cross-repository responses,
expired leases, oversized output, command failure, and timeout fail closed.
The broker environment and stderr are discarded.

Cosign accepts only approved cloud/HSM KMS URI schemes. Registry credentials
are materialized into a private job-scoped Docker configuration, passed through
an otherwise cleared environment, and removed after the command. The shared
contract has no raw-key, environment-secret, alternate executable, or
generation-suffixed path.

Target verification includes broker framing, expiry/repository rejection,
program mutation, KMS-reference rejection, bounded command behavior, and
private-config cleanup. During the current shared-worktree implementation only
the explicitly allowed formatting, diff, and metadata checks are run.
