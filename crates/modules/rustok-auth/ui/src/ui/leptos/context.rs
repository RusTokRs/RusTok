use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local;
#[cfg(target_arch = "wasm32")]
use leptos_use::use_interval_fn;

use super::storage;
use crate::model::{AuthError, AuthSession, AuthUser, ClientAuthScope};
use crate::transport::{self, browser_epoch};

pub use browser_epoch::{AUTH_SCOPE_CHANGE_EVENT, AUTH_SCOPE_CHANGE_STORAGE_KEY};

fn provider_id() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        format!("{}-{}", js_sys::Date::now(), js_sys::Math::random())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}

struct ScopeTransition {
    scope: RwSignal<ClientAuthScope>,
    started_scope: ClientAuthScope,
    stamp: browser_epoch::Epoch,
    previously_current: bool,
    projection_epoch: RwSignal<browser_epoch::Epoch>,
    user: RwSignal<Option<AuthUser>>,
    session: RwSignal<Option<AuthSession>>,
    loading: RwSignal<bool>,
    realm: Option<super::realm::RealmTransition>,
    source: String,
    identity_change: bool,
    completed: bool,
}

impl ScopeTransition {
    /// Check both before sending and before publishing: an event handler may synchronously
    /// dispose this provider or replace the shared in-document lease.
    fn is_current(&self) -> bool {
        self.scope.try_get_untracked() == Some(self.started_scope)
            && browser_epoch::matches(&self.stamp)
            && self.realm.as_ref().is_some_and(|realm| realm.is_current())
    }

    fn complete(mut self, verified: bool) -> bool {
        if !self.is_current() {
            return false;
        }
        let Ok(stamp) = browser_epoch::publish(&self.source, self.identity_change) else {
            return false;
        };
        self.stamp = stamp;
        // Synchronous listeners can dispose the provider. Never publish after
        // disposal, a changed epoch, or a lost document lease.
        if !self.is_current() {
            return false;
        }
        let released = self.realm.take().is_some_and(|realm| realm.release());
        let saved = released && if verified {
            match (self.session.try_get_untracked(), self.user.try_get_untracked()) {
                (Some(Some(session)), Some(Some(user))) =>
                    storage::publish_identity(&session, &user, &self.stamp).is_ok(),
                (Some(None), Some(None)) => { storage::clear_session(); true }
                _ => false,
            }
        } else {
            storage::clear_session();
            true
        };
        // A failed explicit login in a stale tab does not grant permission to
        // subsequently refresh/logout whichever identity another tab installed.
        let epoch_set = if verified && saved || self.previously_current {
            self.projection_epoch.try_update(|epoch| *epoch = self.stamp.clone()).is_some()
        } else {
            self.projection_epoch.try_get_untracked().is_some()
        };
        let ready = verified && saved && epoch_set && browser_epoch::matches(&self.stamp);
        self.completed = true;
        let updated = self.scope.try_update(|scope| scope.finish(ready)).is_some();
        let _ = self.loading.try_update(|loading| *loading = false);
        released && saved && epoch_set && updated && (!verified || ready)
    }
}

impl Drop for ScopeTransition {
    fn drop(&mut self) {
        if !self.completed {
            // Dropping a future does not cancel a request or its cookie side effects. Poison
            // the document lease as well as this provider before notifying other consumers.
            let owns_epoch = browser_epoch::matches(&self.stamp)
                && self.realm.as_ref().is_some_and(|realm| realm.is_current());
            if let Some(realm) = self.realm.as_mut() {
                realm.poison();
            }
            let _ = self.scope.try_update(ClientAuthScope::abandon);
            let _ = self.loading.try_update(|loading| *loading = false);
            if owns_epoch {
                // An abandoned transition must not overwrite a newer tab's epoch.
                storage::clear_session();
                let _ = browser_epoch::publish(&self.source, self.identity_change);
            }
        }
    }
}

fn now_unix_secs() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

