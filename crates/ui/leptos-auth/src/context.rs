use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local;
#[cfg(target_arch = "wasm32")]
use leptos_use::use_interval_fn;

use crate::storage;
use crate::transport;
use crate::{AuthError, AuthSession, AuthUser, ClientAuthScope};

/// Lifecycle-only notification. The detail contains a random provider identifier and whether
/// the operation may change identity; it never carries a session, user, token or tenant.
pub const AUTH_SCOPE_CHANGE_EVENT: &str = "rustok:auth:scope-change";
/// A non-secret invalidation stamp, so another tab can refuse its stale in-memory session.
pub const AUTH_SCOPE_CHANGE_STORAGE_KEY: &str = "rustok-auth-scope-change";

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

fn notify_scope_change(source: &str, identity_change: bool) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return false;
        };
        let detail = format!(
            "{source}:{}",
            if identity_change {
                "identity"
            } else {
                "refresh"
            }
        );
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&leptos::wasm_bindgen::JsValue::from_str(&detail));
        let Ok(event) =
            web_sys::CustomEvent::new_with_event_init_dict(AUTH_SCOPE_CHANGE_EVENT, &init)
        else {
            return false;
        };
        if window.dispatch_event(&event).is_err() {
            return false;
        }
        // Refresh can rotate a shared HttpOnly cookie too. Other tabs cannot prove which
        // identity it now belongs to, so they must invalidate rather than reuse old projections.
        let Ok(Some(storage)) = window.local_storage() else {
            return false;
        };
        let stamp = format!("{source}:{}", js_sys::Math::random());
        storage
            .set_item(AUTH_SCOPE_CHANGE_STORAGE_KEY, &stamp)
            .is_ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (source, identity_change);
        true
    }
}

struct ScopeTransition {
    scope: RwSignal<ClientAuthScope>,
    started_scope: ClientAuthScope,
    realm: Option<crate::realm::RealmTransition>,
    source: String,
    identity_change: bool,
    completed: bool,
}

impl ScopeTransition {
    /// Check both before sending and before publishing: an event handler may synchronously
    /// dispose this provider or replace the shared in-document lease.
    fn is_current(&self) -> bool {
        self.scope.try_get_untracked() == Some(self.started_scope)
            && self.realm.as_ref().is_some_and(|realm| realm.is_current())
    }

    fn complete(mut self, verified: bool) -> bool {
        let announced = notify_scope_change(&self.source, self.identity_change);
        let released = self.realm.take().is_some_and(|realm| realm.release());
        self.completed = true;
        let updated = self
            .scope
            .try_update(|scope| scope.finish(verified && announced && released))
            .is_some();
        announced && released && updated
    }
}

