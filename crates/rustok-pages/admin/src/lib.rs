mod browser_intent;
mod builder;
#[cfg(test)]
mod builder_contract;
mod composition;
mod core;
mod i18n;
mod model;
mod transport;
pub mod ui;

pub use browser_intent::{
    dispatch_pages_browser_intent, PagesBrowserIntentError, PagesBrowserIntentResponse,
};
pub use composition::PagesAdmin;
