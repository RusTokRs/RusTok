# FORUM-23B2G2A1 Search owner-revision counter hardening

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

Tests, Cargo, formatting, and CI were not run by the implementation agent; this note records source review only.

- # FORUM-23B2G2A1 Search owner-revision counter hardening
- baseline migration
- ledger revisions must form one contiguous sequence
- fails the migration
- advance the previous revision by exactly `1`
- DEFERRABLE INITIALLY DEFERRED
- direct counter-only commit
- rejects truncation of both
- does not decode or validate `sys_events.payload`
- did not run these commands
