//! Consumer-port fake for Product transaction tests. This does NOT prove Media
//! persistence/GC or gRPC behavior; those need real owner integration tests.
#![allow(dead_code)]

use rustok_api::{PortContext, PortError};
use rustok_product::{
    ProductMediaReference, ProductMediaReferencePage, ProductMediaReferencePort, async_trait,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use uuid::Uuid;
#[derive(Default)]
pub struct TestMediaReferencePort {
    pub holds: Mutex<BTreeMap<(Uuid, Uuid), Uuid>>,
    pub asset_owners: Mutex<BTreeMap<Uuid, Uuid>>,
    pub calls: Mutex<Vec<(String, PortContext, Uuid, Uuid)>>,
    pub unavailable: AtomicBool,
    pub reject_release: AtomicBool,
    pub releases: AtomicUsize,
}
impl TestMediaReferencePort {
    pub fn has(&self, tenant: Uuid, image: Uuid) -> bool {
        self.holds.lock().unwrap().contains_key(&(tenant, image))
    }
    pub fn hold_count(&self) -> usize {
        self.holds.lock().unwrap().len()
    }
    fn scope(&self, context: &PortContext) -> Result<Uuid, PortError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(PortError::unavailable("test.unavailable", "unavailable"));
        }
        Uuid::parse_str(&context.tenant_id)
            .map_err(|_| PortError::validation("test.tenant", "invalid tenant"))
    }
}
#[async_trait]
impl ProductMediaReferencePort for TestMediaReferencePort {
    async fn retain(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        let tenant = self.scope(&context)?;
        self.calls
            .lock()
            .unwrap()
            .push(("retain".into(), context, media_id, image_id));
        // Unknown assets are lazily seeded for ordinary Product fixtures only.
        let mut owners = self.asset_owners.lock().unwrap();
        let owner = owners.entry(media_id).or_insert(tenant);
        if *owner != tenant {
            return Err(PortError::not_found("test.asset_missing", "asset missing"));
        }
        self.holds
            .lock()
            .unwrap()
            .insert((tenant, image_id), media_id);
        Ok(())
    }
    async fn release(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        let tenant = self.scope(&context)?;
        if self.reject_release.load(Ordering::SeqCst) {
            return Err(PortError::unavailable(
                "test.release_failed",
                "release failed",
            ));
        }
        self.calls
            .lock()
            .unwrap()
            .push(("release".into(), context, media_id, image_id));
        self.releases.fetch_add(1, Ordering::SeqCst);
        let mut holds = self.holds.lock().unwrap();
        if holds
            .get(&(tenant, image_id))
            .is_some_and(|existing| *existing != media_id)
        {
            return Err(PortError::invariant_violation(
                "test.mismatch",
                "mismatched hold",
            ));
        }
        holds.remove(&(tenant, image_id));
        Ok(())
    }
    async fn lookup(
        &self,
        context: PortContext,
        image_ids: Vec<Uuid>,
    ) -> Result<Vec<ProductMediaReference>, PortError> {
        let tenant_id = self.scope(&context)?;
        let ids: BTreeSet<_> = image_ids.into_iter().collect();
        Ok(self
            .holds
            .lock()
            .unwrap()
            .iter()
            .filter_map(|((tenant, image_id), media_id)| {
                (*tenant == tenant_id && ids.contains(image_id)).then_some(ProductMediaReference {
                    tenant_id,
                    image_id: *image_id,
                    media_id: *media_id,
                })
            })
            .collect())
    }
    async fn list(
        &self,
        context: PortContext,
        after: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaReferencePage, PortError> {
        let tenant_id = self.scope(&context)?;
        let mut references: Vec<_> =
            self.holds
                .lock()
                .unwrap()
                .iter()
                .filter_map(|((tenant, image_id), media_id)| {
                    (*tenant == tenant_id && after.is_none_or(|cursor| *image_id > cursor))
                        .then_some(ProductMediaReference {
                            tenant_id,
                            image_id: *image_id,
                            media_id: *media_id,
                        })
                })
                .take(limit as usize + 1)
                .collect();
        let has_more = references.len() > limit as usize;
        references.truncate(limit as usize);
        Ok(ProductMediaReferencePage {
            next_image_id: references.last().map(|row| row.image_id),
            references,
            has_more,
        })
    }
}
