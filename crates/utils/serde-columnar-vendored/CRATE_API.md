# CRATE_API: serde_columnar (vendored)

## 1. Architectural Role & Boundary Enforcement

`serde_columnar` is a vendored high-performance columnar serialization library:
- **Columnar Transformation:** Converts row-oriented structures (`Vec<Row>` or `HashMap<K, V>`) into column-oriented continuous buffers prior to serialization.
- **Run-Length & Delta Compression:** Employs domain-specific encoding strategies (RLE, BoolRLE, DeltaRLE, DeltaOfDelta) to compress sequential records with repetitive or delta-linear properties.
- **Compact Postcard Wire Format:** Leverages `postcard` for zero-overhead, schema-aware binary serialization.
- **Vendored Dependency Isolation:** Isolated vendored package pinned to deterministic local compilation without external network dependencies.

---

## 2. Public Surface & Data Structures

### Primary Encoders & Decoders
- `to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>, ColumnarError>`: High-level serialization entrypoint.
- `from_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ColumnarError>`: High-level deserialization entrypoint.
- `AnyRleEncoder<T>` / `AnyRleDecoder<T>`: Generic run-length encoding for arbitrary cloneable types.
- `BoolRleEncoder` / `BoolRleDecoder`: Bit-packed run-length encoder optimized for boolean vectors.
- `DeltaRleEncoder<T>` / `DeltaRleDecoder<T>`: Delta run-length encoder for monotonically changing integer sequences.
- `DeltaOfDeltaEncoder<T>` / `DeltaOfDeltaDecoder<T>`: Second-order delta encoder for constant-velocity sequences.
- `ColumnarError`: Typed error representation wrapping serialization and encoding failures.

---

## 3. Error Contracts & Edge Case Guarantees

- `ColumnarError`:
  - `PostcardError(postcard::Error)`: Postcard encoding/decoding errors.
  - `RleError`: Run-length encoding / decoding integrity failures.
  - `ZeroLengthRow`: Attempt to serialize empty row without columnar headers.
  - `InvalidCanCopy`: Memory safety invariant violation during fast columnar copying.
- **Empty Slices & Singletons:** Encoders gracefully handle empty collections (`vec![]`) and single-element collections without overflow or division-by-zero.
- **Integer Overflow Protection:** Delta and DeltaOfDelta algorithms use wrapping or widening integer operations to prevent panic on large sequence boundaries.

---

## 4. Concurrency & Concurrency Models

- **Pure Computation:** Encoders and decoders operate synchronously over in-memory buffers without global locks or shared mutable static state.
- **Thread Safety:** All core types implement `Send` and `Sync`, allowing parallel batch compression across worker threads.

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` and authored `README.md`.
  - Replaced unlinked external `rand` dependency in `rle.rs` unit tests with deterministic `SimpleRng`.
  - Fixed doctest syntax in `lib.rs` by adding `text` code fences.
  - Authored comprehensive `contract_tests.rs` verifying RLE, DeltaRLE, BoolRLE, and error variants.
  - Verified unit and contract tests:
    - `cargo test -p serde_columnar --lib`: 9 passed, 0 failed.
  - Clippy verification: 0 warnings with `-D warnings`.

