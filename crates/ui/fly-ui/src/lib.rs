//! Framework-neutral visual-editor state, intents, policies, and contribution contracts.

mod capability_policy;
mod command_capability;
mod contribution;
mod contribution_adapter;
mod contribution_factory;
mod contribution_manifest;
mod dnd;
mod error;
mod keyboard;
mod machine;
mod palette_access;
mod resize;
mod state;
mod style;
mod viewport;

pub use capability_policy::*;
pub use command_capability::*;
pub use contribution::*;
pub use contribution_adapter::*;
pub use contribution_factory::*;
pub use contribution_manifest::*;
pub use dnd::*;
pub use error::*;
pub use keyboard::*;
pub use machine::*;
pub use palette_access::*;
pub use resize::*;
pub use state::*;
pub use style::*;
pub use viewport::*;

pub type UiResult<T> = Result<T, UiError>;

#[cfg(test)]
mod tests;
