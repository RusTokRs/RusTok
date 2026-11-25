# JWT purpose keyrings and cryptographic separation

- Status: Accepted
- Implementation status: In progress (source implementation; runtime verification pending)
- Extends and supersedes the shared-key scope of [JWT keyring](2026-11-25-jwt-keyring.md).

## Decision

Auth owns four independent rings: direct/OAuth access, password reset, email verification and invite. The trusted encode/decode entry point selects the ring and JWT type; the exact kid selects the configured verification algorithm. Neither an untrusted header nor a claim can switch purposes or algorithms. Access retains the existing JWT type for the bounded migration, while special JWTs carry distinct types and mandatory key IDs. Each ring has one active key, bounded history and exclusive retirement deadlines. The decoder accepts only the emitted JOSE fields; null and duplicate identifiers are not missing identifiers. Retirement is checked before and after signature verification.

Access retains the existing algorithm and configuration entry point. Each internal-link ring independently supports HS256 (default) or RS256. Unconfigured HS256 keys derive distinct signing material from the host MAC root with HKDF-SHA256 and closed purpose labels. Explicit independent secrets or RSA pairs enable independent rotation. No active key may be the raw root; immutable snapshot preparation rejects duplicate effective HMAC or RSA verification material across active and historical rings, including reuse under different IDs. RSA identity is the canonical modulus, including across differing PEM encodings or exponents; both public and private components are parsed by the JWT crypto backend, with a 2048-bit minimum modulus and matching pair required before publication of a signer. The raw historical root can only be an explicitly configured, time-bounded access verification key. Host production policy covers the root and every explicitly configured symmetric key even when access uses RSA. Root compromise is not isolated by derived keys; independent deployment secrets provide stronger compartmentalization.

## Cutover

Existing explicitly configured independent access keys and RSA access keys can retain their IDs and material. For old default HS256 access signatures, set a new active ID, move the old raw key into access verification history with a cutoff, and optionally retain the existing bounded missing-ID policy. Do not use that root as an active JWT key. Old shared-key invite/reset/verification links are deliberately rejected rather than accepting a cross-purpose bridge; reissue outstanding links. Refresh/session persistence and server-side revocation authority do not change. Deploy the cutover as a coordinated maintenance rollout; old replicas cannot consume new purpose types. Later rotations can stage verification material independently per ring. Never reintroduce retired material or reuse identifiers.

## Verification

Source regressions cover independent rotation, cross-signature rejection with correct headers/claims, wrong type, missing ID, explicit access-only migration, duplicate active/historical material an independent RSA special ring, and staging, promotion and retirement with two distinct RSA access key pairs. Compilation, regression execution and coordinated deployment remain pending.

The later [JWT algorithm migration](2026-11-25-jwt-algorithm-migration.md) extends
historical entries with optional per-key algorithms and permits staged managed
promotion while preserving these isolation rules.