#[derive(Clone)]
pub struct AuthContext {
    // The handles stay private too: rebinding a public projection on one clone must not let
    // that clone pair a different session with the provider's verified lifecycle stamp.
    user: ReadSignal<Option<AuthUser>>,
    session: ReadSignal<Option<AuthSession>>,
    is_loading: ReadSignal<bool>,
    error: ReadSignal<Option<String>>,
    user_state: RwSignal<Option<AuthUser>>,
    session_state: RwSignal<Option<AuthSession>>,
    loading_state: RwSignal<bool>,
    error_state: RwSignal<Option<String>>,
    scope: RwSignal<ClientAuthScope>,
    projection_epoch: RwSignal<browser_epoch::Epoch>,
    source: String,
}

impl AuthContext {
    pub fn new() -> Self {
        let observed_epoch = browser_epoch::read().unwrap_or_default();
        let (mut session, mut user) = storage::load_identity();
        if !browser_epoch::matches(&observed_epoch) {
            session = None;
            user = None;
        }
        let user = RwSignal::new(user);
        let session = RwSignal::new(session);
        let is_loading = RwSignal::new(cfg!(target_arch = "wasm32"));
        let error = RwSignal::new(None);

        Self {
            user: user.read_only(),
            session: session.read_only(),
            is_loading: is_loading.read_only(),
            error: error.read_only(),
            user_state: user,
            session_state: session,
            loading_state: is_loading,
            error_state: error,
            scope: RwSignal::new(ClientAuthScope::new(!cfg!(target_arch = "wasm32"))),
            projection_epoch: RwSignal::new(observed_epoch),
            source: provider_id(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn install_epoch_listeners(&self) -> bool {
        use leptos::wasm_bindgen::{JsCast, closure::Closure};
        let Some(window) = web_sys::window() else { return false; };
        let auth = self.clone();
        let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
            // Read the current storage value, not an asynchronously delivered
            // event payload. Ignore this provider's own start/end notifications.
            let current = browser_epoch::read();
            let own = current.as_ref().ok().and_then(|epoch| epoch.as_deref())
                .is_some_and(|stamp| stamp.split_once(':').is_some_and(|(source, _)| source == auth.source));
            if own || auth.epoch_is_current() {
                return;
            }
            let _ = auth.scope.try_update(|scope| scope.set_verified(false));
            let _ = auth.user_state.try_update(|user| *user = None);
            let _ = auth.session_state.try_update(|session| *session = None);
            let _ = auth.loading_state.try_update(|loading| *loading = false);
            let _ = auth.error_state.try_update(|error| *error = Some(AuthError::Http(409).to_string()));
            // Do not adopt the foreign epoch: stale logout/refresh intents
            // remain inadmissible until explicit sign-in or document reload.
        });
        let names = [AUTH_SCOPE_CHANGE_EVENT, "storage", "pageshow", "focus"];
        let mut installed = Vec::new();
        for name in names {
            if window.add_event_listener_with_callback(name, listener.as_ref().unchecked_ref()).is_err() {
                for name in installed {
                    let _ = window.remove_event_listener_with_callback(name, listener.as_ref().unchecked_ref());
                }
                return false;
            }
            installed.push(name);
        }
        let registration = send_wrapper::SendWrapper::new((window, installed, listener));
        on_cleanup(move || {
            let (window, names, listener) = &*registration;
            for name in names {
                let _ = window.remove_event_listener_with_callback(name, listener.as_ref().unchecked_ref());
            }
        });
        true
    }

    /// Read-only handles are created with the provider's owner and remain stable across clones.
    pub fn user(&self) -> ReadSignal<Option<AuthUser>> {
        self.user
    }

    pub fn session(&self) -> ReadSignal<Option<AuthSession>> {
        self.session
    }

    pub fn is_loading(&self) -> ReadSignal<bool> {
        self.is_loading
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error
    }

    /// Tracked, credential-free scope stamp. Consumers must still authorize server-side.
    pub fn client_scope(&self) -> ClientAuthScope {
        let mut scope = self.scope.try_get().unwrap_or_else(|| ClientAuthScope::new(false));
        if !self.projection_epoch.try_get().is_some_and(|epoch| browser_epoch::matches(&epoch)) {
            scope.set_verified(false);
        }
        scope
    }

    /// Untracked version for checking an asynchronous operation before publishing its result.
    pub fn client_scope_untracked(&self) -> ClientAuthScope {
        let mut scope = self.scope.try_get_untracked().unwrap_or_else(|| ClientAuthScope::new(false));
        if !self.epoch_is_current() {
            scope.set_verified(false);
        }
        scope
    }

    pub fn scope_source(&self) -> &str {
        &self.source
    }

    fn epoch_is_current(&self) -> bool {
        self.projection_epoch.try_get_untracked()
            .is_some_and(|epoch| browser_epoch::matches(&epoch))
    }

    /// Hold the origin-wide lock across admission, network and publication.
    /// Explicit sign-in/up can recover a stale document, but logout, renewal
    /// and current-user revalidation must never adopt another tab's identity.
    async fn with_provider_lock<T: 'static>(
        &self,
        require_current: bool,
        operation: impl std::future::Future<Output = Result<T, AuthError>> + 'static,
    ) -> Result<T, AuthError> {
        #[cfg(target_arch = "wasm32")]
        {
            let auth = self.clone();
            let scope = auth.scope.try_get_untracked().ok_or(AuthError::Network)?;
            if scope.is_pending() || (require_current && !auth.epoch_is_current()) {
                return Err(AuthError::Http(409));
            }
            transport::browser_lock::with_cookie_lock(async move {
                if auth.scope.try_get_untracked() != Some(scope)
                    || (require_current && !auth.epoch_is_current()) {
                    return Err(AuthError::Http(409));
                }
                operation.await
            }).await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = require_current;
            operation.await
        }
    }

