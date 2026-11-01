# serde_columnar (vendored)

Vendored fork of [`serde_columnar` 0.3.14](https://github.com/loro-dev/columnar):
columnar storage for lists and maps with compressible serialization and
deserialization.

The fork is vendored so the platform does not depend on crates.io availability
for this encoding layer. Local changes are limited to workspace hygiene
(documentation and packaging metadata); functional behavior stays aligned with
upstream 0.3.x.

Public surface and contract invariants are documented in [CRATE_API.md](CRATE_API.md).
