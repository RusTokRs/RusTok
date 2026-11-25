# JWT algorithm migration

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [JWT purpose keyrings](2026-11-25-jwt-purpose-keyrings.md), [durable JWT generation fence](2026-11-25-jwt-generation-fence.md), [rolling replica admission](2026-11-25-jwt-rolling-admission.md) and [JWT rollback witness](2026-11-25-jwt-rollback-witness.md)
- Supersedes: the previous prohibition on active algorithm changes in durable admission.

## Decision

Each purpose has exactly one active signer and one active algorithm (HS256
or RS256). Each bounded historical verification entry may explicitly specify
`algorithm`; omission inherits the ring's active algorithm for compatibility.
The trusted token purpose and exact kid select the prepared decoding key,
algorithm and cutoff. The JOSE alg must equal the selected algorithm; it
cannot select an algorithm, a different key or another purpose. Legacy
missing-kid access maps to one explicit key and its algorithm and cutoff.
RS256 public material must never be configured as HMAC material: PEM-shaped
symmetric material is rejected, but arbitrary public bytes cannot reliably
be identified as public; operators must use independent high-entropy secrets.
Production checks all explicit HMAC history, even in an RSA-active ring.
HMAC verification material remains capable of signing.

The canonical key identities remain algorithm-separated. The metadata stores
the algorithm override only when different from the active ring algorithm.
An old homogeneous ledger or grant therefore deserializes and serializes in
the same representation. No new application migration, MAC format or witness
schema is required. Exact effective algorithm, reuse identity and verification
identity must match across generations. Once demoted, a previously active
key cannot sign again; removed IDs and reused material remain prohibited.
A planned return to HS256 uses a new staged secret, not an old signer.

## Managed migration

Upgrade all server and operator binaries before introducing mixed history.
Older binaries cannot interpret mixed metadata and must not remain in service.
Keep the stable host MAC root and the pinned authority/witness unchanged.
Starting from an enrolled homogeneous generation:

1. Publish a higher generation with the old active signer unchanged and the
   new key staged as a verifier with its explicit algorithm and finite cutoff.
2. Wait until every live replica grant can verify the staged signer; publish
   a higher generation with the new algorithm and staged kid as the active
   signer, retaining the old signer as historical material with an explicit
   algorithm and cutoff covering overlap and remaining outgoing JWT TTL.
3. The durable fence checks the new signer was staged in the immediately
   preceding committed generation, the staging cutoff has not expired, its
   exact identity matches, and the outgoing signer remains as a live verifier.
   A revoked outgoing signer must instead be absent; the existing drain
   protocol applies. All lifetime identity, revocation, deadline and legacy
   mapping checks remain in force. A formerly active or pre-enrollment
   historical key cannot be promoted.
4. Compatible rolling promotion checks both algorithms per key and waits for
   incompatible older grants. If the candidate is incompatible with the
   current generation's accepted verifier set or overlap, the existing
   conservative drain path applies; continuous availability is not guaranteed
   for such a candidate. Operators must budget sufficient cutoffs and preserve
   all still-needed verification keys.
5. Remove retired historical keys in another higher generation after all
   corresponding tokens have expired. Never restore an old signer or extend
   a historical cutoff.

A fixed AuthConfig can verify mixed history. Unmanaged process-local
publication still refuses active algorithm changes; managed publication is
required to enforce durable tombstones and distributed overlap. Static
generation zero is not a migration authority. No new algorithms beyond
HS256 and RS256 are enabled.

## Verification and limits

Source regressions exercise forward and reverse staged migration, waiting
for older replica grants, cross-algorithm header substitution, exact legacy
selection, homogeneous metadata compatibility, HMAC material validation,
witness persistence and revocation refusal. No runtime/compile evidence
has been obtained. Run scoped Auth and server tests and independent
PostgreSQL replica/witness rollout tests before production. Keyring deployment
and token TTL/cutoff budgeting are operator actions. Root MAC rotation and
session/refresh/OAuth/RBAC database recovery remain separate.
