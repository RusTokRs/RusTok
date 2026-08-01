use sha2::{Digest, Sha256};

pub const SHA256_DIGEST_BYTES: usize = 32;

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
    fn fixed_work_comparison_matches_equal_digests_only() {
        let expected = sha256_digest(&[b"expected"]);
        let same = sha256_digest(&[b"expected"]);
        let different = sha256_digest(&[b"different"]);

        assert!(fixed_work_sha256_eq(&expected, &same));
        assert!(!fixed_work_sha256_eq(&expected, &different));
    }
}