    fn begin_scope_transition(&self, identity_change: bool) -> Result<ScopeTransition, AuthError> {
        let previously_current = self.epoch_is_current();
        let started = self
            .scope
            .try_update(ClientAuthScope::begin)
            .ok_or(AuthError::Network)?;
        if !started {
            return Err(AuthError::Http(409));
        }
        let Some(started_scope) = self.scope.try_get_untracked() else {
            return Err(AuthError::Network);
        };
        let realm = match super::realm::RealmTransition::acquire(&self.source) {
            Ok(realm) => realm,
            Err(error) => {
                self.scope.update(|scope| {
                    if *scope == started_scope {
                        scope.finish(false);
                    } else {
                        scope.abandon();
                    }
                });
                return Err(error);
            }
        };
        let stamp = match browser_epoch::publish(&self.source, identity_change) {
            Ok(stamp) => stamp,
            Err(error) => {
                // No request has started. A listener may have disposed the owner.
                let _ = realm.release();
                let _ = self.scope.try_update(|scope| {
                    if *scope == started_scope {
                        scope.finish(false);
                    } else {
                        scope.abandon();
                    }
                });
                return Err(error);
            }
        };
        // Start notification invalidates shared memory before any new I/O.
        storage::clear_session();
        let _ = self.loading_state.try_update(|loading| *loading = true);
        Ok(ScopeTransition {
            scope: self.scope,
            started_scope,
            stamp,
            previously_current,
            projection_epoch: self.projection_epoch,
            user: self.user_state,
            session: self.session_state,
            loading: self.loading_state,
            realm: Some(realm),
            source: self.source.clone(),
            identity_change,
            completed: false,
        })
    }

    pub async fn sign_in(
        &self,
        email: String,
        password: String,
        tenant: String,
    ) -> Result<(), AuthError> {
        let auth = self.clone();
        self.with_provider_lock(false, async move {
            auth.sign_in_locked(email, password, tenant).await
        }).await
    }

