# Host-global operator authority

Host-global operational resources are not owned by a tenant. Tenant roles,
`settings:*`/`logs:*` permissions, OAuth applications, OAuth scopes, app
metadata, default tenants and magic UUIDs never imply authority over these
resources.

## Credential contract

The server accepts a dedicated opaque credential in the HTTP header:

```text
X-RusTok-Host-Token: <high-entropy-token>
```

The raw token belongs in the caller's secret manager. The server configuration
stores only SHA-256 digests in `RUSTOK_HOST_AUTHORITY_CREDENTIALS`:

```json
[
  {
    "actor_id": "7c2db327-8e92-4b2d-b65b-e19234c54755",
    "authority": "manage",
    "token_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  }
]
```

The environment value is the compact JSON representation of that array. Each
entry has the following contract:

- `actor_id` is a non-nil operator/audit identity owned by the deployment, not a
  tenant user or OAuth app id;
- `authority` is exactly `read` or `manage`;
- `token_sha256` is the 64-character hexadecimal SHA-256 digest of a token with
  at least 32 bytes of entropy;
- token hashes are unique; at most 64 credentials are accepted.

Generate a token and digest with a secret-management tool. One example on a
trusted operator workstation is:

```sh
TOKEN="$(openssl rand -base64 48 | tr -d '\n')"
printf '%s' "$TOKEN" | sha256sum
```

Do not put the raw token in repository files, YAML committed to source control,
logs, issue comments, GraphQL variables, URLs or browser storage.

## Admission and transport behavior

- Axum middleware validates the dedicated header independently from ordinary
  tenant authentication and inserts a typed `HostAuthorityContext` for native
  `#[server]` transports.
- HTTP GraphQL validates the same header from request data before resolving a
  host-global resource.
- GraphQL WebSocket connection data intentionally does not retain host
  authority. Host-global queries and mutations are denied over WebSocket.
- `read` can inspect System health/cache/events and global delivery/Iggy
  configuration; `manage` includes reads and can update global delivery/Iggy
  configuration.
- Iggy connector mutation also requires ordinary authenticated tenant context
  matching the routed tenant because encrypted connector secrets are still
  stored under that tenant owner. The mutation audit actor remains the host
  operator from the dedicated credential.
- Invalid credentials receive a static denial. Invalid credential
  configuration is logged without the presented token and returns a static
  internal error only when a host credential is presented or a host-global
  GraphQL operation is attempted.

## Rotation and revocation

Use overlapping entries for zero-downtime rotation:

1. Generate a new high-entropy token and digest.
2. Add the new digest with the same `actor_id` and intended authority while the
   old digest remains configured.
3. Roll the configuration to every replica.
4. Move callers to the new raw token and verify representative HTTP GraphQL and
   native operations.
5. Remove the old digest and roll all replicas again.

Revocation is removal of the digest followed by rollout to every replica. Host
authority is not embedded in JWT or refresh state, so there is no token claim to
wait out. Multi-replica rollout must keep the credential array consistent; a
partial rollout can produce replica-dependent admission.

## Required verification

```sh
node scripts/verify/verify-host-global-authority-boundary.mjs
cargo test -p rustok-api host_authority -- --nocapture
cargo test -p rustok-server host_authority --lib -- --nocapture
cargo check -p rustok-events-module
cargo check -p rustok-server --lib
```

Live evidence must cover:

- no header, wrong token, short token and malformed configuration denial;
- `read` admitted for reads and denied for writes;
- `manage` admitted for reads and writes with the configured audit actor;
- ordinary tenant admin and tenant OAuth credentials denied without the
  dedicated header;
- Iggy mutation denied without matching ordinary tenant authentication;
- old/new overlap during rotation and old-token denial after removal;
- parity across replicas after rollout;
- WebSocket denial for host-global operations.
