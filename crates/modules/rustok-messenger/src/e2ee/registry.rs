use std::{
    collections::{HashMap, HashSet},
    sync::RwLock,
};
use chrono::Utc;
use uuid::Uuid;

use super::keys::{DeviceKey, OneTimePrekey, PrekeyBundle, UploadDeviceKeysInput};
use crate::error::{MessengerError, MessengerResult};

/// Maximum number of one-time prekeys allowed in a single upload batch.
pub const MAX_PREKEYS_PER_UPLOAD: usize = 100;

/// Maximum number of unconsumed one-time prekeys retained in a device pool.
pub const MAX_PREKEY_POOL_SIZE: usize = 500;

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
        if user_id.is_nil() {
            return Err(MessengerError::Validation(
                "user_id must not be nil UUID".to_string(),
            ));
        }
        let device_id = input.device_id.trim().to_string();
        if device_id.is_empty() {
            return Err(MessengerError::Validation(
                "device_id must not be empty".to_string(),
            ));
        }
        if input.identity_key.trim().is_empty() {
            return Err(MessengerError::Validation(
                "identity_key must not be empty".to_string(),
            ));
        }
        if input.curve25519_key.trim().is_empty() {
            return Err(MessengerError::Validation(
                "curve25519_key must not be empty".to_string(),
            ));
        }
        if input.signature.trim().is_empty() {
            return Err(MessengerError::Validation(
                "signature must not be empty".to_string(),
            ));
        }

        if input.one_time_prekeys.len() > MAX_PREKEYS_PER_UPLOAD {
            return Err(MessengerError::Validation(format!(
                "Cannot upload more than {MAX_PREKEYS_PER_UPLOAD} one-time prekeys in a single batch"
            )));
        }

        // Validate OTKs and check for intra-batch duplicate key_ids
        let mut batch_key_ids = HashSet::with_capacity(input.one_time_prekeys.len());
        for ot_input in &input.one_time_prekeys {
            let kid = ot_input.key_id.trim();
            if kid.is_empty() {
                return Err(MessengerError::Validation(
                    "One-time prekey key_id must not be empty".to_string(),
                ));
            }
            if ot_input.key_material.trim().is_empty() {
                return Err(MessengerError::Validation(
                    "One-time prekey key_material must not be empty".to_string(),
                ));
            }
            if !batch_key_ids.insert(kid.to_string()) {
                return Err(MessengerError::Validation(format!(
                    "Duplicate key_id in upload batch: '{kid}'"
                )));
            }
        }

        let device_key = DeviceKey {
            device_id: device_id.clone(),
            user_id,
            identity_key: input.identity_key,
            curve25519_key: input.curve25519_key,
            signature: input.signature,
            created_at: Utc::now(),
        };

        let device_lookup_key = (user_id, device_id.clone());

        {
            let mut write_guard = self.devices.write().unwrap_or_else(|e| e.into_inner());
            write_guard.insert(device_lookup_key.clone(), device_key.clone());
        }

        if !input.one_time_prekeys.is_empty() {
            let mut prekey_guard = self.prekeys.write().unwrap_or_else(|e| e.into_inner());
            let pool = prekey_guard.entry(device_lookup_key).or_default();

            // Collect existing key_ids in pool to prevent duplicates
            let existing_ids: HashSet<String> = pool.iter().map(|k| k.key_id.clone()).collect();

            // Count current unconsumed keys
            let current_unconsumed = pool.iter().filter(|k| !k.consumed).count();
            let mut available_slots = MAX_PREKEY_POOL_SIZE.saturating_sub(current_unconsumed);

            for ot_input in input.one_time_prekeys {
                if available_slots == 0 {
                    break;
                }
                let kid = ot_input.key_id.trim().to_string();
                if !existing_ids.contains(&kid) {
                    pool.push(OneTimePrekey {
                        key_id: kid,
                        device_id: device_id.clone(),
                        key_material: ot_input.key_material,
                        signature: ot_input.signature,
                        consumed: false,
                    });
                    available_slots = available_slots.saturating_sub(1);
                }
            }

            // Prune old consumed keys if pool exceeds double capacity to avoid unbounded memory growth
            if pool.len() > MAX_PREKEY_POOL_SIZE * 2 {
                let mut unconsumed: Vec<OneTimePrekey> = pool.iter().filter(|k| !k.consumed).cloned().collect();
                // Retain up to 50 most recent consumed keys for audit
                let recent_consumed: Vec<OneTimePrekey> = pool.iter().filter(|k| k.consumed).rev().take(50).cloned().collect();
                unconsumed.extend(recent_consumed);
                *pool = unconsumed;
            }
        }

        Ok(device_key)
    }

    /// Lists all registered public device identities for a user.
    pub fn list_user_devices(&self, user_id: Uuid) -> Vec<DeviceKey> {
        if user_id.is_nil() {
            return Vec::new();
        }
        let read_guard = self.devices.read().unwrap_or_else(|e| e.into_inner());
        read_guard
            .iter()
            .filter(|((u, _), _)| *u == user_id)
            .map(|(_, dev)| dev.clone())
            .collect()
    }

    /// Returns the total number of registered devices.
    pub fn total_devices_count(&self) -> usize {
        let read_guard = self.devices.read().unwrap_or_else(|e| e.into_inner());
        read_guard.len()
    }

    /// Lists all registered devices with their unconsumed OTK count for security audit.
    pub fn list_all_devices_audit(&self) -> Vec<(DeviceKey, usize)> {
        let read_devices = self.devices.read().unwrap_or_else(|e| e.into_inner());
        let read_prekeys = self.prekeys.read().unwrap_or_else(|e| e.into_inner());

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
        if target_user_id.is_nil() {
            return Err(MessengerError::Validation(
                "target_user_id must not be nil UUID".to_string(),
            ));
        }
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return Err(MessengerError::Validation(
                "device_id must not be empty".to_string(),
            ));
        }

        let lookup_key = (target_user_id, device_id.to_string());
        let device = {
            let read_guard = self.devices.read().unwrap_or_else(|e| e.into_inner());
            read_guard
                .get(&lookup_key)
                .cloned()
                .ok_or_else(|| MessengerError::NotFound(format!("Device {device_id} not found")))?
        };

        let one_time_prekey = {
            let mut prekey_guard = self.prekeys.write().unwrap_or_else(|e| e.into_inner());
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
