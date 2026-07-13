//! Framework-neutral visual-editor state, intents, policies, and contribution contracts.

mod contribution;
mod dnd;
mod error;
mod machine;
mod state;

pub use contribution::*;
pub use dnd::*;
pub use error::*;
pub use machine::*;
pub use state::*;

pub type UiResult<T> = Result<T, UiError>;

#[cfg(test)]
mod tests;
