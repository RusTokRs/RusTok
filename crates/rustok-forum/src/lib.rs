use async_trait::async_trait;
use rustok_core::RusToKModule;

pub mod constants;
pub mod dto;
pub mod entities;
pub mod error;
pub mod locale;
pub mod services;

pub use constants::*;
pub use dto::*;
pub use error::{ForumError, ForumResult};
pub use services::{CategoryService, ModerationService, ReplyService, TopicService};

pub struct ForumModule;

#[async_trait]
impl RusToKModule for ForumModule {
    fn slug(&self) -> &'static str {
        "forum"
    }

    fn name(&self) -> &'static str {
        "Forum"
    }

    fn description(&self) -> &'static str {
        "Forum categories, topics, replies, and moderation workflows"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content"]
    }
}

#[cfg(test)]
mod contract_tests;
