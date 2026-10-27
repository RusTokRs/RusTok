pub mod envelope;
pub mod keys;
pub mod registry;

pub use envelope::{E2eeEncryptionAlgo, EncryptedPayload};
pub use keys::{
    DeviceKey, OneTimePrekey, OneTimePrekeyInput, PrekeyBundle, UploadDeviceKeysInput,
};
pub use registry::DeviceKeyRegistry;

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn encrypted_payload_structural_validation() {
        let olm_payload = EncryptedPayload {
            algorithm: E2eeEncryptionAlgo::OlmV1Curve25519AesSha2,
            sender_key: "curve25519_alice_key".into(),
            recipient_key: Some("curve25519_bob_key".into()),
            session_id: None,
            ciphertext: "opaque_base64_ciphertext_blob".into(),
            mac: Some("hmac_sha256_tag".into()),
            message_index: None,
        };

        assert!(olm_payload.validate().is_ok());

        // Olm without recipient_key must fail
        let mut invalid_olm = olm_payload.clone();
        invalid_olm.recipient_key = None;
        assert!(invalid_olm.validate().is_err());

        // Megolm payload validation
        let megolm_payload = EncryptedPayload {
            algorithm: E2eeEncryptionAlgo::MegolmV1AesSha2,
            sender_key: "ed25519_alice_identity".into(),
            recipient_key: None,
            session_id: Some("megolm_room_session_xyz".into()),
            ciphertext: "opaque_group_ciphertext".into(),
            mac: Some("mac_tag".into()),
            message_index: Some(42),
        };

        assert!(megolm_payload.validate().is_ok());

        // Megolm without session_id or message_index must fail
        let mut invalid_megolm = megolm_payload.clone();
        invalid_megolm.message_index = None;
        assert!(invalid_megolm.validate().is_err());
    }

    #[test]
    fn device_registration_and_atomic_otk_consumption() {
        let registry = DeviceKeyRegistry::new();
        let user_id = Uuid::now_v7();
        let device_id = "device_ios_primary";

        let input = UploadDeviceKeysInput {
            device_id: device_id.to_string(),
            identity_key: "ed25519_pub_key_base64".into(),
            curve25519_key: "curve25519_prekey_base64".into(),
            signature: "signature_base64".into(),
            one_time_prekeys: vec![
                OneTimePrekeyInput {
                    key_id: "otk_1".into(),
                    key_material: "material_1".into(),
                    signature: None,
                },
                OneTimePrekeyInput {
                    key_id: "otk_2".into(),
                    key_material: "material_2".into(),
                    signature: None,
                },
            ],
        };

        let registered = registry.register_device_keys(user_id, input).unwrap();
        assert_eq!(registered.device_id, device_id);

        let devices = registry.list_user_devices(user_id);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].identity_key, "ed25519_pub_key_base64");

        // 1st claim consumes otk_1
        let bundle1 = registry.claim_prekey_bundle(user_id, device_id).unwrap();
        assert!(bundle1.one_time_prekey.is_some());
        let otk1 = bundle1.one_time_prekey.unwrap();
        assert_eq!(otk1.key_id, "otk_1");
        assert!(otk1.consumed);

        // 2nd claim consumes otk_2
        let bundle2 = registry.claim_prekey_bundle(user_id, device_id).unwrap();
        assert!(bundle2.one_time_prekey.is_some());
        let otk2 = bundle2.one_time_prekey.unwrap();
        assert_eq!(otk2.key_id, "otk_2");
        assert!(otk2.consumed);

        // 3rd claim: all OTKs are consumed, bundle returns None for otk
        let bundle3 = registry.claim_prekey_bundle(user_id, device_id).unwrap();
        assert!(bundle3.one_time_prekey.is_none());
        assert_eq!(bundle3.identity_key, "ed25519_pub_key_base64");
    }
}
