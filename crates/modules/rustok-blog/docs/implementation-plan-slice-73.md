# rustok-blog implementation plan — implementation-plan-slice-73.md

Status: source_verified_no_compile / source-ready / maintainer execution pending.
Compile policy: not_run_by_request. Runtime status: not_run.
Tests, Cargo, formatting, and CI were not run by the implementation agent; no runtime result is claimed.

## Retained source contract


# rustok-blog implementation plan — slice 73 continuation
- source fragment: ## Slice 73 — signed user delegation for Comments TCP writes
- source fragment: `delegated_hmac_sha256`
- source fragment: SHA-256 of the complete serialized `CommentsThreadRequest`
- source fragment: process-local replay cache
- source fragment: hard-bounded to 1..=65536
- source fragment: cluster-wide, multi-process, multi-replica, or durable replay prevention
- source fragment: Plaintext credential modes remain loopback-only
- source fragment: Status: `source_verified_no_compile`.
- source fragment: Compile policy: `not_run_by_request`.
- source fragment: Runtime status: `not_run`.
- intentionally not run in this slice
