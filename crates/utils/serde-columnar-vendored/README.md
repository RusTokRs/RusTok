# serde_columnar (vendored)

Vendored fork of [`serde_columnar` 0.3.14](https://github.com/loro-dev/columnar):
columnar storage for lists and maps with compressible serialization and
deserialization.

The fork is vendored so the platform does not depend on crates.io availability
for this encoding layer. Local changes are limited to workspace hygiene
(documentation and packaging metadata); functional behavior stays aligned with
upstream 0.3.x.

Public surface and contract invariants are documented in [CRATE_API.md](CRATE_API.md).

## Purpose

Vendored high-performance columnar serialization library for batch data processing in RusToK.

## Responsibilities

- Provide columnar storage encoding for collections (`Vec` and `Map`) with compression algorithms (RLE, BoolRLE, DeltaRLE, DeltaOfDelta).
- Optimize serialization footprint for sequential records where adjacent fields have repeated or linearly increasing values.
- Maintain backwards-compatible serialization formats powered by Postcard.

## Entry points

- `serde_columnar::to_vec`: Serializes columnar collections into compact byte arrays.
- `serde_columnar::from_bytes`: Deserializes byte arrays back into structured columnar models.
- Compression encoders: `AnyRleEncoder`, `BoolRleEncoder`, `DeltaRleEncoder`, `DeltaOfDeltaEncoder`.
- Derive macro: `#[columnar]` from `serde_columnar_derive`.
