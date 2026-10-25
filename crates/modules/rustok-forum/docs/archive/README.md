# Forum plan archive

Two snapshots are kept here.

`implementation-plan-2026-08-06.snapshot` is an exact byte-for-byte snapshot of
the former canonical Forum plan before the shared-capability ownership
correction.

`forum-23-search-index-integration.snapshot` is the delivered-history half of the
former `FORUM-23` section (the `FORUM-23B*` slice notes and the
`FORUM-23B2G2B3*` aggregate boundary), moved out of the canonical plan on
2026-10-25. It still names the retired Forum-only storefront executor and
projector, which is why it was not left inline.

Both are retained only for audit and historical task-slice detail. Their prose
describes mechanisms that no longer exist; those statements are historical and
must not be followed.

The only authoritative Forum roadmap is:

```text
crates/modules/rustok-forum/docs/implementation-plan.md
```

Do not update the snapshot, copy status from it, or add new execution work to
this directory.
