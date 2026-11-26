//! Read-only fake: no Taxonomy tables, no successful fallback writes.
use super::forum_under_test::{
    ForumError, ForumTaxonomyCategoryLocaleCopy, ForumTaxonomyCategorySnapshot,
    ForumTaxonomyConnection, ForumTaxonomyPlacement, ForumTaxonomyPort, ForumTaxonomyRouteMatch,
    ForumTaxonomySyncCategoryInput,
};
use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct TestTaxonomy {
    pub tenant: Uuid,
    pub category: Uuid,
    pub parent: Uuid,
    pub fail_at: Option<&'static str>,
    pub fail_on_call: usize,
    pub require_transaction: bool,
    pub parent_projection: Option<HashMap<Uuid, Option<Uuid>>>,
    pub malformed_snapshots: Option<&'static str>,
    pub calls: Arc<Mutex<Vec<&'static str>>>,
}

impl TestTaxonomy {
    pub fn new() -> Self {
        Self {
            tenant: Uuid::new_v4(),
            category: Uuid::new_v4(),
            parent: Uuid::new_v4(),
            fail_at: None,
            fail_on_call: 1,
            require_transaction: false,
            parent_projection: None,
            malformed_snapshots: None,
            calls: Default::default(),
        }
    }
    fn check(
        &self,
        operation: &'static str,
        conn: ForumTaxonomyConnection<'_>,
        tenant: Uuid,
    ) -> Result<(), ForumError> {
        assert_eq!(tenant, self.tenant);
        assert!(
            !matches!(conn, ForumTaxonomyConnection::None),
            "injected reads must retain the caller connection"
        );
        if self.require_transaction {
            assert!(
                matches!(conn, ForumTaxonomyConnection::Transaction(_)),
                "policy reads must retain their caller transaction"
            );
        }
        let operation_calls = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(operation);
            calls.iter().filter(|&&call| call == operation).count()
        };
        if self.fail_at == Some(operation) && operation_calls == self.fail_on_call {
            return Err(ForumError::capability_failure(
                "taxonomy",
                "test.provider_unavailable",
                "secret-provider-diagnostic",
                true,
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl ForumTaxonomyPort for TestTaxonomy {
    async fn lock_category_hierarchy_writer_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
    ) -> Result<(), ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }

    async fn load_category_parents(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Option<Uuid>>, ForumError> {
        self.check("parents", conn, tenant_id)?;
        if let Some(parents) = &self.parent_projection {
            return Ok(parents.clone());
        }
        Ok(category_ids
            .iter()
            .map(|id| {
                (
                    *id,
                    if *id == self.category {
                        Some(self.parent)
                    } else {
                        None
                    },
                )
            })
            .collect())
    }

    async fn load_category_siblings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _parent_id: Option<Uuid>,
    ) -> Result<Vec<Uuid>, ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }

    async fn shift_category_siblings_for_insert_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
        _parent_id: Option<Uuid>,
        _requested_position: i32,
    ) -> Result<(), ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }

    async fn move_category_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
        _parent_id: Option<Uuid>,
        _position: i32,
    ) -> Result<Vec<ForumTaxonomyPlacement>, ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }

    async fn reorder_category_siblings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _parent_id: Option<Uuid>,
        _ordered_ids: &[Uuid],
    ) -> Result<(), ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }

    async fn load_scoped_categories(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<Vec<ForumTaxonomyCategorySnapshot>, ForumError> {
        self.check("snapshots", conn, tenant_id)?;
        let mut snapshots: Vec<ForumTaxonomyCategorySnapshot> = category_ids
            .unwrap_or(&[self.parent, self.category])
            .iter()
            .map(|id| ForumTaxonomyCategorySnapshot {
                id: *id,
                parent_id: if *id == self.category {
                    Some(self.parent)
                } else {
                    None
                },
                position: 0,
                canonical_key: format!("category-{id}"),
                slug: if *id == self.category {
                    "help".into()
                } else {
                    "root".into()
                },
                name: "Provider category".into(),
                description: None,
                icon_key: None,
                color: None,
                available_locales: vec![locale.into()],
                requested_locale: locale.into(),
                effective_locale: locale.into(),
            })
            .collect();
        if snapshots.len() > 1 {
            match self.malformed_snapshots {
                Some("duplicate") => snapshots[1].id = snapshots[0].id,
                Some("foreign") => snapshots[1].id = Uuid::new_v4(),
                _ => {}
            }
        }
        Ok(snapshots)
    }

    async fn load_category_locale_copy_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
        _locale: &str,
    ) -> Result<Option<ForumTaxonomyCategoryLocaleCopy>, ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }

    async fn sync_category_copy_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _input: ForumTaxonomySyncCategoryInput,
    ) -> Result<(), ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }

    async fn resolve_category_route(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        requested_locale: &str,
        requested_slug: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<Option<ForumTaxonomyRouteMatch>, ForumError> {
        self.check("route", conn, tenant_id)?;
        if !["help", "old-help"].contains(&requested_slug) {
            return Ok(None);
        }
        Ok(Some(ForumTaxonomyRouteMatch {
            term_id: self.category,
            matched_locale: requested_locale.into(),
            alias_id: (requested_slug == "old-help").then(Uuid::new_v4),
        }))
    }

    async fn resolve_term_names(
        &self,
        _conn: ForumTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _term_ids: &[Uuid],
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }

    async fn ensure_tags_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _locale: &str,
        _tags: &[String],
    ) -> Result<Vec<Uuid>, ForumError> {
        panic!("unexpected taxonomy operation in read-only fixture")
    }
}
