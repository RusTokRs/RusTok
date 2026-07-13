//! Framework-neutral visual-editor state, intents, policies, and contribution contracts.

mod contribution;
mod dnd;
mod error;
mod keyboard;
mod machine;
mod resize;
mod state;
mod viewport;

pub use contribution::*;
pub use dnd::*;
pub use error::*;
pub use keyboard::*;
pub use machine::*;
pub use resize::*;
pub use state::*;
pub use viewport::*;

pub type UiResult<T> = Result<T, UiError>;

#[cfg(test)]
mod tests;
