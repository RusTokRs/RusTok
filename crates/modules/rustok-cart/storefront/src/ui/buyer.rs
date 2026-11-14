//! Leptos binding of Cart-owned recovery. Local IDs/checkpoints are not owner capabilities.

use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_pricing_storefront::PricingResolutionContext;

use crate::core::{
    BuyerCartCheckpoint, BuyerCartError, BuyerCartLineAction, BuyerCartLinePlan, BuyerCartPhase,
    BuyerCartPlan, BuyerCartSession, CartCapabilityState, CartFetchRequest, CartOperationState,
    buyer_cart_storage_key,
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
        if self
            .state
            .try_update(|state| state.replace(enabled))
            .is_some()
        {
            crate::ui::bridge::publish_count(None);
        }
    }

    /// Listener/host setup failure cannot be repaired by a later optimistic policy update.
    pub fn fail_closed(&self) {
        if self.state.try_update(|state| state.fail_closed()).is_some() {
            crate::ui::bridge::publish_count(None);
        }
    }

    /// The host snapshot and route can remain valid even when Cart itself is disabled.
    /// Product may still request a public quote, but this never grants a Cart write.
    pub fn host_current(&self) -> bool {
        self.verify_host.try_run(()) == Some(true)
    }

    pub fn get(&self) -> CartCapabilityState {
        let mut state = self.state.try_get().unwrap_or(CartCapabilityState {
            enabled: false,
            generation: u64::MAX,
        });
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
    auth: rustok_auth_ui::model::ClientAuthScope,
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
        Some(token) if token.len() <= 16 * 1024 => Some(
            rustok_auth_ui::core::session_id_from_access_token(token)
                .ok_or(BuyerCartError::Restore)?,
        ),
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

/// A dropped future is not evidence that its request was cancelled. Preserve the lock so
/// this owner cannot issue another write until the page is reloaded and recovery is explicit.
struct CartOperationGuard {
    state: RwSignal<CartOperationState>,
    id: u64,
    finished: bool,
}

impl CartOperationGuard {
    fn finish(mut self) -> bool {
        let finished = self.state.try_update(|state| state.finish(self.id)) == Some(true);
        self.finished = finished;
        finished
    }
}

impl Drop for CartOperationGuard {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.state.try_update(|state| state.abandon(self.id));
        }
    }
}

#[derive(Clone, Copy)]
pub struct BuyerCartController {
    pub session: Signal<BuyerCartSession>,
    state: RwSignal<BuyerCartSession>,
    /// Read-only projection; only the current operation guard can release the lock.
    pub busy: Signal<bool>,
    operation: RwSignal<CartOperationState>,
    /// Changes before a restore starts, invalidating earlier Product observations immediately.
    /// Only the Cart owner may advance this monotonic fence.
    generation: RwSignal<u64>,
    reference: RwSignal<Option<String>>,
    scope: Memo<ClientScope>,
    bound_scope: RwSignal<Option<ClientScope>>,
    auth_loading: Signal<bool>,
    auth_refused: Signal<bool>,
    auth_scope: Signal<rustok_auth_ui::model::ClientAuthScope>,
    capability: CartCapability,
    external_auth_changed: RwSignal<bool>,
    inspection_scope: RwSignal<Option<ClientScope>>,
    retry: RwSignal<u64>,
    initialized: StoredValue<bool>,
    initial_reference: StoredValue<Option<String>>,
    /// Failed persistence retains the confirmed original-scope ID in memory, but never enables IO.
    volatile: StoredValue<BTreeMap<String, BuyerCartCheckpoint>>,
    persist_reference: Callback<Option<String>, bool>,
    persist_selection: Callback<String, Option<String>>,
}

