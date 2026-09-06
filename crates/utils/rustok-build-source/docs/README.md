# Build source materialization

This support crate is shared by authoring CLI, the untrusted module-build
worker, and the trusted static-distribution CI launcher. It exists so creation,
inspection, and worker materialization use one immutable archive contract.

`SourceArchiveBuilder` walks an absolute non-symlink source root, rejects links,
special files, source-local Cargo configuration, and a worker-owned final
descriptor, omits only root `.git` and `target` directories, sorts normalized
UTF-8 file paths, and writes a file-only USTAR archive with fixed mode,
owner/group, timestamp, checksum, padding, and terminator bytes. The output must
be a new absolute path outside the source root. The receipt contains the SHA-256
digest, archive bytes, source bytes, and file count.

`SourceTreeMaterializer` is the matching host-side ingress boundary for
reviewed data-only source files. It accepts no filesystem handles or metadata,
rejects duplicate, link-like, forbidden, root `.git`, root `target`, and
source-local Cargo configuration paths, creates exactly one new absolute tree,
and removes only that tree if a write fails. The CLI template writer uses it;
future host materializers, including Alloy Rust/WASM evolution, must use the
same boundary before creating an archive.

`SourceArchiveInspector` hashes and validates a standalone archive without
extracting it. `CasArchiveStore` additionally binds that same parser to an
exact deployment-owned `cas://sha256:<hex>` identity before materialization.

In the current implementation, `CasArchivePublisher` is the matching
archive-specific control-plane writer. It strictly
inspects the input, copies and rehashes it into a private file under the CAS
root, and exposes the final `<sha256-hex>.tar` object only through an atomic
no-replace hard-link commit. Equal concurrent uploads converge. A mismatched
upload is removed before any digest-addressed object becomes visible, and an
existing object is fully rehashed and rescanned before reuse.

Only lowercase `sha256:<hex>` digests and exact `cas://sha256:<hex>` references
are accepted. The fixed source root contains `<hex>.tar` regular files. Archive
symlinks, digest mismatch, non-USTAR formats, invalid checksums, absolute or
parent/current-directory paths, links, devices, duplicate entries, overwrites,
truncated payloads, malformed padding, and non-zero trailing content fail
closed. Archive bytes, extracted bytes, and entry count are independently
bounded by the caller.

This direct `<hex>.tar` layout is an explicit release-safety cutover gap. The
accepted target has the `rustok-modules` preparation-owned
`SourceObjectStore` publishing globally deduplicated `source_digest` blobs and
distinct owner/RLS-scoped `source_receipt_id` records over
owner/preparation/media-type/length/manifest, with one same-request idempotency
and all-reference retention authority.
`rustok-build-source` becomes its archive-specialized client while retaining
the deterministic builder, strict parser, inspector, and materializer. The
archive-specific writer/layout and every repository caller are removed or
updated atomically; they do not survive as a fallback. Reviewed Rhai releases
publish canonical bounded-workspace objects through the generic owner and are
never tar-wrapped.

The destination must be a new absolute child chosen by the caller under its
own verified workspace. This crate never removes a pre-existing destination;
it removes only a directory it created during the failed call.

Target verification includes deterministic byte-for-byte generation, strict
inspection, round-trip materialization, forbidden source paths, malformed
padding, archive resource limits, and both worker integrations. Compile/test
evidence is recorded only when its bounded command completes.
