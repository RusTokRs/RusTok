use sha2::{Digest, Sha256};

pub const SHA256_DIGEST_BYTES: usize = 32;
pub const SHA256_BLOCK_BYTES: usize = 64;

/// Computes a SHA-256 digest from ordered byte chunks without allocating a
/// concatenated buffer.
pub fn sha256_digest(chunks: &[&[u8]]) -> [u8; SHA256_DIGEST_BYTES] {
    let mut hasher = Sha256::new();
    for chunk in chunks {
        hasher.update(chunk);
    }
    let digest = hasher.finalize();
    let mut output = [0_u8; SHA256_DIGEST_BYTES];
    output.copy_from_slice(&digest);
    output
}

/// Computes HMAC-SHA256 over ordered byte chunks.
///
/// The helper follows RFC 2104 key normalization with the SHA-256 64-byte block
/// size and avoids a concatenated message allocation.
pub fn hmac_sha256(key: &[u8], chunks: &[&[u8]]) -> [u8; SHA256_DIGEST_BYTES] {
    let mut key_block = [0_u8; SHA256_BLOCK_BYTES];
    if key.len() > SHA256_BLOCK_BYTES {
        let digest = sha256_digest(&[key]);
        key_block[..SHA256_DIGEST_BYTES].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0x36_u8; SHA256_BLOCK_BYTES];
    let mut outer_pad = [0x5c_u8; SHA256_BLOCK_BYTES];
    for index in 0..SHA256_BLOCK_BYTES {
        inner_pad[index] ^= key_block[index];
        outer_pad[index] ^= key_block[index];
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    for chunk in chunks {
        inner.update(chunk);
    }
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    let digest = outer.finalize();
    let mut output = [0_u8; SHA256_DIGEST_BYTES];
    output.copy_from_slice(&digest);
    output
}

/// Compares two fixed-size SHA-256 digests with the same bounded amount of
/// source-level work for equal and unequal inputs.
///
/// This helper avoids early return and length-dependent loops. It does not claim
/// a separately audited compiler or hardware side-channel guarantee.
pub fn fixed_work_sha256_eq(
    expected: &[u8; SHA256_DIGEST_BYTES],
    candidate: &[u8; SHA256_DIGEST_BYTES],
) -> bool {
    let mut difference = 0_u8;
    for index in 0..SHA256_DIGEST_BYTES {
        difference |= expected[index] ^ candidate[index];
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_accepts_ordered_chunks() {
        assert_eq!(
            sha256_digest(&[b"Bearer ", b"secret"]),
            sha256_digest(&[b"Bearer secret"]),
        );
    }

    #[test]
    fn hmac_matches_rfc_4231_case_one() {
        let key = [0x0b_u8; 20];
        assert_eq!(
            hmac_sha256(&key, &[b"Hi There"]),
            [
                0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53,
                0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b, 0xf1, 0x2b,
                0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7,
                0x26, 0xe9, 0x37, 0x6c, 0x2e, 0x32, 0xcf, 0xf7,
            ],
        );
    }

    #[test]
    fn fixed_work_comparison_matches_equal_digests_only() {
        let expected = sha256_digest(&[b"expected"]);
        let same = sha256_digest(&[b"expected"]);
        let different = sha256_digest(&[b"different"]);

        assert!(fixed_work_sha256_eq(&expected, &same));
        assert!(!fixed_work_sha256_eq(&expected, &different));
    }
}
