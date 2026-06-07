#![allow(clippy::too_many_arguments)]
#![recursion_limit = "256"]

mod api;
pub mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::ForumAdmin;