impl BuyerCartController {
    pub fn reference(&self) -> Option<String> {
        if !self.context_ready() {
            return None;
        }
        self.reference.try_get().flatten()
    }
    pub fn context_ready(&self) -> bool {
        let Some(scope) = self.scope.try_get() else {
            return false;
        };
        self.auth_loading.try_get() == Some(false)
            && self.auth_refused.try_get() == Some(false)
            && self.capability.get().enabled
            && self
                .generation
                .try_get()
                .is_some_and(|generation| generation < u64::MAX)
            && self.bound_scope.try_get().flatten().as_ref() == Some(&scope)
            && self.current(&scope)
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn context_ready_untracked(&self) -> bool {
        let Some(scope) = self.scope.try_get_untracked() else {
            return false;
        };
        self.generation
            .try_get_untracked()
            .is_some_and(|generation| self.current_generation(&scope, generation))
    }

    /// Product catalog quotes do not depend on Cart being enabled. They still use the
    /// verified tenant, auth and host route and are fenced by the Cart/auth generations.
    pub fn quote_context_ready(&self) -> bool {
        let Some(scope) = self.scope.try_get() else {
            return false;
        };
        self.auth_loading.try_get() == Some(false)
            && self.auth_refused.try_get() == Some(false)
            && self.external_auth_changed.try_get() == Some(false)
            && self
                .generation
                .try_get()
                .is_some_and(|generation| generation < u64::MAX)
            && self.capability.get().generation < u64::MAX
            && self.capability.host_current()
            && scope.auth.is_ready()
            && self.auth_scope.try_get() == Some(scope.auth)
            && (!cfg!(target_arch = "wasm32") || scope.page_path.is_some())
            && browser_path() == scope.page_path
            && rustok_auth_ui::ui::leptos::storage::get_token() == scope.token
            && rustok_auth_ui::ui::leptos::storage::matches_request_tenant(&scope.tenant)
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
            && rustok_auth_ui::ui::leptos::storage::get_token() == scope.token
            && rustok_auth_ui::ui::leptos::storage::matches_request_tenant(&scope.tenant)
    }

    /// The host owns history writes, including selection-only changes. Its returned URL
    /// includes current host/Cart selectors rather than stale values from the Product snapshot.
    pub fn persist_selection(&self, href: String) -> Option<String> {
        if !self.quote_context_ready() {
            return None;
        }
        let scope = self.scope.try_get()?;
        let capability = self.capability.get();
        let generation = self.generation.try_get()?;
        let target = self.persist_selection.try_run(href).flatten()?;
        (self.quote_context_ready()
            && self.scope.try_get().as_ref() == Some(&scope)
            && self.capability.get() == capability
            && self.generation.try_get() == Some(generation))
        .then_some(target)
    }

    /// Read-only Cart generation; a disposed owner always returns the exhausted sentinel.
    pub fn generation(&self) -> u64 {
        self.generation.try_get().unwrap_or(u64::MAX)
    }

    /// Separate synchronous generations also fence A → B → A before an effect is polled.
    pub fn capability_generation(&self) -> u64 {
        self.capability.get().generation
    }
    pub fn auth_generation(&self) -> u64 {
        self.auth_scope
            .try_get()
            .map_or(u64::MAX, |scope| scope.generation)
    }
    pub fn confirmed_item_count(&self) -> Option<u64> {
        if self.busy.try_get() != Some(false) || !self.context_ready() {
            None
        } else {
            self.session.try_get()?.confirmed_item_count()
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn confirmed_item_count_untracked(&self) -> Option<u64> {
        if self.busy.try_get_untracked() != Some(false) || !self.context_ready_untracked() {
            None
        } else {
            self.session.try_get_untracked()?.confirmed_item_count()
        }
    }
    /// Other browser providers/tabs may have changed an HttpOnly session we cannot inspect.
    /// Refuse until a full reload restores the canonical session; no credentials are broadcast.
    pub fn invalidate_external_auth(&self) {
        // Fence Product observations synchronously, before any reactive restore effect runs.
        let _ = self
            .generation
            .try_update(|generation| *generation = generation.saturating_add(1));
        let _ = self.external_auth_changed.try_set(true);
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn matches_checkpoint_key(&self, key: &str) -> bool {
        let Some(scope) = self.scope.try_get_untracked() else {
            return true;
        };
        checkpoint_key(&scope).is_ok_and(|current| current == key)
            || legacy_buyer_cart_storage_prefix(&scope.tenant)
                .is_ok_and(|prefix| key.starts_with(&prefix))
    }
    pub fn can_write(&self) -> bool {
        self.busy.try_get() == Some(false)
            && self.context_ready()
            && self
                .state
                .try_get()
                .is_some_and(|session| session.permits_write())
    }
    pub fn can_reset(&self) -> bool {
        self.busy.try_get() == Some(false) && self.context_ready()
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
            && rustok_auth_ui::ui::leptos::storage::get_token() == scope.token
            && rustok_auth_ui::ui::leptos::storage::matches_request_tenant(&scope.tenant)
    }
    fn current_generation(&self, scope: &ClientScope, generation: u64) -> bool {
        generation < u64::MAX
            && self.generation.try_get_untracked() == Some(generation)
            && self.current(scope)
    }
    /// Reserve before any synchronous browser event. Recovery is allowed in failed phases,
    /// but it shares the same lock as writes so callbacks cannot reenter either operation.
    fn begin_operation(
        &self,
        require_write: bool,
    ) -> Option<(ClientScope, u64, CartOperationGuard)> {
        if self.busy.try_get_untracked() != Some(false) {
            return None;
        }
        let scope = self.scope.try_get_untracked()?;
        let generation = self.generation.try_get_untracked()?;
        let allowed = || {
            (!require_write
                || self
                    .state
                    .try_get_untracked()
                    .is_some_and(|session| session.permits_write()))
                && self.current_generation(&scope, generation)
        };
        if !allowed() {
            return None;
        }
        let id = self.operation.try_update(CartOperationState::begin)??;
        let guard = CartOperationGuard {
            state: self.operation,
            id,
            finished: false,
        };
        if !allowed() {
            guard.finish();
            return None;
        }
        Some((scope, generation, guard))
    }
    fn finish_and_retry(&self, guard: CartOperationGuard) {
        if guard.finish() {
            // Releasing the guard notifies observers; its owner may already be gone.
            let _ = self
                .retry
                .try_update(|retry| *retry = retry.saturating_add(1));
        }
    }
    fn checkpoint(&self, scope: &ClientScope) -> Result<BuyerCartCheckpoint, BuyerCartError> {
        let key = checkpoint_key(scope)?;
        if let Some(checkpoint) = self
            .volatile
            .try_with_value(|entries| entries.get(&key).cloned())
            .ok_or(BuyerCartError::Restore)?
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
        self.volatile
            .try_update_value(|entries| {
                entries.insert(key.clone(), checkpoint.clone());
            })
            .ok_or(BuyerCartError::Restore)?;
        save_browser_checkpoint(scope, &checkpoint)?;
        self.volatile
            .try_update_value(|entries| {
                entries.remove(&key);
            })
            .ok_or(BuyerCartError::Restore)?;
        Ok(())
    }
    /// Owner read only, never a repeat of an unconfirmed mutation.
    pub fn inspect(&self) {
        let Some((scope, generation, guard)) = self.begin_operation(false) else {
            return;
        };
        let Some(session) = self.state.try_get_untracked() else {
            guard.finish();
            return;
        };
        if !session.can_inspect() {
            guard.finish();
            return;
        }
        crate::ui::bridge::publish_count(None);
        if !self.current_generation(&scope, generation) {
            self.finish_and_retry(guard);
            return;
        }
        let _ = self.inspection_scope.try_set(Some(scope));
        let _ = self
            .state
            .try_set(BuyerCartSession::restoring(session.cart_id));
        let _ = self.bound_scope.try_set(None);
        let _ = self
            .generation
            .try_update(|generation| *generation = generation.saturating_add(1));
        // The restore effect must run after this synchronous recovery reservation is released.
        self.finish_and_retry(guard);
    }
    /// Explicit local reset does not delete a Cart or cancel an operation whose outcome is unknown.
    pub fn reset_reference(&self) {
        let Some((scope, generation, guard)) = self.begin_operation(false) else {
            return;
        };
        if self
            .save_checkpoint(&scope, BuyerCartCheckpoint::default())
            .is_err()
        {
            if self.current_generation(&scope, generation) {
                let _ = self
                    .state
                    .try_update(|session| session.fail(BuyerCartError::Restore));
            }
            guard.finish();
            return;
        }
        if !self.current_generation(&scope, generation) {
            self.finish_and_retry(guard);
            return;
        }
        let persisted = self.persist_reference.try_run(None) == Some(true);
        if !self.current_generation(&scope, generation) {
            self.finish_and_retry(guard);
            return;
        }
        if !persisted {
            let _ = self
                .state
                .try_update(|session| session.fail(BuyerCartError::Restore));
            guard.finish();
            return;
        }
        crate::ui::bridge::publish_count(None);
        if !self.current_generation(&scope, generation) {
            self.finish_and_retry(guard);
            return;
        }
        let _ = self.initialized.try_set_value(true);
        let _ = self.inspection_scope.try_set(None);
        let _ = self.reference.try_set(None);
        let _ = self.state.try_set(BuyerCartSession::restoring(None));
        let _ = self.bound_scope.try_set(None);
        let _ = self
            .generation
            .try_update(|generation| *generation = generation.saturating_add(1));
        self.finish_and_retry(guard);
    }
    pub async fn add_variant(
        &self,
        variant_id: String,
        context: PricingResolutionContext,
        locale: Option<String>,
    ) -> bool {
        let Some((scope, generation, guard)) = self.begin_operation(true) else {
            return false;
        };
        let result = self
            .start_add(&scope, generation, variant_id, context, locale)
            .await;
        if !self.current_generation(&scope, generation) {
            // The original pending marker remains. A completed old operation can release only
            // its own guard, never a newer operation's lock or a disposed owner's signals.
            self.finish_and_retry(guard);
            return false;
        }
        let succeeded = match result {
            Ok(()) => true,
            Err(error) => {
                self.state.update(|session| session.fail(error));
                false
            }
        };
        guard.finish() && succeeded
    }

    async fn start_add(
        &self,
        scope: &ClientScope,
        generation: u64,
        variant_id: String,
        context: PricingResolutionContext,
        locale: Option<String>,
    ) -> Result<(), BuyerCartError> {
        if self.checkpoint(scope)?.mutation_pending {
            return Err(BuyerCartError::Unconfirmed);
        }
        let session = self
            .state
            .try_get_untracked()
            .ok_or(BuyerCartError::Restore)?;
        let plan = session.plan_add(variant_id.clone(), context.clone(), locale.clone())?;
        if !self.current_generation(scope, generation) {
            return Err(BuyerCartError::Restore);
        }
        // Persist uncertainty BEFORE IO. A reload/auth round-trip cannot become an automatic retry.
        self.save_checkpoint(
            scope,
            BuyerCartCheckpoint {
                cart_id: session.cart_id,
                mutation_pending: true,
            },
        )?;
        crate::ui::bridge::publish_count(None);
        if !self.current_generation(scope, generation) {
            return Err(BuyerCartError::Unconfirmed);
        }
        self.run_add(scope, generation, plan, variant_id, context, locale)
            .await
    }
    /// Decrement/remove share the same pending checkpoint and scope fence as Product adds.
    /// A successful write is not a new count until a subsequent owner read succeeds.
    pub async fn change_line(
        &self,
        line_id: String,
        action: BuyerCartLineAction,
        locale: Option<String>,
    ) -> bool {
        let Some((scope, generation, guard)) = self.begin_operation(true) else {
            return false;
        };
        let result = self
            .start_line_change(&scope, generation, line_id, action, locale)
            .await;
        if !self.current_generation(&scope, generation) {
            self.finish_and_retry(guard);
            return false;
        }
        let succeeded = match result {
            Ok(restored) => {
                self.state.set(restored);
                true
            }
            Err(error) => {
                self.state.update(|session| session.fail(error));
                false
            }
        };
        guard.finish() && succeeded
    }

    async fn start_line_change(
        &self,
        scope: &ClientScope,
        generation: u64,
        line_id: String,
        action: BuyerCartLineAction,
        locale: Option<String>,
    ) -> Result<BuyerCartSession, BuyerCartError> {
        let checkpoint = self.checkpoint(scope)?;
        if checkpoint.mutation_pending {
            return Err(BuyerCartError::Unconfirmed);
        }
        let session = self
            .state
            .try_get_untracked()
            .ok_or(BuyerCartError::Restore)?;
        let plan = session.plan_line_change(line_id, action)?;
        let cart_id = session.cart_id.clone().ok_or(BuyerCartError::Restore)?;
        if checkpoint.cart_id.as_ref() != Some(&cart_id)
            || !self.current_generation(scope, generation)
        {
            return Err(BuyerCartError::Restore);
        }
        self.save_checkpoint(
            scope,
            BuyerCartCheckpoint {
                cart_id: Some(cart_id.clone()),
                mutation_pending: true,
            },
        )?;
        crate::ui::bridge::publish_count(None);
        // Publishing invalidation synchronously invokes page listeners. They may change or
        // dispose the host before this scheduled command reaches its transport.
        if !self.current_generation(scope, generation) {
            return Err(BuyerCartError::Unconfirmed);
        }
        match plan {
            BuyerCartLinePlan::Decrement(request) => {
                crate::transport::decrement_line_item(request).await
            }
            BuyerCartLinePlan::Remove(request) => crate::transport::remove_line_item(request).await,
        }
        .map_err(|_| BuyerCartError::Unconfirmed)?;
        if !self.current_generation(scope, generation) {
            return Err(BuyerCartError::Unconfirmed);
        }
        let cart = crate::transport::fetch_cart(CartFetchRequest {
            selected_cart_id: Some(cart_id),
            locale,
        })
        .await
        .map_err(|_| BuyerCartError::Unconfirmed)?
        .cart
        .ok_or(BuyerCartError::Unconfirmed)?;
        if !self.current_generation(scope, generation) {
            return Err(BuyerCartError::Unconfirmed);
        }
        let restored = session
            .finish_restore(Some(cart))
            .map_err(|_| BuyerCartError::Unconfirmed)?;
        self.save_checkpoint(
            scope,
            BuyerCartCheckpoint {
                cart_id: restored.cart_id.clone(),
                mutation_pending: false,
            },
        )
        .map_err(|_| BuyerCartError::Unconfirmed)?;
        Ok(restored)
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
            if !self.current_generation(scope, generation) {
                return Err(BuyerCartError::Unconfirmed);
            }
            self.reference.set(created.cart_id.clone());
            self.state.set(created.clone());
            if self.persist_reference.try_run(created.cart_id.clone()) != Some(true)
                || saved.is_err()
                || !self.current_generation(scope, generation)
            {
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
        if !self.current_generation(scope, generation) {
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
    capability: CartCapability,
    persist_reference: Callback<Option<String>, bool>,
    persist_selection: Callback<String, Option<String>>,
) -> BuyerCartController {
    let auth = use_context::<rustok_auth_ui::ui::leptos::AuthContext>();
    let auth_for_stamp = auth.clone();
    let auth_scope = Signal::derive(move || {
        auth_for_stamp.as_ref().map_or_else(
            || rustok_auth_ui::model::ClientAuthScope::new(false),
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
        auth: auth_scope
            .try_get()
            .unwrap_or_else(|| rustok_auth_ui::model::ClientAuthScope::new(false)),
        capability: capability.get(),
    });
    let external_auth_changed = RwSignal::new(false);
    let auth_for_refusal = auth.clone();
    let auth_refused = Signal::derive(move || {
        external_auth_changed.try_get().unwrap_or(true)
            || auth_for_refusal.as_ref().is_none_or(|auth| {
                let Some(session) = auth.session().try_get() else {
                    return true;
                };
                auth.get_tenant()
                    .is_some_and(|tenant| tenant != tenant_slug)
                    || (session.is_some() && auth.is_token_expired())
                    || !rustok_auth_ui::ui::leptos::storage::matches_request_tenant(&tenant_slug)
            })
    });
    let auth_loading = Signal::derive(move || {
        !auth_scope.try_get().is_some_and(|scope| scope.is_ready())
            || auth
                .as_ref()
                .is_none_or(|auth| auth.is_loading().try_get().unwrap_or(true))
    });
    let state = RwSignal::new(BuyerCartSession::restoring(cart_id.clone()));
    let bound_scope = RwSignal::new(None::<ClientScope>);
    let session = Signal::derive(move || {
        if !capability.get().enabled {
            let mut hidden = BuyerCartSession::restoring(None);
            hidden.phase = BuyerCartPhase::Unavailable;
            return hidden;
        }
        if auth_refused.try_get().unwrap_or(true) {
            let mut hidden = BuyerCartSession::restoring(None);
            hidden.phase = BuyerCartPhase::AuthRefused;
            return hidden;
        }
        let Some(current_scope) = scope.try_get() else {
            return BuyerCartSession::restoring(None);
        };
        if auth_loading.try_get().unwrap_or(true)
            || bound_scope.try_get().flatten().as_ref() != Some(&current_scope)
            || auth_scope.try_get() != Some(current_scope.auth)
            || capability.get() != current_scope.capability
            || rustok_auth_ui::ui::leptos::storage::get_token() != current_scope.token
        {
            return BuyerCartSession::restoring(None);
        }
        state
            .try_get()
            .unwrap_or_else(|| BuyerCartSession::restoring(None))
    });
    let operation = RwSignal::new(CartOperationState::default());
    let busy = Signal::derive(move || operation.try_get().is_none_or(|state| state.is_busy()));
    let controller = BuyerCartController {
        session,
        state,
        busy,
        operation,
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
                    let persisted = saved.is_ok()
                        && controller
                            .persist_reference
                            .try_run(restored.cart_id.clone())
                            == Some(true);
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

    fn init_test_executor() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            any_spawner::Executor::init_futures_executor()
                .expect("one native executor for Cart owner tests");
        });
    }

    #[test]
    fn guard_cancellation_poisoning_and_stale_completion_are_owner_scoped() {
        init_test_executor();
        let owner = Owner::new();
        owner.with(|| {
            let state = RwSignal::new(CartOperationState::default());
            let first = state
                .try_update(CartOperationState::begin)
                .flatten()
                .unwrap();
            let old = CartOperationGuard {
                state,
                id: first,
                finished: false,
            };
            assert!(old.finish());
            let second = state
                .try_update(CartOperationState::begin)
                .flatten()
                .unwrap();
            let stale = CartOperationGuard {
                state,
                id: first,
                finished: false,
            };
            assert!(!stale.finish());
            assert!(state.get_untracked().is_busy());
            let pending = CartOperationGuard {
                state,
                id: second,
                finished: false,
            };
            drop(pending);
            assert!(state.get_untracked().is_busy());
            assert_eq!(state.try_update(CartOperationState::begin), Some(None));
        });
    }

    #[test]
    fn recovery_and_mutations_share_one_owner_reservation() {
        init_test_executor();
        let owner = Owner::new();
        owner.with(|| {
            provide_context(rustok_auth_ui::ui::leptos::AuthContext::new());
            let controller = use_buyer_cart(
                None,
                Some("en".into()),
                "shop".into(),
                CartCapability::new(true),
                Callback::new(|_| true),
                Callback::new(Some),
            );
            controller
                .bound_scope
                .set(Some(controller.scope.get_untracked()));
            controller.state.set(BuyerCartSession::empty());
            let (_, _, guard) = controller.begin_operation(true).unwrap();
            assert!(controller.begin_operation(false).is_none());
            assert!(controller.begin_operation(true).is_none());
            controller.reset_reference();
            assert_eq!(controller.busy.get_untracked(), true);
            assert!(guard.finish());

            controller
                .state
                .update(|session| session.fail(BuyerCartError::Restore));
            assert!(controller.begin_operation(true).is_none());
            let (_, _, recovery) = controller.begin_operation(false).unwrap();
            assert!(controller.begin_operation(true).is_none());
            assert!(recovery.finish());
        });
    }

    #[test]
    fn disposed_owner_refuses_late_guards_and_controller_reads() {
        init_test_executor();
        let owner = Owner::new();
        let (guard, controller) = owner.with(|| {
            provide_context(rustok_auth_ui::ui::leptos::AuthContext::new());
            let state = RwSignal::new(CartOperationState::default());
            let id = state
                .try_update(CartOperationState::begin)
                .flatten()
                .unwrap();
            let guard = CartOperationGuard {
                state,
                id,
                finished: false,
            };
            let controller = use_buyer_cart(
                None,
                Some("en".into()),
                "shop".into(),
                CartCapability::new(false),
                Callback::new(|_| false),
                Callback::new(|_| None),
            );
            (guard, controller)
        });
        owner.cleanup();
        assert!(!guard.finish());
        assert!(!controller.context_ready());
        assert!(!controller.quote_context_ready_untracked());
        assert_eq!(controller.generation(), u64::MAX);
        assert!(!controller.can_write());
        assert_eq!(controller.confirmed_item_count(), None);
    }

    #[test]
    fn disabled_cart_can_persist_selection_and_returns_the_effective_host_url() {
        init_test_executor();
        let owner = Owner::new();
        owner.with(|| {
            provide_context(rustok_auth_ui::ui::leptos::AuthContext::new());
            let controller = use_buyer_cart(
                None,
                Some("en".into()),
                "shop".into(),
                CartCapability::new(false),
                Callback::new(|_| false),
                Callback::new(|_: String| Some("/products?cart_id=current&quantity=2".into())),
            );
            assert!(!controller.can_write());
            assert_eq!(
                controller.persist_selection("/products?cart_id=old&quantity=2".into()),
                Some("/products?cart_id=current&quantity=2".into())
            );
        });
    }

    #[test]
    fn selection_callback_cannot_acknowledge_a_replaced_capability() {
        init_test_executor();
        let owner = Owner::new();
        owner.with(|| {
            provide_context(rustok_auth_ui::ui::leptos::AuthContext::new());
            let capability = CartCapability::new(false);
            let controller = use_buyer_cart(
                None,
                Some("en".into()),
                "shop".into(),
                capability,
                Callback::new(|_| false),
                Callback::new(move |href: String| {
                    capability.replace(false);
                    Some(href)
                }),
            );
            assert!(controller.quote_context_ready());
            assert_eq!(controller.persist_selection("/products".into()), None);
        });
    }

    #[test]
    fn selection_callback_cannot_acknowledge_a_disposed_owner() {
        init_test_executor();
        let callback_owner = Owner::new();
        let owner = Owner::new();
        let owner_to_dispose = owner.clone();
        let callback = callback_owner.with(|| {
            Callback::new(move |href: String| {
                owner_to_dispose.cleanup();
                Some(href)
            })
        });
        owner.with(|| {
            provide_context(rustok_auth_ui::ui::leptos::AuthContext::new());
            let controller = use_buyer_cart(
                None,
                Some("en".into()),
                "shop".into(),
                CartCapability::new(false),
                Callback::new(|_| false),
                callback,
            );
            assert!(controller.quote_context_ready());
            assert_eq!(controller.persist_selection("/products".into()), None);
            assert!(!controller.quote_context_ready());
        });
    }

    #[test]
    fn disabled_cart_does_not_hide_public_quotes_but_stale_host_does() {
        init_test_executor();
        let owner = Owner::new();
        owner.with(|| {
            provide_context(rustok_auth_ui::ui::leptos::AuthContext::new());
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
                Callback::new(|_| None),
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
