use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreSnapshot<T> {
    pub state: T,
}

impl<T> StoreSnapshot<T> {
    pub fn new(state: T) -> Self {
        Self { state }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreUpdate<T> {
    pub previous: T,
    pub next: T,
}