    async fn sign_in_locked(&self, email: String, password: String, tenant: String) -> Result<(), AuthError> {
        let transition = self.begin_scope_transition(true)?;
        self.error_state.set(None);
        if !transition.is_current() {
            return Err(AuthError::Network);
        }
        let result = transport::sign_in_unlocked(email, password, tenant).await;
        if !transition.is_current() {
            return Err(AuthError::Network);
        }

        match result {
            Ok((user, session)) => {
                self.user_state.set(Some(user));
                self.session_state.set(Some(session));
                if transition.complete(true) {
                    Ok(())
                } else {
                    Err(AuthError::Network)
                }
            }
            Err(e) => {
                self.error_state.set(Some(format!("{}", e)));
                transition.complete(false);
                Err(e)
            }
        }
    }

    pub async fn sign_up(
        &self,
        email: String,
        password: String,
        name: Option<String>,
        tenant: String,
    ) -> Result<(), AuthError> {
        let auth = self.clone();
        self.with_provider_lock(false, async move {
            auth.sign_up_locked(email, password, name, tenant).await
        }).await
    }

    async fn sign_up_locked(&self, email: String, password: String, name: Option<String>, tenant: String) -> Result<(), AuthError> {
        let transition = self.begin_scope_transition(true)?;
        self.error_state.set(None);
        if !transition.is_current() {
            return Err(AuthError::Network);
        }
        let result = transport::sign_up_unlocked(email, password, name, tenant).await;
        if !transition.is_current() {
            return Err(AuthError::Network);
        }

        match result {
            Ok((user, session)) => {
                self.user_state.set(Some(user));
                self.session_state.set(Some(session));
                if transition.complete(true) {
                    Ok(())
                } else {
                    Err(AuthError::Network)
                }
            }
            Err(e) => {
                self.error_state.set(Some(format!("{}", e)));
                transition.complete(false);
                Err(e)
            }
        }
    }

    pub async fn sign_out(&self) -> Result<(), AuthError> {
        let auth = self.clone();
        self.with_provider_lock(true, async move {
            auth.sign_out_with(transport::sign_out_unlocked).await
        }).await
    }

    async fn sign_out_with<F, Fut>(&self, revoke: F) -> Result<(), AuthError>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<(), AuthError>>,
    {
        let transition = self.begin_scope_transition(true)?;
        if self
            .loading_state
            .try_update(|loading| *loading = true)
            .is_none()
        {
            return Err(AuthError::Network);
        }
        self.error_state.update(|error| *error = None);
        if !transition.is_current() {
            return Err(AuthError::Network);
        }

        // The refresh token lives in the server-held cookie, so logout
        // only needs the tenant; it runs even without in-memory state
        // (e.g. right after a reload) to clear that cookie.
        let tenant = self
            .session
            .try_get_untracked()
            .flatten()
            .map(|session| session.tenant)
            .or_else(storage::get_tenant);
        let result = if let Some(tenant) = tenant {
            revoke(tenant).await
        } else {
            Err(AuthError::Unauthorized)
        };
        if !transition.is_current() {
            // A disposed or replaced owner must not clear shared browser identity or
            // write to another transition. Drop poisons the abandoned lease.
            return Err(AuthError::Network);
        }

        match &result {
            Ok(()) => {
                if !transition.is_current() {
                    return Err(AuthError::Network);
                }
                if self.user_state.try_update(|user| *user = None).is_none()
                    || self
                        .session_state
                        .try_update(|session| *session = None)
                        .is_none()
                {
                    return Err(AuthError::Network);
                }
            }
            Err(error) => {
                // Keep the old in-memory identity for an explicit retry. The cookie may
                // still be live, so neither that identity nor guest scope is verified.
                self.error_state.update(|slot| *slot = Some(error.to_string()));
            }
        }
        if transition.complete(result.is_ok()) {
            result
        } else {
            Err(AuthError::Network)
        }
    }

    #[cfg(any(test, target_arch = "wasm32"))]
    fn finish_initial_loading(&self) {
        // Startup may lose a race to an explicit sign-in/out. Its rejected restore
        // must not clear the loading flag belonging to that other transition.
        if self
            .scope
            .try_get_untracked()
            .is_some_and(|scope| !scope.is_pending())
        {
            self.loading_state.update(|loading| *loading = false);
        }
    }