impl Drop for ScopeTransition {
    fn drop(&mut self) {
        if !self.completed {
            // Dropping a future does not cancel a request or its cookie side effects. Poison
            // the document lease as well as this provider before notifying other consumers.
            if let Some(realm) = self.realm.as_mut() {
                realm.poison();
            }
            let _ = self.scope.try_update(ClientAuthScope::abandon);
            let _ = notify_scope_change(&self.source, self.identity_change);
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
    pub user: RwSignal<Option<AuthUser>>,
    pub session: RwSignal<Option<AuthSession>>,
    pub is_loading: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
    scope: RwSignal<ClientAuthScope>,
    source: String,
}

impl AuthContext {
    pub fn new() -> Self {
        let user = RwSignal::new(storage::load_user().ok());
        let session = RwSignal::new(storage::load_session().ok());
        let is_loading = RwSignal::new(cfg!(target_arch = "wasm32"));
        let error = RwSignal::new(None);

        Self {
            user,
            session,
            is_loading,
            error,
            scope: RwSignal::new(ClientAuthScope::new(!cfg!(target_arch = "wasm32"))),
            source: provider_id(),
        }
    }

    /// Tracked, credential-free scope stamp. Consumers must still authorize server-side.
    pub fn client_scope(&self) -> ClientAuthScope {
        self.scope.get()
    }

    /// Untracked version for checking an asynchronous operation before publishing its result.
    pub fn client_scope_untracked(&self) -> ClientAuthScope {
        self.scope.get_untracked()
    }

    pub fn scope_source(&self) -> &str {
        &self.source
    }

    fn begin_scope_transition(&self, identity_change: bool) -> Result<ScopeTransition, AuthError> {
        let mut started = false;
        self.scope.update(|scope| started = scope.begin());
        if !started {
            return Err(AuthError::Http(409));
        }
        let Some(started_scope) = self.scope.try_get_untracked() else {
            return Err(AuthError::Network);
        };
        let realm = match crate::realm::RealmTransition::acquire(&self.source) {
            Ok(realm) => realm,
            Err(error) => {
                self.scope.update(|scope| scope.finish(false));
                return Err(error);
            }
        };
        if !notify_scope_change(&self.source, identity_change) {
            // No request has started. Release this lease but leave the provider unverified.
            let _ = realm.release();
            self.scope.update(|scope| scope.finish(false));
            return Err(AuthError::Network);
        }
        Ok(ScopeTransition {
            scope: self.scope,
            started_scope,
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
        let transition = self.begin_scope_transition(true)?;
        self.is_loading.set(true);
        self.error.set(None);
        if !transition.is_current() {
            self.is_loading.set(false);
            return Err(AuthError::Network);
        }
        let result = transport::sign_in(email, password, tenant).await;
        if !transition.is_current() {
            self.is_loading.set(false);
            return Err(AuthError::Network);
        }

        match result {
            Ok((user, session)) => {
                let _ = storage::save_user(&user);
                let _ = storage::save_session(&session);
                self.user.set(Some(user));
                self.session.set(Some(session));
                self.is_loading.set(false);
                if transition.complete(true) {
                    Ok(())
                } else {
                    Err(AuthError::Network)
                }
            }
            Err(e) => {
                self.is_loading.set(false);
                self.error.set(Some(format!("{}", e)));
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
        let transition = self.begin_scope_transition(true)?;
        self.is_loading.set(true);
        self.error.set(None);
        if !transition.is_current() {
            self.is_loading.set(false);
            return Err(AuthError::Network);
        }
        let result = transport::sign_up(email, password, name, tenant).await;
        if !transition.is_current() {
            self.is_loading.set(false);
            return Err(AuthError::Network);
        }

        match result {
            Ok((user, session)) => {
                let _ = storage::save_user(&user);
                let _ = storage::save_session(&session);
                self.user.set(Some(user));
                self.session.set(Some(session));
                self.is_loading.set(false);
                if transition.complete(true) {
                    Ok(())
                } else {
                    Err(AuthError::Network)
                }
            }
            Err(e) => {
                self.is_loading.set(false);
                self.error.set(Some(format!("{}", e)));
                transition.complete(false);
                Err(e)
            }
        }
    }

    pub async fn sign_out(&self) -> Result<(), AuthError> {
        let transition = self.begin_scope_transition(true)?;
        self.is_loading.set(true);
        if !transition.is_current() {
            self.is_loading.set(false);
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
            transport::sign_out(tenant).await
        } else {
            Err(AuthError::Unauthorized)
        };
        if !transition.is_current() {
            self.is_loading.set(false);
            return Err(AuthError::Network);
        }

        storage::clear_session();
        self.user.set(None);
        self.session.set(None);
        // A failed logout may have left the server cookie intact. Do not silently
        // treat that uncertain identity as an established guest session.
        self.is_loading.set(false);
        if transition.complete(result.is_ok()) {
            result
        } else {
            Err(AuthError::Network)
        }
    }

    pub fn is_token_expired(&self) -> bool {
        self.session
            .get()
            .map(|s| s.is_expired(now_unix_secs()))
            .unwrap_or(true)
    }

    pub fn secs_until_expiry(&self) -> i64 {
        self.session
            .get()
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
        let result = transport::refresh_session(tenant).await;
        if !transition.is_current() {
            return Err(AuthError::Network);
        }
        let (new_session, new_user) = match result {
            Ok(value) => value,
            Err(error) => {
                // Only a definite absent cookie without a previous session establishes guest.
                let guest = previous_session.is_none() && matches!(&error, AuthError::Unauthorized);
                transition.complete(guest);
                return Err(error);
            }
        };
        let _ = storage::save_session(&new_session);
        let _ = storage::save_user(&new_user);
        self.session.set(Some(new_session));
        self.user.set(Some(new_user));
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
                let _ = storage::save_user(&user);
                self.user.set(Some(user));
                if transition.complete(true) {
                    Ok(())
                } else {
                    Err(AuthError::Network)
                }
            }
            Ok(None) => {
                self.user.set(None);
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
        self.user.get().is_some() && self.session.get().is_some() && !self.is_token_expired()
    }

    pub fn get_token(&self) -> Option<String> {
        self.session.get().map(|s| s.token)
    }

    pub fn get_tenant(&self) -> Option<String> {
        self.session.get().map(|s| s.tenant)
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
        auth_context.error.set(Some(error.to_string()));
        auth_context.scope.update(|scope| scope.set_verified(false));
        auth_context.is_loading.set(false);
    }

    provide_context(auth_context.clone());

    #[cfg(target_arch = "wasm32")]
    if binding.is_ok() {
        // Credentials are never read from browser storage: restore the
        // session from the server-held HttpOnly cookie (silent
        // refresh), keeping the loading flag up so protected routes
        // do not flash the login form first.
        let auth_for_init = auth_context.clone();
        Effect::new(move |previous: Option<()>| {
            if previous.is_none() {
                let auth = auth_for_init.clone();
                spawn_local(async move {
                    auth.is_loading.set(true);
                    let _ = auth.restore_session().await;
                    auth.is_loading.set(false);
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
                    if secs <= 0 {
                        let _ = auth.sign_out().await;
                    } else if secs < 300 {
                        let _ = auth.refresh_session().await;
                    }
                });
            },
            60_000,
        );
    }

    children()
}
