use std::{
    collections::HashMap,
    sync::RwLock,
};
use chrono::Utc;
use uuid::Uuid;

use super::keys::{DeviceKey, OneTimePrekey, PrekeyBundle, UploadDeviceKeysInput};
use crate::error::{MessengerError, MessengerResult};

/// Thread-safe server-side repository for public device keys and one-time prekeys.
///
/// In Zero-Knowledge architecture, the server acts solely as a public key distribution
/// directory and NEVER has access to private ratchet keys or plaintext data.
#[derive(Default)]
pub struct DeviceKeyRegistry {
    devices: RwLock<HashMap<(Uuid, String), DeviceKey>>,
    prekeys: RwLock<HashMap<(Uuid, String), Vec<OneTimePrekey>>>,
}

impl DeviceKeyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or updates a device public identity and appends new one-time prekeys.
    pub fn register_device_keys(
        &self,
        user_id: Uuid,
        input: UploadDeviceKeysInput,
    ) -> MessengerResult<DeviceKey> {
        if input.device_id.trim().is_empty() {
            return Err(MessengerError::Validation(
                "device_id must not be empty".to_string(),
            ));
        }
        if input.identity_key.trim().is_empty() {
            return Err(MessengerError::Validation(
                "identity_key must not be empty".to_string(),
            ));
        }

        let device_key = DeviceKey {
            device_id: input.device_id.clone(),
            user_id,
            identity_key: input.identity_key,
            curve25519_key: input.curve25519_key,
            signature: input.signature,
            created_at: Utc::now(),
        };

        let device_lookup_key = (user_id, input.device_id.clone());

        {
            let mut write_guard = self.devices.write().unwrap();
            write_guard.insert(device_lookup_key.clone(), device_key.clone());
        }

        if !input.one_time_prekeys.is_empty() {
            let mut prekey_guard = self.prekeys.write().unwrap();
            let pool = prekey_guard.entry(device_lookup_key).or_default();
            for ot_input in input.one_time_prekeys {
                pool.push(OneTimePrekey {
                    key_id: ot_input.key_id,
                    device_id: input.device_id.clone(),
                    key_material: ot_input.key_material,
                    signature: ot_input.signature,
                    consumed: false,
                });
            }
        }

        Ok(device_key)
    }

    /// Lists all registered public device identities for a user.
    pub fn list_user_devices(&self, user_id: Uuid) -> Vec<DeviceKey> {
        let read_guard = self.devices.read().unwrap();
        read_guard
            .iter()
            .filter(|((u, _), _)| *u == user_id)
            .map(|(_, dev)| dev.clone())
            .collect()
    }

    /// Returns the total number of registered devices.
    pub fn total_devices_count(&self) -> usize {
        let read_guard = self.devices.read().unwrap();
        read_guard.len()
    }

    /// Lists all registered devices with their unconsumed OTK count for security audit.
    pub fn list_all_devices_audit(&self) -> Vec<(DeviceKey, usize)> {
        let read_devices = self.devices.read().unwrap();
        let read_prekeys = self.prekeys.read().unwrap();

        read_devices
            .values()
            .map(|dev| {
                let lookup_key = (dev.user_id, dev.device_id.clone());
                let otk_count = read_prekeys
                    .get(&lookup_key)
                    .map(|pool| pool.iter().filter(|k| !k.consumed).count())
                    .unwrap_or(0);
                (dev.clone(), otk_count)
            })
            .collect()
    }

    /// Atomically claims a prekey bundle for a target user device.
    /// If an unconsumed one-time prekey is available, it is marked as consumed.
    pub fn claim_prekey_bundle(
        &self,
        target_user_id: Uuid,
        device_id: &str,
    ) -> MessengerResult<PrekeyBundle> {
        let lookup_key = (target_user_id, device_id.to_string());
        let device = {
            let read_guard = self.devices.read().unwrap();
            read_guard
                .get(&lookup_key)
                .cloned()
                .ok_or_else(|| MessengerError::NotFound(format!("Device {device_id} not found")))?
        };

        let one_time_prekey = {
            let mut prekey_guard = self.prekeys.write().unwrap();
            if let Some(pool) = prekey_guard.get_mut(&lookup_key) {
                // Find first unconsumed OTK and atomically consume it
                if let Some(otk) = pool.iter_mut().find(|k| !k.consumed) {
                    otk.consumed = true;
                    Some(otk.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        Ok(PrekeyBundle {
            user_id: target_user_id,
            device_id: device.device_id,
            identity_key: device.identity_key,
            curve25519_key: device.curve25519_key,
            one_time_prekey,
        })
    }
}
