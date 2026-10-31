# serde_columnar (vendored)

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