    pub fn is_token_expired(&self) -> bool {
        self.session
            .try_get()
            .flatten()
            .map(|s| s.is_expired(now_unix_secs()))
            .unwrap_or(true)
    }

    pub fn secs_until_expiry(&self) -> i64 {
        self.session
            .try_get()
            .flatten()
            .map(|s| s.secs_until_expiry(now_unix_secs()))
            .unwrap_or(0)
    }

    #[cfg(target_arch = "wasm32")]
    fn secs_until_expiry_untracked(&self) -> i64 {
        self.session
            .try_get_untracked()
            .flatten()
            .map(|s| s.secs_until_expiry(now_unix_secs()))
            .unwrap_or(0)
    }

    pub async fn refresh_session(&self) -> Result<(), AuthError> {
        let auth = self.clone();
        self.with_provider_lock(true, async move { auth.refresh_session_locked().await }).await
    }

    async fn refresh_session_locked(&self) -> Result<(), AuthError> {
        let transition = self.begin_scope_transition(false)?;
        if !transition.is_current() {
            return Err(AuthError::Network);
        }
        let previous_session = self.session.try_get_untracked().flatten();
        let tenant = previous_session
            .as_ref()
            .map(|session| session.tenant.clone())
            .or_else(storage::get_tenant);
        let Some(tenant) = tenant else {
            transition.complete(false);
            return Err(AuthError::Unauthorized);
        };
        let result = transport::refresh_session_unlocked(tenant).await;
        if !transition.is_current() {
            return Err(AuthError::Network);
        }
        let (new_session, new_user) = match result {
            Ok(Some(value)) => value,
            Ok(None) => {
                // An absent native-host cookie establishes guest only when no in-memory
                // identity was already present. A 401 or malformed cookie is not absence.
                let guest = previous_session.is_none();
                return if transition.complete(guest) {
                    Err(AuthError::Unauthorized)
                } else {
                    Err(AuthError::Network)
                };
            }
            Err(error) => {
                return if transition.complete(false) {
                    Err(error)
                } else {
                    Err(AuthError::Network)
                };
            }
        };
        let previous_user = self.user.try_get_untracked().flatten();
        if !refresh_matches_identity(previous_session.as_ref(), previous_user.as_ref(), &new_session, &new_user) {
            self.session_state.set(None);
            self.user_state.set(None);
            // Do not rebind a mismatched cookie on a second refresh attempt.
            // Drop abandons this document's lease; reload or a new provider is required.
            drop(transition);
            return Err(AuthError::Http(409));
        }
        self.session_state.set(Some(new_session));
        self.user_state.set(Some(new_user));
        if transition.complete(true) {
            Ok(())
        } else {
            Err(AuthError::Network)
        }
    }

    /// Restore session state after a page load: revalidate the
    /// in-memory/SSR-provided session when present, otherwise attempt
    /// a silent cookie refresh for the persisted tenant.
    pub async fn restore_session(&self) -> Result<(), AuthError> {
        match self.session.try_get_untracked() {
            Some(Some(_)) => self.fetch_current_user().await,
            Some(None) => self.refresh_session().await,
            None => Err(AuthError::Network),
        }
    }

    pub async fn fetch_current_user(&self) -> Result<(), AuthError> {
        let auth = self.clone();
        self.with_provider_lock(true, async move { auth.fetch_current_user_locked().await }).await
    }

