mod create;
mod helpers;
mod persistence;
mod read;
mod update;

use rustok_content::entities::node::ContentStatus;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::entities::{page_body, page_translation};

pub(crate) use helpers::is_page_visible_for_channel;

pub(super) const PAGE_KIND: &str = "page";

pub struct PageService {
    pub(super) db: DatabaseConnection,
    pub(super) event_bus: TransactionalEventBus,
}

impl PageService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }
}

pub(super) struct PageResponseParts {
    pub(super) channel_slugs: Vec<String>,
    pub(super) locale: String,
    pub(super) fallback_locale: Option<String>,
}

pub(super) struct PreparedPageBody {
    pub(super) locale: String,
    pub(super) content: String,
    pub(super) format: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageTransition {
    Publish,
    Unpublish,
    Archive,
}

impl PageTransition {
    pub(super) fn from_status(status: Option<&ContentStatus>) -> Option<Self> {
        match status {
            Some(ContentStatus::Published) => Some(Self::Publish),
            Some(ContentStatus::Draft) => Some(Self::Unpublish),
            Some(ContentStatus::Archived) => Some(Self::Archive),
            None => None,
        }
    }

    pub(super) fn status(self) -> ContentStatus {
        match self {
            Self::Publish => ContentStatus::Published,
            Self::Unpublish => ContentStatus::Draft,
            Self::Archive => ContentStatus::Archived,
        }
    }
}

pub(super) struct ResolvedTranslationRecord<'a> {
    pub(super) translation: Option<&'a page_translation::Model>,
    pub(super) effective_locale: String,
}

pub(super) struct ResolvedBodyRecord<'a> {
    pub(super) body: Option<&'a page_body::Model>,
    pub(super) effective_locale: String,
}
