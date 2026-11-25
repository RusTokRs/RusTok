# JWT purpose keyrings and cryptographic separation

- Status: Accepted
- Implementation status: In progress (source implementation; runtime verification pending)
- Extends and supersedes the shared-key scope of [JWT keyring](2026-11-25-jwt-keyring.md).

## Decision

Auth owns four independent rings: direct/OAuth access, password reset, email verification and invite. The trusted encode/decode entry point selects the ring, configured algorithm and JWT type; neither an untrusted header nor a claim can switch purposes. Access retains the existing JWT type for the bounded migration, while special JWTs carry distinct types and mandatory key IDs. Each ring has one active key, bounded history and exclusive retirement deadlines.

Access retains the existing algorithm and configuration entry point. Each internal-link ring independently supports HS256 (default) or RS256. Unconfigured HS256 keys derive distinct signing material from the host MAC root with HKDF-SHA256 and closed purpose labels. Explicit independent secrets or RSA pairs enable independent rotation. No active key may be the raw root; startup and the selected-key use boundary reject duplicate effective HMAC or RSA verification material across active and historical rings, including reuse under different IDs. The raw historical root can only be an explicitly configured, time-bounded access verification key. Host production policy covers the root and every explicitly configured symmetric key even when access uses RSA. Root compromise is not isolated by derived keys; independent deployment secrets provide stronger compartmentalization.

## Cutover

Existing explicitly configured independent access keys and RSA access keys can retain their IDs and material. For old default HS256 access signatures, set a new active ID, move the old raw key into access verification history with a cutoff, and optionally retain the existing bounded missing-ID policy. Do not use that root as an active JWT key. Old shared-key invite/reset/verification links are deliberately rejected rather than accepting a cross-purpose bridge; reissue outstanding links. Refresh/session persistence and server-side revocation authority do not change. Deploy the cutover as a coordinated maintenance rollout; old replicas cannot consume new purpose types. Later rotations can stage verification material independently per ring. Never reintroduce retired material or reuse identifiers.

## Verification

Source regressions cover independent rotation, cross-signature rejection with correct headers/claims, wrong type, missing ID, explicit access-only migration, duplicate active/historical material and an independent RSA special ring. Compilation and coordinated deployment remain pending.