    async fn fetch_current_user_locked(&self) -> Result<(), AuthError> {
        let transition = self.begin_scope_transition(false)?;
        if !transition.is_current() {
            return Err(AuthError::Network);
        }
        let Some(session) = self.session.try_get_untracked().flatten() else {
            transition.complete(false);
            return Err(AuthError::Unauthorized);
        };
        let result =
            transport::fetch_current_user(session.token.clone(), session.tenant.clone()).await;
        if !transition.is_current() {
            return Err(AuthError::Network);
        }
        match result {
            Ok(Some(user)) => {
                if self.user.try_get_untracked().flatten().is_some_and(|previous| previous.id != user.id) {
                    self.user_state.set(None);
                    self.session_state.set(None);
                    drop(transition);
                    return Err(AuthError::Http(409));
                }
                self.user_state.set(Some(user));
                if transition.complete(true) {
                    Ok(())
                } else {
                    Err(AuthError::Network)
                }
            }
            Ok(None) => {
                self.user_state.set(None);
                transition.complete(false);
                Err(AuthError::Unauthorized)
            }
            Err(error) => {
                transition.complete(false);
                Err(error)
            }
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.client_scope().is_ready()
            && self.user.try_get().flatten().is_some()
            && self.session.try_get().flatten().is_some()
            && !self.is_token_expired()
    }

    /// Do not hand a retained, uncertain session to transport consumers as if it were verified.
    pub fn get_token(&self) -> Option<String> {
        if !self.client_scope().is_ready() {
            return None;
        }
        self.session.try_get().flatten().map(|s| s.token)
    }

    pub fn get_tenant(&self) -> Option<String> {
        if !self.client_scope().is_ready() {
            return None;
        }
        self.session.try_get().flatten().map(|s| s.tenant)
    }
}

/// A refresh may update role/permissions, but never silently replace the
/// tenant, subject or session lineage already projected by this provider.
fn refresh_matches_identity(
    previous_session: Option<&AuthSession>,
    previous_user: Option<&AuthUser>,
    next_session: &AuthSession,
    next_user: &AuthUser,
) -> bool {
    match (previous_session, previous_user) {
        (None, None) => true,
        (Some(session), Some(user)) => {
            session.tenant == next_session.tenant && user.id == next_user.id
                && crate::core::session_id_from_access_token(&session.token)
                    .zip(crate::core::session_id_from_access_token(&next_session.token))
                    .is_some_and(|(old, new)| old == new)
        }
        _ => false,
    }
}

impl Default for AuthContext {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
pub fn AuthProvider(
    children: Children,
    #[prop(optional)] tenant_slug: Option<String>,
) -> impl IntoView {
    let binding = tenant_slug
        .as_deref()
        .map(storage::bind_request_tenant)
        .transpose();
    let auth_context = AuthContext::new();
    if let Err(error) = &binding {
        auth_context.error_state.set(Some(error.to_string()));
        auth_context.scope.update(|scope| scope.set_verified(false));
        auth_context.loading_state.set(false);
    }

    provide_context(auth_context.clone());

    #[cfg(target_arch = "wasm32")]
    let listeners_ready = auth_context.install_epoch_listeners();
    #[cfg(target_arch = "wasm32")]
    if !listeners_ready {
        auth_context.scope.update(|scope| scope.set_verified(false));
        auth_context.loading_state.set(false);
        auth_context.error_state.set(Some(AuthError::Network.to_string()));
    }

    #[cfg(target_arch = "wasm32")]
    if binding.is_ok() && listeners_ready {
        // Credentials are never read from browser storage: restore the
        // session from the server-held HttpOnly cookie (silent
        // refresh), keeping the loading flag up so protected routes
        // do not flash the login form first.
        let auth_for_init = auth_context.clone();
        Effect::new(move |previous: Option<()>| {
            if previous.is_none() {
                let auth = auth_for_init.clone();
                spawn_local(async move {
                    if auth
                        .loading_state
                        .try_update(|loading| *loading = true)
                        .is_none()
                    {
                        return;
                    }
                    if let Err(err) = auth.restore_session().await {
                        tracing::debug!(?err, "initial session restore failed or session not present");
                    }
                    auth.finish_initial_loading();
                });
            }
        });

        let auth_for_interval = auth_context.clone();
        use_interval_fn(
            move || {
                let auth = auth_for_interval.clone();
                spawn_local(async move {
                    if auth.session.try_get_untracked().flatten().is_none() {
                        return;
                    }
                    let secs = auth.secs_until_expiry_untracked();
                    // Access expiry does not revoke the independently-lived refresh
                    // session. A sleeping tab must attempt renewal, not log out.
                    if secs < 300 {
                        if let Err(err) = auth.refresh_session().await {
                            tracing::warn!(?err, "proactive session refresh failed");
                        }
                    }
                });
            },
            60_000,
        );
    }

    children()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn invalidated_transition_cannot_verify_a_different_scope() {
        let owner = Owner::new();
        owner.with(|| {
            let auth = AuthContext::new();
            let transition = auth.begin_scope_transition(false).unwrap();
            auth.scope.update(|scope| scope.set_verified(false));
            assert!(!transition.complete(true));
            assert_eq!(auth.client_scope().generation, u64::MAX);
            assert!(!auth.client_scope().is_ready());
            assert!(auth.begin_scope_transition(false).is_err());
        });
    }

    #[test]
    fn disposed_provider_never_verifies_a_late_transition() {
        let owner = Owner::new();
        let (auth, transition) = owner.with(|| {
            let auth = AuthContext::new();
            let transition = auth.begin_scope_transition(false).unwrap();
            (auth, transition)
        });
        owner.cleanup();
        assert!(!transition.complete(true));
        assert!(!auth.client_scope().is_ready());
        assert!(!auth.client_scope_untracked().is_ready());
        assert_eq!(auth.get_token(), None);
        assert!(auth.is_token_expired());
        assert!(auth.begin_scope_transition(false).is_err());
    }

    fn session() -> AuthSession {
        AuthSession {
            token: "access".into(),
            expires_at: i64::MAX,
            tenant: "shop".into(),
        }
    }

    fn user() -> AuthUser {
        AuthUser {
            id: "user".into(),
            email: "user@example.test".into(),
            name: None,
            role: "user".into(),
            permissions: Vec::new(),
        }
    }

    #[test]
    fn failed_logout_retains_identity_for_retry_without_verifying_guest() {
        use futures_util::FutureExt;

        let owner = Owner::new();
        owner.with(|| {
            let auth = AuthContext::new();
            auth.session_state.set(Some(session()));
            auth.user_state.set(Some(user()));
            let auth_for_memo = auth.clone();
            let token = Memo::new(move |_| auth_for_memo.get_token());
            assert_eq!(token.get().as_deref(), Some("access"));
            let failed = auth
                .sign_out_with(|tenant| {
                    assert_eq!(tenant, "shop");
                    std::future::ready(Err(AuthError::Network))
                })
                .now_or_never();
            assert_eq!(failed, Some(Err(AuthError::Network)));
            assert_eq!(auth.session.get_untracked(), Some(session()));
            assert_eq!(auth.user.get_untracked(), Some(user()));
            assert!(auth.error.get_untracked().is_some());
            assert!(!auth.client_scope_untracked().is_ready());
            assert!(!auth.is_authenticated());
            assert_eq!(token.get(), None);
            assert_eq!(auth.get_tenant(), None);
            assert!(!auth.is_loading.get_untracked());

            let retried = auth
                .sign_out_with(|_| std::future::ready(Ok(())))
                .now_or_never();
            assert_eq!(retried, Some(Ok(())));
            assert_eq!(auth.session.get_untracked(), None);
            assert_eq!(auth.user.get_untracked(), None);
            assert_eq!(auth.error.get_untracked(), None);
            assert_eq!(token.get(), None);
            assert_eq!(auth.get_tenant(), None);
            assert!(auth.client_scope_untracked().is_ready());
        });
    }

    #[test]
    fn reactive_credentials_follow_scope_and_session_transitions() {
        let owner = Owner::new();
        owner.with(|| {
            let auth = AuthContext::new();
            auth.session_state.set(Some(session()));
            auth.user_state.set(Some(user()));
            let auth_for_memo = auth.clone();
            let credentials =
                Memo::new(move |_| (auth_for_memo.get_token(), auth_for_memo.get_tenant()));
            assert_eq!(
                credentials.get(),
                (Some("access".into()), Some("shop".into()))
            );
            assert_eq!(auth.session(), auth.clone().session());

            let transition = auth.begin_scope_transition(true).unwrap();
            assert_eq!(credentials.get(), (None, None));
            auth.session_state.set(Some(AuthSession {
                token: "other-access".into(),
                tenant: "other-shop".into(),
                ..session()
            }));
            assert_eq!(credentials.get(), (None, None));
            assert!(transition.complete(true));
            assert_eq!(
                credentials.get(),
                (Some("other-access".into()), Some("other-shop".into()))
            );
        });
    }

    #[test]
    fn refresh_cannot_substitute_tenant_subject_or_session_lineage() {
        let mut old = session();
        old.token = "header.eyJzZXNzaW9uX2lkIjoib25lIn0.signature".into();
        let mut next = old.clone();
        next.token = "header.eyJzZXNzaW9uX2lkIjoib25lIn0.newsignature".into();
        let previous_user = user();
        let mut next_user = previous_user.clone();
        next_user.permissions.push("pages:read".into());
        assert!(refresh_matches_identity(Some(&old), Some(&previous_user), &next, &next_user));
        next_user.id = "other".into();
        assert!(!refresh_matches_identity(Some(&old), Some(&previous_user), &next, &next_user));
        next_user.id = previous_user.id.clone();
        next.tenant = "other".into();
        assert!(!refresh_matches_identity(Some(&old), Some(&previous_user), &next, &next_user));
        next.tenant = old.tenant.clone();
        next.token = "header.eyJzZXNzaW9uX2lkIjoidHdvIn0.signature".into();
        assert!(!refresh_matches_identity(Some(&old), Some(&previous_user), &next, &next_user));
        assert!(!refresh_matches_identity(Some(&old), None, &next, &next_user));
        assert!(refresh_matches_identity(None, None, &next, &next_user));
    }

    #[test]
    fn pending_or_unverified_projection_is_not_authenticated() {
        let owner = Owner::new();
        owner.with(|| {
            let auth = AuthContext::new();
            auth.session_state.set(Some(session()));
            auth.user_state.set(Some(user()));
            assert!(auth.is_authenticated());
            let transition = auth.begin_scope_transition(false).unwrap();
            assert!(!auth.is_authenticated());
            assert!(transition.complete(false));
            assert!(!auth.is_authenticated());
            assert_eq!(auth.get_token(), None);
        });
    }

    #[test]
    fn startup_does_not_clear_another_pending_transitions_loading() {
        let owner = Owner::new();
        owner.with(|| {
            let auth = AuthContext::new();
            let transition = auth.begin_scope_transition(true).unwrap();
            auth.loading_state.set(true);
            auth.finish_initial_loading();
            assert!(auth.is_loading.get_untracked());
            assert!(transition.complete(true));
            auth.finish_initial_loading();
            assert!(!auth.is_loading.get_untracked());
        });
    }

    #[test]
    fn a_logout_response_after_disposal_never_clears_or_publishes() {
        use std::{
            cell::Cell,
            future::{Future, poll_fn},
            rc::Rc,
            task::{Context, Poll, Waker},
        };

        let owner = Owner::new();
        let auth = owner.with(|| {
            let auth = AuthContext::new();
            auth.session_state.set(Some(session()));
            auth.user_state.set(Some(user()));
            auth
        });
        let ready = Rc::new(Cell::new(false));
        let completed = ready.clone();
        let mut logout = Box::pin(auth.sign_out_with(move |_| {
            poll_fn(move |_| {
                if completed.get() {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        }));
        let mut context = Context::from_waker(Waker::noop());
        assert!(logout.as_mut().poll(&mut context).is_pending());
        owner.cleanup();
        ready.set(true);
        assert_eq!(
            logout.as_mut().poll(&mut context),
            Poll::Ready(Err(AuthError::Network))
        );
        assert!(!auth.client_scope_untracked().is_ready());
        assert_eq!(auth.get_token(), None);
    }
}
