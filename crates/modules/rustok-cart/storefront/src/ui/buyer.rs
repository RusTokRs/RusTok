//! Leptos binding of Cart-owned recovery. Local IDs/checkpoints are not owner capabilities.

use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_pricing_storefront::PricingResolutionContext;

use crate::core::{
    BuyerCartCheckpoint, BuyerCartError, BuyerCartLineAction, BuyerCartLinePlan, BuyerCartPhase,
    BuyerCartPlan, BuyerCartSession, CartCapabilityState, CartFetchRequest, buyer_cart_storage_key,
};
#[cfg(target_arch = "wasm32")]
use crate::core::{legacy_buyer_cart_storage_key, legacy_buyer_cart_storage_prefix};

/// Host-owned, reactive presentation availability. Server admission remains mandatory.
#[derive(Clone, Copy)]
pub struct CartCapability {
    state: RwSignal<CartCapabilityState>,
    verify_host: Callback<(), bool>,
}

impl CartCapability {
    pub fn new(enabled: bool) -> Self {
        Self::with_host_verifier(enabled, Callback::new(|_| true))
    }

    /// The verifier must synchronously refuse a missing or changed host projection. This
    /// closes the gap between a DOM snapshot replacement and delivery of its host event.
    pub fn with_host_verifier(enabled: bool, verify_host: Callback<(), bool>) -> Self {
        Self {
            state: RwSignal::new(CartCapabilityState::new(enabled)),
            verify_host,
        }
    }

    /// Call for every new host policy snapshot, including same-value replacements.
    pub fn replace(&self, enabled: bool) {
        self.state.update(|state| state.replace(enabled));
        crate::ui::bridge::publish_count(None);
    }

    /// Listener/host setup failure cannot be repaired by a later optimistic policy update.
    pub fn fail_closed(&self) {
        self.state.update(|state| state.fail_closed());
        crate::ui::bridge::publish_count(None);
    }

    /// The host snapshot and route can remain valid even when Cart itself is disabled.
    /// Product may still request a public quote, but this never grants a Cart write.
    pub fn host_current(&self) -> bool {
        self.verify_host.try_run(()) == Some(true)
    }

    pub fn get(&self) -> CartCapabilityState {
        let mut state = self.state.get();
        state.enabled &= self.host_current();
        state
    }

    pub fn get_untracked(&self) -> CartCapabilityState {
        let mut state = self
            .state
            .try_get_untracked()
            .unwrap_or(CartCapabilityState {
                enabled: false,
                generation: u64::MAX,
            });
        state.enabled &= self.host_current();
        state
    }
}

// A token remains page-local, never in DOM events, serialized snapshots, or diagnostics.
#[derive(Clone, PartialEq, Eq)]
struct ClientScope {
    token: Option<String>,
    tenant: String,
    page_path: Option<String>,
    auth: leptos_auth::ClientAuthScope,
    capability: CartCapabilityState,
}

fn browser_path() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()?.location().pathname().ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

fn checkpoint_key(scope: &ClientScope) -> Result<String, BuyerCartError> {
    let lineage = match scope.token.as_deref() {
        None => None,
        Some(token) if token.len() <= 16 * 1024 => {
            Some(leptos_auth::session_id_from_access_token(token).ok_or(BuyerCartError::Restore)?)
        }
        Some(_) => return Err(BuyerCartError::Restore),
    };
    buyer_cart_storage_key(&scope.tenant, lineage.as_deref())
}

