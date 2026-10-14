use serde::{Deserialize, Serialize};

/// Supported Zero-Knowledge cryptographic algorithms for end-to-end encryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum E2eeEncryptionAlgo {
    /// 1-on-1 pairwise Double Ratchet (Olm v1) over Curve25519, AES-256-CBC, and HMAC-SHA-256.
    #[serde(rename = "m.olm.v1.curve25519-aes-sha2")]
    OlmV1Curve25519AesSha2,
    /// High-throughput group ratchet (Megolm v1) over AES-256-CTR and HMAC-SHA-256.
    #[serde(rename = "m.megolm.v1.aes-sha2")]
    MegolmV1AesSha2,
}

impl E2eeEncryptionAlgo {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OlmV1Curve25519AesSha2 => "m.olm.v1.curve25519-aes-sha2",
            Self::MegolmV1AesSha2 => "m.megolm.v1.aes-sha2",
        }
    }
}

impl std::str::FromStr for E2eeEncryptionAlgo {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "m.olm.v1.curve25519-aes-sha2" => Ok(Self::OlmV1Curve25519AesSha2),
            "m.megolm.v1.aes-sha2" => Ok(Self::MegolmV1AesSha2),
            other => Err(format!("Unsupported E2EE encryption algorithm: {other}")),
        }
    }
}

/// Opaque zero-knowledge ciphertext payload transferred across server without decryption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedPayload {
    /// Ratchet algorithm used to construct this ciphertext.
    pub algorithm: E2eeEncryptionAlgo,
    /// Base64-encoded public identity/ephemeral key of the sender.
    pub sender_key: String,
    /// Recipient public key (present for 1-on-1 Olm sessions).
    pub recipient_key: Option<String>,
    /// Session identifier (Megolm group session ID or Olm session ID).
    pub session_id: Option<String>,
    /// Opaque base64-encoded ciphertext blob.
    pub ciphertext: String,
    /// Message authentication code (MAC) tag for integrity verification.
    pub mac: Option<String>,
    /// Monotonic sequence index inside the ratchet stream.
    pub message_index: Option<u32>,
}

impl EncryptedPayload {
    /// Validates basic structural invariants of the opaque ciphertext.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.sender_key.is_empty() {
            return Err("Sender key must not be empty");
        }
        if self.ciphertext.is_empty() {
            return Err("Ciphertext must not be empty");
        }
        match self.algorithm {
            E2eeEncryptionAlgo::OlmV1Curve25519AesSha2 => {
                if self.recipient_key.is_none() {
                    return Err("Olm 1-on-1 ciphertext must designate recipient_key");
                }
            }
            E2eeEncryptionAlgo::MegolmV1AesSha2 => {
                if self.session_id.is_none() {
                    return Err("Megolm group ciphertext must designate session_id");
                }
                if self.message_index.is_none() {
                    return Err("Megolm group ciphertext must include message_index");
                }
            }
        }
        Ok(())
    }
}
