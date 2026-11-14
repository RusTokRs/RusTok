use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local;
#[cfg(target_arch = "wasm32")]
use leptos_use::use_interval_fn;

use super::storage;
use crate::model::{AuthError, AuthSession, AuthUser, ClientAuthScope};
use crate::transport;

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
            && self.realm.as_ref().is_some_and(|realm| realm.is_current())
    }

    fn complete(mut self, verified: bool) -> bool {
        if !self.is_current() {
            return false;
        }
        let announced = notify_scope_change(&self.source, self.identity_change);
        // Dispatch is synchronous: a listener can dispose the owner or invalidate the lease.
        // Drop then poisons the old lease rather than verifying a different scope.
        if !self.is_current() {
            return false;
        }
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
    source: String,
}

impl AuthContext {
    pub fn new() -> Self {
        let user = RwSignal::new(storage::load_user().ok());
        let session = RwSignal::new(storage::load_session().ok());
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
            source: provider_id(),
        }
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
        self.scope
            .try_get()
            .unwrap_or_else(|| ClientAuthScope::new(false))
    }

    /// Untracked version for checking an asynchronous operation before publishing its result.
    pub fn client_scope_untracked(&self) -> ClientAuthScope {
        self.scope
            .try_get_untracked()
            .unwrap_or_else(|| ClientAuthScope::new(false))
    }

    pub fn scope_source(&self) -> &str {
        &self.source
    }

    fn begin_scope_transition(&self, identity_change: bool) -> Result<ScopeTransition, AuthError> {
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
        if !notify_scope_change(&self.source, identity_change) {
            // No request has started. A listener may have disposed or changed the scope.
            // Release only this lease and never finish a different transition.
            let _ = realm.release();
            let _ = self.scope.try_update(|scope| {
                if *scope == started_scope {
                    scope.finish(false);
                } else {
                    scope.abandon();
                }
            });
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
        self.loading_state.set(true);
        self.error_state.set(None);
        if !transition.is_current() {
            self.loading_state.set(false);
            return Err(AuthError::Network);
        }
        let result = transport::sign_in(email, password, tenant).await;
        if !transition.is_current() {
            self.loading_state.set(false);
            return Err(AuthError::Network);
        }

        match result {
            Ok((user, session)) => {
                let _ = storage::save_user(&user);
                let _ = storage::save_session(&session);
                self.user_state.set(Some(user));
                self.session_state.set(Some(session));
                self.loading_state.set(false);
                if transition.complete(true) {
                    Ok(())
                } else {
                    Err(AuthError::Network)
                }
            }
            Err(e) => {
                self.loading_state.set(false);
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
        let transition = self.begin_scope_transition(true)?;
        self.loading_state.set(true);
        self.error_state.set(None);
        if !transition.is_current() {
            self.loading_state.set(false);
            return Err(AuthError::Network);
        }
        let result = transport::sign_up(email, password, name, tenant).await;
        if !transition.is_current() {
            self.loading_state.set(false);
            return Err(AuthError::Network);
        }

        match result {
            Ok((user, session)) => {
                let _ = storage::save_user(&user);
                let _ = storage::save_session(&session);
                self.user_state.set(Some(user));
                self.session_state.set(Some(session));
                self.loading_state.set(false);
                if transition.complete(true) {
                    Ok(())
                } else {
                    Err(AuthError::Network)
                }
            }
            Err(e) => {
                self.loading_state.set(false);
                self.error_state.set(Some(format!("{}", e)));
                transition.complete(false);
                Err(e)
            }
        }
    }

    pub async fn sign_out(&self) -> Result<(), AuthError> {
        self.sign_out_with(transport::sign_out).await
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
        let _ = self.error_state.try_update(|error| *error = None);
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
                storage::clear_session();
                // Browser storage calls can be synchronous integration boundaries.
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
                let _ = self
                    .error_state
                    .try_update(|slot| *slot = Some(error.to_string()));
            }
        }
        let _ = self.loading_state.try_update(|loading| *loading = false);
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
            let _ = self.loading_state.try_update(|loading| *loading = false);
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
        let _ = storage::save_session(&new_session);
        let _ = storage::save_user(&new_user);
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
        self.user.try_get().flatten().is_some()
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
                    if auth
                        .loading_state
                        .try_update(|loading| *loading = true)
                        .is_none()
                    {
                        return;
                    }
                    let _ = auth.restore_session().await;
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
