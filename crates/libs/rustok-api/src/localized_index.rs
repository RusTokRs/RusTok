//! Native owner projection contract for exact localized records.

use std::{num::NonZeroU64, sync::Arc};

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PortError;

pub const MAX_LOCALIZED_INDEX_SCAN: usize = 1_000;
pub const MAX_LOCALIZED_INDEX_LOAD: usize = 256;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalizedIndexKey {
    pub id: Uuid,
    pub locale: String,
}

pub struct LocalizedIndexRow {
    pub tenant_id: Uuid,
    pub key: LocalizedIndexKey,
    pub revision: NonZeroU64,
    /// A published title, or a tombstone at this owner's current revision.
    pub published_title: Option<String>,
}

pub struct LocalizedIndexPage {
    pub rows: Vec<LocalizedIndexRow>,
    pub next: Option<LocalizedIndexKey>,
}

/// Internal replay and point reads. The host supplies the trusted tenant.
/// Rows retain exact locale identities and never synthesize missing copies.
#[async_trait]
pub trait LocalizedIndexReader: Send + Sync {
    /// Load every exact copy of a single record from one owner snapshot.
    /// Refresh requires at least one copy, all at the same revision.
    async fn load_record(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Vec<LocalizedIndexRow>, PortError>;

    async fn scan(
        &self,
        tenant_id: Uuid,
        locale: Option<String>,
        after: Option<LocalizedIndexKey>,
        limit: usize,
    ) -> Result<LocalizedIndexPage, PortError>;

    async fn load(
        &self,
        tenant_id: Uuid,
        keys: Vec<LocalizedIndexKey>,
    ) -> Result<Vec<LocalizedIndexRow>, PortError>;
}

#[derive(Clone, Copy)]
pub struct LocalizedIndexSource {
    pub module_slug: &'static str,
    pub entity_type: &'static str,
    pub build: fn(DatabaseConnection) -> Arc<dyn LocalizedIndexReader>,
    pub public_discovery_key: &'static str,
    /// Trusted owner SQL that proves the exact locale is published at the Index
    /// row's revision. The Index compiler supplies the `{{entity}}` alias.
    pub postgres_publication_freshness: &'static str,
}

#[derive(Default)]
pub struct LocalizedIndexSources(Vec<LocalizedIndexSource>);

impl LocalizedIndexSources {
    pub fn register(&mut self, source: LocalizedIndexSource) {
        self.0.push(source);
    }

    pub fn sources(&self) -> &[LocalizedIndexSource] {
        &self.0
    }
}
