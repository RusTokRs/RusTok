# Build source materialization

This support crate is shared by the untrusted module-build worker and the
trusted static-distribution CI launcher. It exists so both paths enforce one
archive parser and one immutable CAS identity contract.

Only lowercase `sha256:<hex>` digests and exact `cas://sha256:<hex>` references
are accepted. The fixed source root contains `<hex>.tar` regular files. Archive
symlinks, digest mismatch, non-USTAR formats, invalid checksums, absolute or
parent/current-directory paths, links, devices, duplicate entries, overwrites,
truncated payloads, malformed padding, and non-zero trailing content fail
closed. Archive bytes, extracted bytes, and entry count are independently
bounded by the caller.

The destination must be a new absolute child chosen by the caller under its
own verified workspace. This crate never removes a pre-existing destination;
it removes only a directory it created during the failed call.

Target verification includes strict archive fixtures and both worker
integrations. During the current shared-worktree implementation only the
explicitly allowed formatting, diff, and metadata checks are run.
