//! Leptos binding of Cart-owned recovery. Local IDs/checkpoints are not owner capabilities.

use std::collections::BTreeMap;

use leptos::prelude::*;
use rustok_pricing_storefront::PricingResolutionContext;

use crate::core::{
    BuyerCartCheckpoint, BuyerCartError, BuyerCartPhase, BuyerCartPlan, BuyerCartSession,
    CartFetchRequest, buyer_cart_storage_key,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ClientScope {
    token: Option<String>,
    tenant: String,
}

fn browser_checkpoint(scope: &ClientScope) -> Result<BuyerCartCheckpoint, BuyerCartError> {
    let key = buyer_cart_storage_key(&scope.tenant, scope.token.as_deref())?;
    #[cfg(target_arch = "wasm32")]
    {
        let storage = web_sys::window()
            .ok_or(BuyerCartError::Restore)?
            .local_storage()
            .map_err(|_| BuyerCartError::Restore)?
            .ok_or(BuyerCartError::Restore)?;
        let Some(raw) = storage
            .get_item(&key)
            .map_err(|_| BuyerCartError::Restore)?
        else {
            return Ok(BuyerCartCheckpoint::default());
        };
        if raw.len() > 1024 {
            return Err(BuyerCartError::Restore);
        }
        let checkpoint: BuyerCartCheckpoint =
            serde_json::from_str(&raw).map_err(|_| BuyerCartError::Restore)?;
        checkpoint.validate()?;
        Ok(checkpoint)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        Err(BuyerCartError::Restore)
    }
}

fn save_browser_checkpoint(
    scope: &ClientScope,
    checkpoint: &BuyerCartCheckpoint,
) -> Result<(), BuyerCartError> {
    checkpoint.validate()?;
    let key = buyer_cart_storage_key(&scope.tenant, scope.token.as_deref())?;
    #[cfg(target_arch = "wasm32")]
    {
        let storage = web_sys::window()
            .ok_or(BuyerCartError::Restore)?
            .local_storage()
            .map_err(|_| BuyerCartError::Restore)?
            .ok_or(BuyerCartError::Restore)?;
        let raw = serde_json::to_string(checkpoint).map_err(|_| BuyerCartError::Restore)?;
        storage
            .set_item(&key, &raw)
            .map_err(|_| BuyerCartError::Restore)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        Err(BuyerCartError::Restore)
    }
}

#[derive(Clone, Copy)]
pub struct BuyerCartController {
    pub session: Signal<BuyerCartSession>,
    state: RwSignal<BuyerCartSession>,
    pub busy: RwSignal<bool>,
    /// Changes before a restore starts, invalidating earlier Product observations immediately.
    pub generation: RwSignal<u64>,
    reference: RwSignal<Option<String>>,
    scope: Memo<ClientScope>,
    bound_scope: RwSignal<Option<ClientScope>>,
    auth_loading: Signal<bool>,
    auth_refused: Signal<bool>,
    retry: RwSignal<u64>,
    initialized: StoredValue<bool>,
    initial_reference: StoredValue<Option<String>>,
    /// Failed persistence retains the confirmed original-scope ID in memory, but never enables IO.
    volatile: StoredValue<BTreeMap<String, BuyerCartCheckpoint>>,
    persist_reference: Callback<Option<String>, bool>,
}

impl BuyerCartController {
    pub fn reference(&self) -> Option<String> {
        if self.bound_scope.get().as_ref() == Some(&self.scope.get()) {
            self.reference.get()
        } else if !self.initialized.get_value() {
            self.initial_reference.get_value()
        } else {
            None
        }
    }
    pub fn context_ready(&self) -> bool {
        !self.auth_loading.get()
            && !self.auth_refused.get()
            && self.bound_scope.get().as_ref() == Some(&self.scope.get())
            && self.current(&self.scope.get())
    }
    pub fn can_write(&self) -> bool {
        !self.busy.get()
            && !self.auth_loading.get()
            && !self.auth_refused.get()
            && self.bound_scope.get().as_ref() == Some(&self.scope.get())
            && self.current(&self.scope.get())
            && self.state.get().permits_write()
    }
    pub fn can_reset(&self) -> bool {
        !self.busy.get() && self.current(&self.scope.get())
    }
    fn current(&self, scope: &ClientScope) -> bool {
        !self.auth_loading.get_untracked()
            && !self.auth_refused.get_untracked()
            && &self.scope.get_untracked() == scope
            && self.bound_scope.get_untracked().as_ref() == Some(scope)
            && leptos_auth::storage::get_token() == scope.token
            && leptos_auth::storage::get_tenant().map_or(!cfg!(target_arch = "wasm32"), |tenant| {
                tenant == scope.tenant
            })
    }
    fn checkpoint(&self, scope: &ClientScope) -> Result<BuyerCartCheckpoint, BuyerCartError> {
        let key = buyer_cart_storage_key(&scope.tenant, scope.token.as_deref())?;
        if let Some(checkpoint) = self
            .volatile
            .with_value(|entries| entries.get(&key).cloned())
        {
            return Ok(checkpoint);
        }
        browser_checkpoint(scope)
    }
    fn save_checkpoint(
        &self,
        scope: &ClientScope,
        checkpoint: BuyerCartCheckpoint,
    ) -> Result<(), BuyerCartError> {
        let key = buyer_cart_storage_key(&scope.tenant, scope.token.as_deref())?;
        self.volatile.update_value(|entries| {
            entries.insert(key.clone(), checkpoint.clone());
        });
        save_browser_checkpoint(scope, &checkpoint)?;
        self.volatile.update_value(|entries| {
            entries.remove(&key);
        });
        Ok(())
    }
    /// Owner read only, never a repeat of an unconfirmed mutation.
    pub fn inspect(&self) {
        if self.can_reset() && self.state.get_untracked().can_inspect() {
            self.retry.update(|retry| *retry = retry.wrapping_add(1));
        }
    }
    /// Explicit local reset does not delete a Cart or cancel an operation whose outcome is unknown.
    pub fn reset_reference(&self) {
        if !self.can_reset() {
            return;
        }
        let scope = self.scope.get_untracked();
        if self
            .save_checkpoint(&scope, BuyerCartCheckpoint::default())
            .is_err()
        {
            self.state
                .update(|session| session.fail(BuyerCartError::Restore));
            return;
        }
        if !self.persist_reference.run(None) {
            self.state
                .update(|session| session.fail(BuyerCartError::Restore));
            return;
        }
        self.initialized.set_value(true);
        self.reference.set(None);
        self.retry.update(|retry| *retry = retry.wrapping_add(1));
    }
    pub async fn add_variant(
        &self,
        variant_id: String,
        context: PricingResolutionContext,
        locale: Option<String>,
    ) -> bool {
        if !self.can_write() {
            return false;
        }
        let scope = self.scope.get_untracked();
        match self.checkpoint(&scope) {
            Ok(checkpoint) if checkpoint.mutation_pending => {
                self.state
                    .update(|session| session.fail(BuyerCartError::Unconfirmed));
                return false;
            }
            Err(error) => {
                self.state.update(|session| session.fail(error));
                return false;
            }
            Ok(_) => {}
        }
        let plan = match self.state.get_untracked().plan_add(
            variant_id.clone(),
            context.clone(),
            locale.clone(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                self.state.update(|session| session.fail(error));
                return false;
            }
        };
        // Persist uncertainty BEFORE IO. A reload/auth round-trip cannot become an automatic retry.
        let pending = BuyerCartCheckpoint {
            cart_id: self.state.get_untracked().cart_id,
            mutation_pending: true,
        };
        if self.save_checkpoint(&scope, pending).is_err() {
            self.state
                .update(|session| session.fail(BuyerCartError::Restore));
            return false;
        }
        self.busy.set(true);
        let result = self
            .run_add(&scope, plan, variant_id, context, locale)
            .await;
        self.busy.set(false);
        if !self.current(&scope) {
            // A confirmed old-scope response was retained under its original key, not published here.
            self.retry.update(|retry| *retry = retry.wrapping_add(1));
            return false;
        }
        match result {
            Ok(()) => true,
            Err(error) => {
                self.state.update(|session| session.fail(error));
                false
            }
        }
    }
    async fn run_add(
        &self,
        scope: &ClientScope,
        mut plan: BuyerCartPlan,
        variant_id: String,
        context: PricingResolutionContext,
        locale: Option<String>,
    ) -> Result<(), BuyerCartError> {
        if let BuyerCartPlan::Create(request) = plan {
            if !self.current(scope) {
                return Err(BuyerCartError::Restore);
            }
            let cart = crate::transport::create_cart(request)
                .await
                .map_err(|_| BuyerCartError::Unconfirmed)?;
            let mut created = BuyerCartSession::empty();
            created
                .adopt(cart, false)
                .map_err(|_| BuyerCartError::Unconfirmed)?;
            // Store even after an auth switch; do not put the old actor's response in the new UI/URL.
            let saved = self.save_checkpoint(
                scope,
                BuyerCartCheckpoint {
                    cart_id: created.cart_id.clone(),
                    mutation_pending: true,
                },
            );
            if !self.current(scope) {
                return Err(BuyerCartError::Unconfirmed);
            }
            self.reference.set(created.cart_id.clone());
            self.state.set(created.clone());
            if !self.persist_reference.run(created.cart_id.clone()) || saved.is_err() {
                return Err(BuyerCartError::Unconfirmed);
            }
            plan = created.plan_add(variant_id, context, locale)?;
        }
        let BuyerCartPlan::Add(request) = plan else {
            return Err(BuyerCartError::Invalid);
        };
        if !self.current(scope) {
            return Err(BuyerCartError::Restore);
        }
        let expected_id = request.cart_id.clone();
        let cart = crate::transport::add_variant(request)
            .await
            .map_err(|_| BuyerCartError::Unconfirmed)?;
        let mut added = BuyerCartSession::restoring(Some(expected_id));
        added
            .adopt(cart, true)
            .map_err(|_| BuyerCartError::Unconfirmed)?;
        let saved = self.save_checkpoint(
            scope,
            BuyerCartCheckpoint {
                cart_id: added.cart_id.clone(),
                mutation_pending: false,
            },
        );
        if !self.current(scope) {
            return Err(BuyerCartError::Unconfirmed);
        }
        self.state.set(added);
        saved.map_err(|_| BuyerCartError::Unconfirmed)
    }
}

pub fn use_buyer_cart(
    cart_id: Option<String>,
    locale: Option<String>,
    tenant_slug: String,
    persist_reference: Callback<Option<String>, bool>,
) -> BuyerCartController {
    let auth = use_context::<leptos_auth::AuthContext>();
    let auth_for_scope = auth.clone();
    let scope_tenant = tenant_slug.clone();
    let scope = Memo::new(move |_| ClientScope {
        token: auth_for_scope.as_ref().and_then(|auth| auth.get_token()),
        tenant: scope_tenant.clone(),
    });
    let auth_for_refusal = auth.clone();
    let auth_refused = Signal::derive(move || {
        auth_for_refusal.as_ref().is_none_or(|auth| {
            auth.get_tenant()
                .or_else(leptos_auth::storage::get_tenant)
                .map_or(cfg!(target_arch = "wasm32"), |tenant| tenant != tenant_slug)
        })
    });
    let auth_loading =
        Signal::derive(move || auth.as_ref().is_none_or(|auth| auth.is_loading.get()));
    let state = RwSignal::new(BuyerCartSession::restoring(cart_id.clone()));
    let bound_scope = RwSignal::new(None::<ClientScope>);
    let session = Signal::derive(move || {
        if auth_refused.get() {
            let mut hidden = BuyerCartSession::restoring(None);
            hidden.phase = BuyerCartPhase::AuthRefused;
            return hidden;
        }
        if auth_loading.get() || bound_scope.get().as_ref() != Some(&scope.get()) {
            return BuyerCartSession::restoring(None);
        }
        state.get()
    });
    let controller = BuyerCartController {
        session,
        state,
        busy: RwSignal::new(false),
        generation: RwSignal::new(0),
        reference: RwSignal::new(cart_id.clone()),
        scope,
        bound_scope,
        auth_loading,
        auth_refused,
        retry: RwSignal::new(0),
        initialized: StoredValue::new(false),
        initial_reference: StoredValue::new(cart_id),
        volatile: StoredValue::new(BTreeMap::new()),
        persist_reference,
    };
    Effect::new(move |_| {
        let scope = controller.scope.get();
        let loading = controller.auth_loading.get();
        let refused = controller.auth_refused.get();
        let _retry = controller.retry.get();
        let previous_scope = controller.bound_scope.get_untracked();
        controller.bound_scope.set(None);
        controller
            .generation
            .update(|generation| *generation = generation.wrapping_add(1));
        let generation = controller.generation.get_untracked();
        if refused {
            controller
                .state
                .update(|session| session.phase = BuyerCartPhase::AuthRefused);
            return;
        }
        if loading || controller.busy.get_untracked() {
            return;
        }
        // Clear private projections before rebinding a new actor. Same-scope inspection may
        // retain its public UUID, but a previous actor's Cart is never shown in the new scope.
        let known_id = (previous_scope.as_ref() == Some(&scope))
            .then(|| controller.state.get_untracked().cart_id)
            .flatten();
        controller.state.set(BuyerCartSession::restoring(known_id));
        controller.bound_scope.set(Some(scope.clone()));
        let checkpoint = match controller.checkpoint(&scope) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                controller.state.update(|session| session.fail(error));
                return;
            }
        };
        // Only the initial explicit URL is an adoption request. Later actor changes use their own
        // saved reference, not the previous actor's URL projection or cached Cart data.
        let id = if !controller.initialized.get_value() {
            controller
                .initial_reference
                .get_value()
                .or(checkpoint.cart_id.clone())
        } else {
            checkpoint.cart_id.clone()
        };
        controller.initialized.set_value(true);
        controller.reference.set(id.clone());
        let mut session = BuyerCartSession::restoring(id);
        if checkpoint.mutation_pending && checkpoint.cart_id.is_none() {
            session.fail(BuyerCartError::Unconfirmed);
            controller.state.set(session);
            controller.bound_scope.set(Some(scope));
            return;
        }
        let id = session.restore_id();
        controller.state.set(session);
        let locale = locale.clone();
        spawn_local(async move {
            let restored = match id {
                Ok(Some(id)) => crate::transport::fetch_cart(CartFetchRequest {
                    selected_cart_id: Some(id),
                    locale,
                })
                .await
                .map(|data| data.cart)
                .map_err(|_| BuyerCartError::Restore),
                Ok(None) => Ok(None),
                Err(_) => Err(BuyerCartError::Restore),
            };
            if !controller.current(&scope) || controller.generation.get_untracked() != generation {
                return;
            }
            let mut session = controller.state.get_untracked();
            match restored.and_then(|cart| session.finish_restore(cart)) {
                Ok(restored) => {
                    let saved = controller.save_checkpoint(
                        &scope,
                        BuyerCartCheckpoint {
                            cart_id: restored.cart_id.clone(),
                            mutation_pending: false,
                        },
                    );
                    if saved.is_err() || !controller.persist_reference.run(restored.cart_id.clone())
                    {
                        session = restored;
                        session.fail(BuyerCartError::Restore);
                    } else {
                        session = restored;
                    }
                    controller.reference.set(session.cart_id.clone());
                }
                Err(error) => session.fail(error),
            }
            controller.state.set(session);
            controller.bound_scope.set(Some(scope));
        });
    });
    controller
}
