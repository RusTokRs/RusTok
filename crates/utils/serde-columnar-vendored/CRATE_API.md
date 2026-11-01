# serde_columnar (vendored) — Crate API

Vendored fork of `serde_columnar` 0.3.14: columnar storage for lists and maps with compressible serialization.

## Public surface

- `to_vec` / `from_bytes` — columnar serialize/deserialize entry points.
- Containers: `ColumnarVec`, `ColumnarMap`.
- Strategies: RLE, DeltaRle, DeltaOfDelta, BoolRle and their encoder/decoder pairs.
- `#[columnar(...)]` — the derive macro re-exported from `serde_columnar_derive`.
- `ColumnarError`, `PostcardError`, and the `__serde_utils` support module.

## Invariants

- This is a vendored upstream fork: functional behavior must stay aligned with upstream 0.3.x; local changes are limited to workspace hygiene (docs, metadata).
- The derive macro is re-exported by name (`columnar`) only; no wildcard re-exports.
- Wire formats are versioned by the upstream crate; consumers must not mix encoder/decoder versions.