fn browser_checkpoint(scope: &ClientScope) -> Result<BuyerCartCheckpoint, BuyerCartError> {
    let key = checkpoint_key(scope)?;
    #[cfg(target_arch = "wasm32")]
    {
        let storage = web_sys::window()
            .ok_or(BuyerCartError::Restore)?
            .local_storage()
            .map_err(|_| BuyerCartError::Restore)?
            .ok_or(BuyerCartError::Restore)?;
        let mut raw = storage
            .get_item(&key)
            .map_err(|_| BuyerCartError::Restore)?;
        let mut migrating = false;
        if raw.is_none() {
            let legacy_key = legacy_buyer_cart_storage_key(&scope.tenant, scope.token.as_deref())?;
            raw = storage
                .get_item(&legacy_key)
                .map_err(|_| BuyerCartError::Restore)?;
            migrating = raw.is_some();
            if raw.is_none() {
                // An older token may have rotated while a command was unconfirmed. It cannot
                // safely be assigned to this lineage (or silently ignored). Explicit reset can
                // start a v2 namespace without deleting another actor's hint.
                let prefix = legacy_buyer_cart_storage_prefix(&scope.tenant)?;
                let length = storage.length().map_err(|_| BuyerCartError::Restore)?;
                if length > 4096 {
                    return Err(BuyerCartError::Restore);
                }
                for index in 0..length {
                    if storage
                        .key(index)
                        .map_err(|_| BuyerCartError::Restore)?
                        .is_some_and(|name| name.starts_with(&prefix))
                    {
                        return Err(BuyerCartError::Restore);
                    }
                }
            }
        }
        let Some(raw) = raw else {
            return Ok(BuyerCartCheckpoint::default());
        };
        if raw.len() > 1024 {
            return Err(BuyerCartError::Restore);
        }
        let checkpoint: BuyerCartCheckpoint =
            serde_json::from_str(&raw).map_err(|_| BuyerCartError::Restore)?;
        checkpoint.validate()?;
        if migrating {
            storage
                .set_item(&key, &raw)
                .map_err(|_| BuyerCartError::Restore)?;
        }
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
    let key = checkpoint_key(scope)?;
    #[cfg(target_arch = "wasm32")]
    {
        let storage = web_sys::window()
            .ok_or(BuyerCartError::Restore)?
            .local_storage()
            .map_err(|_| BuyerCartError::Restore)?
            .ok_or(BuyerCartError::Restore)?;
        let raw = serde_json::to_string(checkpoint).map_err(|_| BuyerCartError::Restore)?;
        if storage
            .get_item(&key)
            .map_err(|_| BuyerCartError::Restore)?
            .as_deref()
            == Some(raw.as_str())
        {
            return Ok(());
        }
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
    auth_scope: Signal<leptos_auth::ClientAuthScope>,
    capability: CartCapability,
    external_auth_changed: RwSignal<bool>,
    inspection_scope: RwSignal<Option<ClientScope>>,
    retry: RwSignal<u64>,
    initialized: StoredValue<bool>,
    initial_reference: StoredValue<Option<String>>,
    /// Failed persistence retains the confirmed original-scope ID in memory, but never enables IO.
    volatile: StoredValue<BTreeMap<String, BuyerCartCheckpoint>>,
    persist_reference: Callback<Option<String>, bool>,
    persist_selection: Callback<String, bool>,
}

impl BuyerCartController {
    pub fn reference(&self) -> Option<String> {
        if !self.context_ready() {
            return None;
        }
        self.reference.get()
    }
    pub fn context_ready(&self) -> bool {
        !self.auth_loading.get()
            && !self.auth_refused.get()
            && self.capability.get().enabled
            && self.generation.get() < u64::MAX
            && self.bound_scope.get().as_ref() == Some(&self.scope.get())
            && self.current(&self.scope.get())
    }
    /// Product catalog quotes do not depend on Cart being enabled. They still use the
    /// verified tenant, auth and host route and are fenced by the Cart/auth generations.
    pub fn quote_context_ready(&self) -> bool {
        let scope = self.scope.get();
        !self.auth_loading.get()
            && !self.auth_refused.get()
            && self.generation.get() < u64::MAX
            && self.capability.get().generation < u64::MAX
            && self.capability.host_current()
            && scope.auth.is_ready()
            && self.auth_scope.get() == scope.auth
            && (!cfg!(target_arch = "wasm32") || scope.page_path.is_some())
            && browser_path() == scope.page_path
            && leptos_auth::storage::get_token() == scope.token
            && leptos_auth::storage::matches_request_tenant(&scope.tenant)
    }

    /// Disposal-safe, untracked pre-send check for asynchronously scheduled Product quotes.
    pub fn quote_context_ready_untracked(&self) -> bool {
        let Some(scope) = self.scope.try_get_untracked() else {
            return false;
        };
        self.auth_loading.try_get_untracked() == Some(false)
            && self.auth_refused.try_get_untracked() == Some(false)
            && self.external_auth_changed.try_get_untracked() == Some(false)
            && self
                .generation
                .try_get_untracked()
                .is_some_and(|value| value < u64::MAX)
            && self.capability.get_untracked().generation < u64::MAX
            && self.capability.host_current()
            && scope.auth.is_ready()
            && self.auth_scope.try_get_untracked() == Some(scope.auth)
            && (!cfg!(target_arch = "wasm32") || scope.page_path.is_some())
            && browser_path() == scope.page_path
            && leptos_auth::storage::get_token() == scope.token
            && leptos_auth::storage::matches_request_tenant(&scope.tenant)
    }

    /// The host owns history writes, including selection-only changes.
    pub fn persist_selection(&self, href: String) -> bool {
        self.context_ready() && self.persist_selection.run(href)
    }

    /// Separate synchronous generations also fence A → B → A before an effect is polled.
    pub fn capability_generation(&self) -> u64 {
        self.capability.get().generation
    }
    pub fn auth_generation(&self) -> u64 {
        self.auth_scope.get().generation
    }
    pub fn confirmed_item_count(&self) -> Option<u64> {
        if self.busy.get() || !self.context_ready() {
            None
        } else {
            self.session.get().confirmed_item_count()
        }
    }
    /// Other browser providers/tabs may have changed an HttpOnly session we cannot inspect.
    /// Refuse until a full reload restores the canonical session; no credentials are broadcast.
    pub fn invalidate_external_auth(&self) {
        // Fence Product observations synchronously, before any reactive restore effect runs.
        self.generation
            .update(|generation| *generation = generation.saturating_add(1));
        self.external_auth_changed.set(true);
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn matches_checkpoint_key(&self, key: &str) -> bool {
        let scope = self.scope.get_untracked();
        checkpoint_key(&scope).is_ok_and(|current| current == key)
            || legacy_buyer_cart_storage_prefix(&scope.tenant)
                .is_ok_and(|prefix| key.starts_with(&prefix))
    }
    pub fn can_write(&self) -> bool {
        !self.busy.get() && self.context_ready() && self.state.get().permits_write()
    }
    pub fn can_reset(&self) -> bool {
        !self.busy.get() && self.context_ready()
    }
    fn current(&self, scope: &ClientScope) -> bool {
        self.auth_loading.try_get_untracked() == Some(false)
            && self.auth_refused.try_get_untracked() == Some(false)
            && self.external_auth_changed.try_get_untracked() == Some(false)
            && scope.auth.is_ready()
            && self.auth_scope.try_get_untracked() == Some(scope.auth)
            && scope.capability.enabled
            && self.capability.get_untracked() == scope.capability
            && self.scope.try_get_untracked().as_ref() == Some(scope)
            && self.bound_scope.try_get_untracked().flatten().as_ref() == Some(scope)
            && (!cfg!(target_arch = "wasm32") || scope.page_path.is_some())
            && browser_path() == scope.page_path
            && leptos_auth::storage::get_token() == scope.token
            && leptos_auth::storage::matches_request_tenant(&scope.tenant)
    }
    fn current_generation(&self, scope: &ClientScope, generation: u64) -> bool {
        generation < u64::MAX
            && self.generation.try_get_untracked() == Some(generation)
            && self.current(scope)
    }
    fn checkpoint(&self, scope: &ClientScope) -> Result<BuyerCartCheckpoint, BuyerCartError> {
        let key = checkpoint_key(scope)?;
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
        let key = checkpoint_key(scope)?;
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
            let id = self.state.get_untracked().cart_id;
            crate::ui::bridge::publish_count(None);
            self.inspection_scope.set(Some(self.scope.get_untracked()));
            self.state.set(BuyerCartSession::restoring(id));
            self.bound_scope.set(None);
            self.generation
                .update(|generation| *generation = generation.saturating_add(1));
            self.retry.update(|retry| *retry = retry.saturating_add(1));
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
        crate::ui::bridge::publish_count(None);
        self.initialized.set_value(true);
        self.inspection_scope.set(None);
        self.reference.set(None);
        self.state.set(BuyerCartSession::restoring(None));
        self.bound_scope.set(None);
        self.generation
            .update(|generation| *generation = generation.saturating_add(1));
        self.retry.update(|retry| *retry = retry.saturating_add(1));
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
        let generation = self.generation.get_untracked();
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
        crate::ui::bridge::publish_count(None);
        self.busy.set(true);
        let result = self
            .run_add(&scope, generation, plan, variant_id, context, locale)
            .await;
        self.busy.set(false);
        if !self.current_generation(&scope, generation) {
            // The original pending marker remains. Do not publish or clear an old-scope result.
            self.retry.update(|retry| *retry = retry.saturating_add(1));
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
    /// Decrement/remove share the same pending checkpoint and scope fence as Product adds.
    /// A successful write is not a new count until a subsequent owner read succeeds.
    pub async fn change_line(
        &self,
        line_id: String,
        action: BuyerCartLineAction,
        locale: Option<String>,
    ) -> bool {
        if !self.can_write() {
            return false;
        }
        let scope = self.scope.get_untracked();
        let generation = self.generation.get_untracked();
        let checkpoint = match self.checkpoint(&scope) {
            Ok(checkpoint) if !checkpoint.mutation_pending => checkpoint,
            Ok(_) => {
                self.state
                    .update(|session| session.fail(BuyerCartError::Unconfirmed));
                return false;
            }
            Err(error) => {
                self.state.update(|session| session.fail(error));
                return false;
            }
        };
        let session = self.state.get_untracked();
        let plan = match session.plan_line_change(line_id, action) {
            Ok(plan) => plan,
            Err(error) => {
                self.state.update(|session| session.fail(error));
                return false;
            }
        };
        let Some(cart_id) = session.cart_id.clone() else {
            return false;
        };
        if checkpoint.cart_id.as_ref() != Some(&cart_id)
            || self
                .save_checkpoint(
                    &scope,
                    BuyerCartCheckpoint {
                        cart_id: Some(cart_id.clone()),
                        mutation_pending: true,
                    },
                )
                .is_err()
        {
            self.state
                .update(|session| session.fail(BuyerCartError::Restore));
            return false;
        }
        crate::ui::bridge::publish_count(None);
        self.busy.set(true);
        // Publishing invalidation synchronously invokes page listeners. They may change or
        // dispose the host before this scheduled command reaches its transport.
        if !self.current_generation(&scope, generation) {
            self.busy.set(false);
            self.retry.update(|retry| *retry = retry.saturating_add(1));
            return false;
        }
        let changed = match plan {
            BuyerCartLinePlan::Decrement(request) => {
                crate::transport::decrement_line_item(request).await
            }
            BuyerCartLinePlan::Remove(request) => crate::transport::remove_line_item(request).await,
        };
        let observed = if changed.is_ok() && self.current_generation(&scope, generation) {
            crate::transport::fetch_cart(CartFetchRequest {
                selected_cart_id: Some(cart_id),
                locale,
            })
            .await
            .ok()
            .and_then(|data| data.cart)
        } else {
            None
        };
        if !self.current_generation(&scope, generation) {
            self.busy.set(false);
            self.retry.update(|retry| *retry = retry.saturating_add(1));
            return false;
        }
        let result = observed
            .ok_or(BuyerCartError::Unconfirmed)
            .and_then(|cart| session.finish_restore(Some(cart)))
            .and_then(|restored| {
                self.save_checkpoint(
                    &scope,
                    BuyerCartCheckpoint {
                        cart_id: restored.cart_id.clone(),
                        mutation_pending: false,
                    },
                )
                .map_err(|_| BuyerCartError::Unconfirmed)?;
                Ok(restored)
            });
        match result {
            Ok(restored) => {
                self.state.set(restored);
                self.busy.set(false);
                true
            }
            Err(_) => {
                self.state
                    .update(|session| session.fail(BuyerCartError::Unconfirmed));
                self.busy.set(false);
                false
            }
        }
    }

    async fn run_add(
        &self,
        scope: &ClientScope,
        generation: u64,
        mut plan: BuyerCartPlan,
        variant_id: String,
        context: PricingResolutionContext,
        locale: Option<String>,
    ) -> Result<(), BuyerCartError> {
        if let BuyerCartPlan::Create(request) = plan {
            if !self.current_generation(scope, generation) {
                return Err(BuyerCartError::Restore);
            }
            let cart = crate::transport::create_cart(request)
                .await
                .map_err(|_| BuyerCartError::Unconfirmed)?;
            let mut created = BuyerCartSession::empty();
            created
                .adopt(cart, false)
                .map_err(|_| BuyerCartError::Unconfirmed)?;
            // A stale response must not overwrite a newer checkpoint or URL. The original
            // pending marker remains; without a current owner the create outcome needs recovery.
            if !self.current_generation(scope, generation) {
                return Err(BuyerCartError::Unconfirmed);
            }
            let saved = self.save_checkpoint(
                scope,
                BuyerCartCheckpoint {
                    cart_id: created.cart_id.clone(),
                    mutation_pending: true,
                },
            );
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
        if !self.current_generation(scope, generation) {
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
        if !self.current_generation(scope, generation) {
            return Err(BuyerCartError::Unconfirmed);
        }
        let saved = self.save_checkpoint(
            scope,
            BuyerCartCheckpoint {
                cart_id: added.cart_id.clone(),
                mutation_pending: false,
            },
        );
        self.state.set(added);
        saved.map_err(|_| BuyerCartError::Unconfirmed)
    }
}

pub fn use_buyer_cart(
    cart_id: Option<String>,
    locale: Option<String>,
    tenant_slug: String,
    capability: CartCapability,
    persist_reference: Callback<Option<String>, bool>,
    persist_selection: Callback<String, bool>,
) -> BuyerCartController {
    let auth = use_context::<leptos_auth::AuthContext>();
    let auth_for_stamp = auth.clone();
    let auth_scope = Signal::derive(move || {
        auth_for_stamp.as_ref().map_or_else(
            || leptos_auth::ClientAuthScope::new(false),
            |auth| auth.client_scope(),
        )
    });
    let auth_for_scope = auth.clone();
    let scope_tenant = tenant_slug.clone();
    let page_path = browser_path();
    let scope = Memo::new(move |_| ClientScope {
        token: auth_for_scope.as_ref().and_then(|auth| auth.get_token()),
        tenant: scope_tenant.clone(),
        page_path: page_path.clone(),
        auth: auth_scope.get(),
        capability: capability.get(),
    });
    let external_auth_changed = RwSignal::new(false);
    let auth_for_refusal = auth.clone();
    let auth_refused = Signal::derive(move || {
        external_auth_changed.get()
            || auth_for_refusal.as_ref().is_none_or(|auth| {
                auth.get_tenant()
                    .is_some_and(|tenant| tenant != tenant_slug)
                    || (auth.session.get().is_some() && auth.is_token_expired())
                    || !leptos_auth::storage::matches_request_tenant(&tenant_slug)
            })
    });
    let auth_loading = Signal::derive(move || {
        !auth_scope.get().is_ready() || auth.as_ref().is_none_or(|auth| auth.is_loading.get())
    });
    let state = RwSignal::new(BuyerCartSession::restoring(cart_id.clone()));
    let bound_scope = RwSignal::new(None::<ClientScope>);
    let session = Signal::derive(move || {
        if !capability.get().enabled {
            let mut hidden = BuyerCartSession::restoring(None);
            hidden.phase = BuyerCartPhase::Unavailable;
            return hidden;
        }
        if auth_refused.get() {
            let mut hidden = BuyerCartSession::restoring(None);
            hidden.phase = BuyerCartPhase::AuthRefused;
            return hidden;
        }
        let current_scope = scope.get();
        if auth_loading.get()
            || bound_scope.get().as_ref() != Some(&current_scope)
            || auth_scope.get() != current_scope.auth
            || capability.get() != current_scope.capability
            || leptos_auth::storage::get_token() != current_scope.token
        {
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
        auth_scope,
        capability,
        external_auth_changed,
        inspection_scope: RwSignal::new(None),
        retry: RwSignal::new(0),
        initialized: StoredValue::new(false),
        initial_reference: StoredValue::new(cart_id),
        volatile: StoredValue::new(BTreeMap::new()),
        persist_reference,
        persist_selection,
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
            .update(|generation| *generation = generation.saturating_add(1));
        let generation = controller.generation.get_untracked();
        if !scope.capability.enabled || generation == u64::MAX {
            controller.state.set(BuyerCartSession {
                cart_id: None,
                cart: None,
                phase: BuyerCartPhase::Unavailable,
            });
            return;
        }
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
        let inspecting = controller.inspection_scope.get_untracked().as_ref() == Some(&scope);
        controller.inspection_scope.set(None);
        // Only the initial URL is an adoption request; it cannot override an unconfirmed
        // operation or silently discard a different saved reference.
        let requested = (!controller.initialized.get_value())
            .then(|| controller.initial_reference.get_value())
            .flatten();
        controller.initialized.set_value(true);
        let conflict = requested
            .as_ref()
            .zip(checkpoint.cart_id.as_ref())
            .is_some_and(|(requested, saved)| requested != saved);
        let id = if checkpoint.mutation_pending || conflict {
            checkpoint.cart_id.clone()
        } else {
            requested.or(checkpoint.cart_id.clone())
        };
        controller.reference.set(id.clone());
        let mut session = BuyerCartSession::restoring(id);
        if checkpoint.mutation_pending && (!inspecting || checkpoint.cart_id.is_none()) {
            session.fail(BuyerCartError::Unconfirmed);
            controller.state.set(session);
            return;
        }
        if conflict && !inspecting {
            session.fail(BuyerCartError::Restore);
            controller.state.set(session);
            return;
        }
        let id = session.restore_id();
        controller.state.set(session);
        let locale = locale.clone();
        spawn_local(async move {
            // The effect and its spawned task need not run in the same turn. Never send a
            // restore after a capability, auth, tenant, route or owner transition.
            if !controller.current_generation(&scope, generation) {
                return;
            }
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
            if !controller.current_generation(&scope, generation) {
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
                    if !controller.current_generation(&scope, generation) {
                        return;
                    }
                    let persisted =
                        saved.is_ok() && controller.persist_reference.run(restored.cart_id.clone());
                    if !controller.current_generation(&scope, generation) {
                        return;
                    }
                    session = restored;
                    if !persisted {
                        session.fail(BuyerCartError::Restore);
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

#[cfg(all(test, not(target_arch = "wasm32"), not(feature = "hydrate")))]
mod tests {
    use super::*;

    #[test]
    fn disabled_cart_does_not_hide_public_quotes_but_stale_host_does() {
        let owner = Owner::new();
        owner.with(|| {
            provide_context(leptos_auth::AuthContext::new());
            let host_current = RwSignal::new(true);
            let capability = CartCapability::with_host_verifier(
                false,
                Callback::new(move |_| host_current.get()),
            );
            let controller = use_buyer_cart(
                None,
                Some("en".into()),
                "shop".into(),
                capability,
                Callback::new(|_| false),
                Callback::new(|_| false),
            );
            assert!(!controller.can_write());
            assert!(controller.quote_context_ready());
            let old_generation = controller.capability_generation();
            capability.replace(false);
            assert!(controller.quote_context_ready());
            assert_ne!(controller.capability_generation(), old_generation);
            host_current.set(false);
            assert!(!controller.quote_context_ready());
            assert!(!controller.quote_context_ready_untracked());
            assert!(!controller.can_write());
            host_current.set(true);
            capability.fail_closed();
            assert!(!controller.quote_context_ready());
            assert!(!controller.quote_context_ready_untracked());
        });
    }
}
