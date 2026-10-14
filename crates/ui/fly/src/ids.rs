use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait IdGenerator {
    fn next_id(&mut self, hint: &str) -> String;
}

fn normalize_hint(hint: &str) -> String {
    let normalized = hint
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if normalized.is_empty() {
        "node".to_string()
    } else {
        normalized
    }
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
static GENERATOR_SALT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Collision-resistant id generator for interactive editing sessions.
///
/// [`SequentialIdGenerator`] restarts at `1` in every session, so two authors editing copies of
/// the same project concurrently mint identical ids for different components; the collision is
/// only discovered when the documents meet. This generator mixes a nanosecond timestamp, the
/// process id, a process-wide instance salt, and a per-instance counter, which makes accidental
/// collision across sessions practically impossible while keeping ids inside the
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
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or_default();
        let instance = GENERATOR_SALT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let salt = now_nanos
            ^ (u64::from(std::process::id()) << 32)
            ^ instance.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        Self {
            prefix: prefix.into(),
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
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let counter = self.next;
        self.next += 1;
        format!(
            "{}-{}-{}-{}-{}",
            self.prefix,
            normalize_hint(hint),
            base36(now_nanos),
            base36(u128::from(counter)),
            base36(u128::from(self.salt)),
        )
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
            prefix: prefix.into(),
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
        let id = format!("{}-{}-{}", self.prefix, normalize_hint(hint), self.next);
        self.next += 1;
        id
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
}
