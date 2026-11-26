# Proof-bound direct session refresh recovery

- Date: 2026-11-26
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform / auth
- Supersedes: [Direct session refresh history](./2026-11-25-session-refresh-history.md)
- Superseded by: None

## Context and decision

Hash-only rotation history detects theft but cannot recover a committed
successor when the response is lost. Returning that successor to anyone
presenting the previous bearer token would turn the concurrency grace into
an additional theft window. Direct first-party clients may supply the secret
`x-rustok-refresh-retry-key` header: exactly one 64-character lowercase hex
encoding of a nonzero 256-bit secret, independently CSPRNG-generated
or pseudorandomly derived from an independently CSPRNG-generated client
seed using a purpose-separated PRF, available before the attempt. The client must never reuse the key for a
different rotation or send it to a different service. It is not the public
Idempotency-Key header and must never be logged.

Auth derives a 256-bit opaque successor using HMAC-SHA256 with the existing
HKDF-SHA256 independent host MAC root and a new closed purpose,
`session-refresh-retry`. The fixed 64-byte payload is tenant UUID bytes,
session UUID bytes, then the decoded retry key. The initial login credential
and rotations without a retry key remain OS-CSPRNG-generated. The latter
are deliberately non-recoverable, not a fallback for an incorrect proof.
JWT signing and verification remain exclusively RS256.

## Authority and permitted transitions

The session row is the tenant, subject, current digest, revocation and expiry
authority. The immutable session-owned history contains the outgoing digest
and its rotation timestamp, linked with delete cascade. No new persistence,
raw credential, raw proof, encrypted credential or per-process result cache
is required. The previous-token projection remains for last-rotation lookup
and logout, not independent replay authority.

Under the tenant-scoped user-before-session locks, revalidate the current
subject, email-verification policy, active session and original absolute
90-day cap. On rotation, record the outgoing digest and replace the current
digest in one transaction. Never reissue a candidate digest already present
in history.

A replay of a historical digest outside its exclusive 30-second grace
(including missing or future timestamps) revokes the live lineage durably.
Within grace, recovery is allowed only when the presented digest is the
immediately preceding one and the derived successor digest equals the
currently persisted digest. All other historical requests conflict without
revoking a live session during grace. The same operation may also present
the current credential when a Set-Cookie was applied before the response
body was lost; the matching derived digest and last history timestamp
recognize it during the same window. Outside that window a matching current
credential with its already-used key conflicts, without revoking the valid
session. Revoked or expired sessions never recover. Unknown credentials and
cross-tenant lookups fail closed. Store and issuance failures propagate.

Recovery returns the identical refresh credential, without a second history
insert, sliding expiry extension or change to rotation time. It signs a
fresh access JWT with the currently authorized role and permissions, rather
than reproducing stale access claims or the old signing key. This is
idempotent session state, not byte-identical HTTP response replay.

Raw REST refresh and GraphQL HTTP refresh mutations accept the same
optional secret header. A malformed or repeated header is a bad request,
not permission to rotate without proof. The cookie endpoint derives its
proof from its Auth-issued envelope and rejects a caller-supplied proof;
it retains the existing CSRF header requirement. Unrecoverable concurrency conflicts remain
REST 409 and GraphQL CONFLICT rather than an authentication failure. No
OAuth token-family semantics are changed.

## Concurrency, deployment and limitations

All replicas must share the same validated independent host MAC root and
use the same owner database. Deploy the host changes together; an earlier
randomly generated successor cannot be reconstructed and still conflicts.
Retain the existing history migration and backfill; overwritten
pre-migration history cannot be recreated. No database migration is added.
Do not rotate the independent host root across in-flight recovery windows;
root rotation and session-store rollback remain separately governed work.

Clients must retain the secret and presented credential (or cookie) until
the response is known, retry only the same operation during grace, and
discard the proof after resolution. A new operation uses a fresh key.
For the cookie-session REST transport, Auth issues one canonical HttpOnly
`token.seed` envelope. Login and registration sample the independent
256-bit seed before mutation. Each cookie refresh derives an HMAC-SHA256
proof under that seed over the fixed purpose
`rustok/auth/cookie-refresh-retry` followed by NUL, tenant UUID bytes, and
the fixed-width lowercase hex presented token. The same envelope produces
the same proof on every replica; only the token component rotates. No
secret is exposed to browser JavaScript or a JSON response. This supersedes
Next Admin's BFF-local seed/proof calculation: the BFF now relays the exact
Auth envelope unchanged, verifies seed continuity and retries one ambiguous
upstream outcome. Cookie logout validates the complete envelope, then
revokes using the bare token; only actual absence counts as no cookie.
Cookie mutation responses use no-store. Bare cookie credentials are
rejected and require coordinated revocation and fresh sign-in.

The direct Leptos CSR transport uses the browser's origin-wide exclusive
Web Lock for login/register/refresh/logout, including the complete response
body and at most one ambiguous refresh retry. Lock admission is bounded
by a 25-second AbortSignal, network operations by a 10-second AbortSignal.
The browser must support secure-context Web Locks and AbortSignal.timeout;
otherwise mutation fails before network I/O. Fetch uses same-origin mode
and credentials, no-store, and redirect:error. If the original cookie
remains installed, a retry within grace recovers the same successor; if
the new cookie was applied already, a fresh proof is derived for that
generation. This does not serialize unrelated scripts, other clients,
SSR navigation or arbitrary delayed delivery after a document is destroyed.
Cross-tab provider projection invalidation and stale intent admission
remain separate from the network fence.

The Leptos native/SSR BFF uses the same strategy with its own purpose
`rustok/leptos-bff/refresh-retry` followed by NUL, the tenant bytes, NUL,
and the fixed-width hex bearer. Its typed, non-serializable envelope is
`tenant.token.seed`; a new login generates the seed before I/O. The BFF
marks the proof sensitive and forbids redirects. Admin SSR and native auth
functions share the same exchange; middleware does not exchange before
an Auth server function or override its cookie mutation. Access expiry
must not initiate refresh-session revocation.
The seed is transmitted only in the protected HttpOnly cookie envelope
and trusted server-side memory, never in client-readable storage or logs.
A stolen bearer token alone cannot recover the committed successor; theft of
both bearer and proof is still credential compromise. This is not
sender-constrained authentication. TLS, client storage discipline, avoiding
header logging and normal credential revocation remain required. Client
integration across standalone Storefront SSR host composition, Next
Storefront and mobile/other native paths, mounted browser/BFF/cross-replica
verification, generated API header
documentation, and failure-injection verification remain pending. Without a proof-bearing
client, a lost response is not recoverable.

## Verification

Source regressions cover canonical singleton admission, domain and session
separation, lost response with old/new cookie, wrong/missing proof,
unchanged persisted deadline/history, key reuse, and post-grace revocation.
Run scoped Auth and server checks, mounted REST and GraphQL request/error
mapping, PostgreSQL concurrency and history retention, cross-replica
lost-response injection, client storage/header redaction and API reference
verification. Compilation and runtime checks are not yet executed.
