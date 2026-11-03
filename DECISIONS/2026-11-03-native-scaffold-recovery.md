# Native scaffolding uses workspace locking and recoverable writes

- Date: 2026-11-03
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform tooling (`xtask`)
- Extends: [Canonical native module source layout](./2026-09-18-canonical-native-module-source-layout.md)
- Supersedes: None
- Superseded by: None

## Context

Native generation changes one module tree and shared workspace, host and
documentation registrations. In-memory undo cannot survive process termination;
different module slugs still compete for the same manifests. A partially
registered Cargo workspace can prevent `cargo xtask` from starting recovery.

## Decision

Serialize live scaffold operations in each workspace and retain sufficient state
to recover after process interruption before reporting a successful commit.

## Sources of truth and ownership

`xtask/src/scaffold` owns the complete protocol. It uses the standard-library
file locking API already supported by the workspace MSRV, existing serde/JSON
and hashing dependencies, and one ignored workspace-local `.rustok/scaffold/`
directory. Recovery state must survive `cargo clean` and must not enter source
control. No domain database, tenant, event or UI contract is added.

## Invariants

One permanent `lock` file is opened read/write and exclusively try-locked before
live recovery or planning. The same handle remains alive through writes,
validation, commit or rollback. Contention fails immediately and actionably;
different slugs share the lock, different workspaces do not. Never delete the
lock file to unlock or infer ownership from a PID. Process termination releases
the OS lock. Other tools/editors must not mutate the workspace concurrently.

Dry-run creates no control files and performs no recovery. It shared-locks an
existing lock file when present and refuses pending recovery. Without an existing
control directory it remains a read-only preview, not a reserved apply plan;
every live invocation builds and checks a fresh plan under the exclusive lock.

## Data, transaction, and concurrency boundary

1. Validate exact original bytes, paths and the complete registration plan.
2. Retain a copy of the running CLI as `recover.exe` (`recover` on Unix) outside
   `target`, so recovery does not depend on resolving interrupted Cargo manifests.
3. Create `preparing/` containing an immutable `plan.json`, complete synced
   intended payloads and original backups. No workspace content changes yet.
4. Publish that directory as `active/` before the first workspace mutation.
5. Exclusively claim the module directory and persist its claim before creating
   descendants. Publish new source files with no-clobber hard links to complete
   staged payloads. Publish existing-file updates with same-filesystem rename
   after checking the expected original and preserving permission bits.
6. Run existing lint/metadata validation while still holding the lock.
7. Atomically publish a synced `committed` marker bound to the immutable plan's
   digest before reporting success. A committed operation is never undone.
8. Move completed state to `retired/` before cleaning its private payloads.
   Interrupted preparation/retirement cleanup never drives workspace mutations.

Supported local filesystems must support the lock, hard-link and rename
operations. Unsupported operations fail with retained recovery evidence; there
is no copy/truncate fallback that exposes partial target files. Read-only update
targets fail before mutation. The protocol does not promise ACL/xattr cloning.

## Context dimensions

The canonical workspace path binds the journal to the checkout. The module slug
binds generated paths to one module root. Tenant, locale and user context do not
participate in filesystem recovery.

## Events and projections

The command reports the recovery outcome directly. The manifest and document
registration entries are the existing owner projections. Recovery adds no event
stream or separate registry.

## Failure semantics

The next live invocation recovers first; `scaffold --recover` also runs recovery
without requesting a module. The retained CLI supports the same command even
when Cargo cannot resolve the interrupted workspace. Do not delete recovery
state to bypass an error.

For an uncommitted journal, validate its canonical workspace identity, bounded
size, unique normal relative paths, exact registration allowlist, module root,
backups and absence of symlink/reparse traversal before any recovery write.
Already-original files and already-absent created files are idempotent successes.
Only exact intended contents may be reverted; other contents, missing originals,
foreign files and ambiguous directory claims are preserved and block completion.
Restore via a complete synced temporary copy and rename, leaving the immutable
backup intact so interrupted recovery can repeat safely. Remove only planned
created files and empty owned directories, never a recursive module deletion.

A crash between directory creation and its persisted claim leaves an ambiguous
directory: preserve it, retain the journal and require explicit reconciliation.
Invalid/truncated journals or commit markers fail closed. Originals remain in
the journal/backups until rollback or committed retirement succeeds. Error
messages identify the affected paths and recovery entrypoint; no success message
may precede durable commit.

## Non-goals

This contract covers interruption of cooperating processes on a supported local
filesystem. It does not establish power-loss/host-crash durability, universal
directory-entry persistence, network-filesystem semantics, or isolation from
noncooperating writers replacing files/ancestors between checks and operations.
Those require separate filesystem/platform evidence and, if expanded, a contract
update. Source manifests and generated business readiness remain separate.

## Migration and cutover

The first live generation initializes the local control directory. Dry-run keeps
its zero-write contract. Journals record their format version and unknown
versions require a matching recovery runner. Existing generated modules and
tracked manifests require no migration.

## Alternatives considered

Rejected alternatives: PID/create-new sentinel locks (stale ownership), state in
`target` (lost on clean), in-place target writes (ambiguous partial contents),
automatic overwriting of conflicts, recursive module cleanup, a second recovery
implementation in shell/Python, and silently treating source checks as runtime
or cross-OS execution evidence.

## Consequences

Live scaffolds for different slugs in one workspace serialize because they share
registration targets. Preparation requires temporary storage for complete
payloads and original files. If automatic recovery detects a conflict, an operator
must reconcile it before resuming live generation.

## Verification

Use scoped `xtask` tests and real child processes: contention across slugs,
independent roots, killed owner and lock reuse; interrupted preparation/apply,
validation, rollback, commit and retirement; repeated recovery; staged partial
writes; corrupt/unsafe journals; foreign edits and files; and zero-write preview.
Run canonical architecture/ADR/remediation checks. Retain Windows and Linux
results separately; do not promote unexecuted platform or power-loss evidence.
