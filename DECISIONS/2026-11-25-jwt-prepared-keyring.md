# Immutable prepared JWT keyring snapshots

- Status: Accepted
- Implementation status: In progress (source implementation; scoped checks pending)
- Extends [JWT purpose keyrings](2026-11-25-jwt-purpose-keyrings.md).

## Decision

Auth compiles all four purpose keyrings as a single immutable snapshot before host startup succeeds. The snapshot owns prepared encoding and decoding keys, active identifiers and indexed verification-only history. All algorithms, key pairs, cross-purpose identities and migration settings are validated before publication. History never contains an encoding key. Each operation selects one purpose and one key; time-based retirement is checked at use and again after verification. No PEM parsing, HKDF derivation or cross-ring key identity scan is performed by the Auth request path.

Cryptographic configuration fields are private. A shared immutable key state owns both raw key inputs and a OnceLock containing either the complete prepared snapshot or its construction error. Cloning a configuration shares the raw secret material and initialization result; cryptographic edits use a single copy-on-write boundary. The host builder initializes eagerly; direct constructors used by embedders initialize on first use. Changes through the RSA builder or settings override detach the unpublished candidate from the old cache before mutation. A failed override never publishes a partial configuration; existing clones retain their original snapshot. Non-key policy fields do not participate in key preparation.

The existing JWT library still builds backend signer/verifier objects internally when signing or decoding. This change does not replace its audited signature and claim validation or assert zero per-request crypto allocation. This original fixed-snapshot decision is extended by [process-local JWT rotation](2026-11-25-jwt-hot-rotation.md); no process-global cache is introduced.

## Verification

Source tests cover shared first initialization, concurrent access, invalid candidate rollback, independent rotation and exclusive cutoffs. Run scoped Auth and server checks before deployment. Wire format, configuration JSON, persisted sessions and migration policy are unchanged.
