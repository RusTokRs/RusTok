use serde::{Deserialize, Serialize};

pub trait IdGenerator {
    fn next_id(&mut self, hint: &str) -> String;
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
        let id = format!(
            "{}-{}-{}",
            self.prefix,
            if normalized.is_empty() { "node" } else { &normalized },
            self.next
        );
        self.next += 1;
        id
    }
}
