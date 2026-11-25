use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

pub trait IdGenerator {
    fn next_id(&mut self, hint: &str) -> String;
}

const MAX_GENERATED_ID_LENGTH: usize = 128;
const MAX_GENERATED_PREFIX_LENGTH: usize = 16;

fn normalize_segment(value: &str, fallback: &str, maximum_length: usize) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let bounded = normalized
        .trim_matches('-')
        .chars()
        .take(maximum_length)
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if bounded.is_empty() {
        fallback.to_string()
    } else {
        bounded
    }
}

fn normalize_prefix(prefix: &str) -> String {
    normalize_segment(prefix, "fly", MAX_GENERATED_PREFIX_LENGTH)
}

fn normalize_hint(hint: &str, maximum_length: usize) -> String {
    normalize_segment(hint, "node", maximum_length)
}

fn base36(mut value: u128) -> String {
    if value == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while value > 0 {
        let digit = (value % 36) as u8;
        digits.push(if digit < 10 {
            b'0' + digit
        } else {
            b'a' + (digit - 10)
        });
        value /= 36;
    }
    digits.reverse();
    String::from_utf8(digits).unwrap_or_else(|_| "0".to_string())
}

/// Process-wide salt source so two generator instances created in the same nanosecond still
/// occupy disjoint id spaces.
#[cfg(not(target_arch = "wasm32"))]
static GENERATOR_SALT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(not(target_arch = "wasm32"))]
fn id_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn id_stamp() -> u128 {
    // Browser wasm32 has neither a supported std system clock nor a process ID.
    // UUID's workspace-enabled JS entropy source works without either.
    uuid::Uuid::new_v4().as_u128()
}

#[cfg(not(target_arch = "wasm32"))]
fn generator_salt() -> u64 {
    let instance = GENERATOR_SALT_COUNTER.fetch_add(1, Ordering::Relaxed);
    (id_stamp() as u64)
        ^ (u64::from(std::process::id()) << 32)
        ^ instance.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

#[cfg(target_arch = "wasm32")]
fn generator_salt() -> u64 {
    id_stamp() as u64
}

/// Collision-resistant id generator for interactive editing sessions.
///
/// [`SequentialIdGenerator`] restarts at `1` in every session, so two authors editing copies of
/// the same project concurrently mint identical ids for different components; the collision is
/// only discovered when the documents meet. This generator mixes a nanosecond timestamp, the
/// process id, a process-wide instance salt, and a per-instance counter on native targets;
/// WASM uses UUIDv4 entropy instead of unsupported standard-library clock/process APIs.
/// Both keep ids inside the
/// `[A-Za-z0-9_.:-]` identifier charset and far below the 128-byte limit. Ids are not
/// cryptographic tokens and must not be used as credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UniqueIdGenerator {
    prefix: String,
    salt: u64,
    next: u64,
}

impl UniqueIdGenerator {
    pub fn new(prefix: impl Into<String>) -> Self {
        let salt = generator_salt();
        let prefix = normalize_prefix(&prefix.into());
        Self {
            prefix,
            salt,
            next: 1,
        }
    }
}

impl Default for UniqueIdGenerator {
    fn default() -> Self {
        Self::new("fly")
    }
}

impl IdGenerator for UniqueIdGenerator {
    fn next_id(&mut self, hint: &str) -> String {
        let stamp = id_stamp();
        let counter = self.next;
        self.next = self.next.wrapping_add(1);

        // Deserialization can bypass new, so normalize the stored prefix at the
        // boundary too. Allocate the remaining identifier budget to the hint.
        let prefix = normalize_prefix(&self.prefix);
        let timestamp = base36(stamp);
        let counter = base36(u128::from(counter));
        let salt = base36(u128::from(self.salt));
        let fixed_length = prefix.len() + timestamp.len() + counter.len() + salt.len() + 4;
        let hint = normalize_hint(
            hint,
            MAX_GENERATED_ID_LENGTH.saturating_sub(fixed_length),
        );

        format!("{prefix}-{hint}-{timestamp}-{counter}-{salt}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SequentialIdGenerator {
    prefix: String,
    next: u64,
}

impl SequentialIdGenerator {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: normalize_prefix(&prefix.into()),
            next: 1,
        }
    }
}

impl Default for SequentialIdGenerator {
    fn default() -> Self {
        Self::new("fly")
    }
}

impl IdGenerator for SequentialIdGenerator {
    fn next_id(&mut self, hint: &str) -> String {
        // A deserialized prefix must not bypass the identifier contract.
        let prefix = normalize_prefix(&self.prefix);
        let counter = self.next.to_string();
        self.next = self.next.wrapping_add(1);
        let fixed_length = prefix.len() + counter.len() + 2;
        let hint = normalize_hint(
            hint,
            MAX_GENERATED_ID_LENGTH.saturating_sub(fixed_length),
        );
        format!("{prefix}-{hint}-{counter}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_identifier;
    use std::collections::BTreeSet;

    #[test]
    fn unique_generator_never_repeats_within_or_across_instances() {
        let mut first = UniqueIdGenerator::default();
        let mut second = UniqueIdGenerator::default();
        let mut seen = BTreeSet::new();
        for _ in 0..512 {
            assert!(seen.insert(first.next_id("hero")));
            assert!(seen.insert(second.next_id("hero")));
        }
    }

    #[test]
    fn unique_generator_ids_satisfy_the_identifier_contract() {
        let mut generator = UniqueIdGenerator::default();
        for hint in ["hero", "Hero Block!", "", "a/b"] {
            let id = generator.next_id(hint);
            validate_identifier(&id).unwrap_or_else(|error| {
                panic!("generated id `{id}` must be a valid identifier: {error}")
            });
        }
    }

    #[test]
    fn sequential_generator_stays_deterministic_for_fixtures() {
        let mut generator = SequentialIdGenerator::default();
        assert_eq!(generator.next_id("hero"), "fly-hero-1");
        assert_eq!(generator.next_id("hero"), "fly-hero-2");
    }

    #[test]
    fn generators_bound_prefix_and_hint_to_the_identifier_contract() {
        let long = "Long Prefix !".repeat(32);
        let mut unique = UniqueIdGenerator::new(long.clone());
        let mut sequential = SequentialIdGenerator::new(long.clone());

        for id in [unique.next_id(&long), sequential.next_id(&long)] {
            assert!(id.len() <= MAX_GENERATED_ID_LENGTH);
            validate_identifier(&id).unwrap_or_else(|error| {
                panic!("generated id {id} must be valid: {error}")
            });
        }
    }

    #[test]
    fn deserialized_generators_normalize_untrusted_prefixes_before_emitting_ids() {
        let mut unique: UniqueIdGenerator = serde_json::from_str(
            r#"{"prefix":" !!! invalid prefix !!! ","salt":1,"next":1}"#,
        )
        .expect("unique generator");
        let mut sequential: SequentialIdGenerator =
            serde_json::from_str(r#"{"prefix":" !!! invalid prefix !!! ","next":1}"#)
                .expect("sequential generator");

        for id in [unique.next_id(""), sequential.next_id("")] {
            assert!(id.len() <= MAX_GENERATED_ID_LENGTH);
            validate_identifier(&id).unwrap_or_else(|error| {
                panic!("generated id {id} must be valid: {error}")
            });
        }
    }
}
