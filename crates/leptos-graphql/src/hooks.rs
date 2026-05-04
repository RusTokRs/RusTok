// GraphQL Hooks для Leptos
// Reactive hooks для удобной работы с GraphQL queries и mutations

use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_api::UiRouteContext;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::{execute, GraphqlHttpError, GraphqlRequest};

fn get_locale() -> Option<String> {
    use_context::<UiRouteContext>()
        .and_then(|context| context.locale)
        .map(|locale| locale.trim().to_string())
        .filter(|locale| !locale.is_empty())
}

/// Result структура для use_query hook
#[derive(Clone)]
pub struct QueryResult<T> {
    pub data: ReadSignal<Option<T>>,
    pub error: ReadSignal<Option<GraphqlHttpError>>,
    pub loading: ReadSignal<bool>,
    refetch_trigger: WriteSignal<u32>,
}

impl<T> QueryResult<T> {
    pub fn refetch(&self) {
        self.refetch_trigger.update(|v| *v += 1);
    }
}

/// Hook для выполнения GraphQL query с reactive state
///
/// # Example
/// ```rust,no_run
/// use leptos::prelude::Get;
/// use leptos_graphql::use_query;
/// use serde_json::{json, Value};
///
/// const USERS_QUERY: &str = "query Users($limit: Int!) { users(limit: $limit) { id email } }";
///
/// let token = "demo-token".to_string();
/// let tenant = "demo-tenant".to_string();
///
/// let result = use_query::<Value, Value>(
///     "/api/graphql".to_string(),
///     USERS_QUERY.to_string(),
///     Some(json!({ "limit": 10 })),
///     Some(token),
///     Some(tenant),
/// );
///
/// let _is_loading = result.loading.get();
/// ```
pub fn use_query<V, T>(
    endpoint: String,
    query: String,
    variables: Option<V>,
    token: Option<String>,
    tenant: Option<String>,
) -> QueryResult<T>
where
    V: Serialize + Clone + 'static,
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    let (data, set_data) = signal(None);
    let (error, set_error) = signal(None);
    let (loading, set_loading) = signal(true);
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    Effect::new(move |_| {
        // Trigger refetch when refetch_trigger changes
        let _ = refetch_trigger.get();

        set_loading.set(true);
        set_error.set(None);

        let endpoint = endpoint.clone();
        let query = query.clone();
        let variables = variables.clone();
        let token = token.clone();
        let tenant = tenant.clone();

        spawn_local(async move {
            let request = GraphqlRequest::new(query, variables);

            match execute::<V, T>(&endpoint, request, token, tenant, get_locale()).await {
                Ok(response) => {
                    set_data.set(Some(response));
                    set_loading.set(false);
                }
                Err(err) => {
                    set_error.set(Some(err));
                    set_loading.set(false);
                }
            }
        });
    });

    QueryResult {
        data,
        error,
        loading,
        refetch_trigger: set_refetch_trigger,
    }
}

/// Result структура для use_mutation hook
#[derive(Clone)]
pub struct MutationResult<T> {
    pub data: ReadSignal<Option<T>>,
    pub error: ReadSignal<Option<GraphqlHttpError>>,
    pub loading: ReadSignal<bool>,
    mutate_fn: StoredValue<Arc<dyn Fn(Value) + Send + Sync>>,
}

pub type LazyQueryFetchFn<V> = Box<dyn Fn(Option<V>) + Send + Sync>;

impl<T> MutationResult<T> {
    pub fn mutate(&self, variables: Value) {
        self.mutate_fn.with_value(|f| f(variables));
    }
}

/// Hook для выполнения GraphQL mutation
///
/// # Example
/// ```rust,no_run
/// use leptos_graphql::use_mutation;
/// use serde_json::{json, Value};
///
/// const CREATE_USER_MUTATION: &str =
///     "mutation CreateUser($input: CreateUserInput!) { createUser(input: $input) { id } }";
///
/// let token = "demo-token".to_string();
/// let tenant = "demo-tenant".to_string();
///
/// let create_user = use_mutation::<Value>(
///     "/api/graphql".to_string(),
///     CREATE_USER_MUTATION.to_string(),
///     Some(token),
///     Some(tenant),
/// );
///
/// create_user.mutate(json!({
///     "input": {
///         "email": "user@example.com",
///         "name": "Demo User"
///     }
/// }));
/// ```
pub fn use_mutation<T>(
    endpoint: String,
    mutation: String,
    token: Option<String>,
    tenant: Option<String>,
) -> MutationResult<T>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    let (data, set_data) = signal(None);
    let (error, set_error) = signal(None);
    let (loading, set_loading) = signal(false);

    let mutate_fn = StoredValue::new(Arc::new(move |variables: Value| {
        set_loading.set(true);
        set_error.set(None);

        let endpoint = endpoint.clone();
        let mutation = mutation.clone();
        let token = token.clone();
        let tenant = tenant.clone();

        spawn_local(async move {
            let request = GraphqlRequest::new(mutation, Some(variables));

            match execute::<Value, T>(&endpoint, request, token, tenant, get_locale()).await {
                Ok(response) => {
                    set_data.set(Some(response));
                    set_loading.set(false);
                }
                Err(err) => {
                    set_error.set(Some(err));
                    set_loading.set(false);
                }
            }
        });
    }) as Arc<dyn Fn(Value) + Send + Sync>);

    MutationResult {
        data,
        error,
        loading,
        mutate_fn,
    }
}

/// Lazy query hook - query не выполняется автоматически
///
/// Используйте когда нужно выполнить query по клику или другому event
pub fn use_lazy_query<V, T>(
    endpoint: String,
    query: String,
    token: Option<String>,
    tenant: Option<String>,
) -> (QueryResult<T>, LazyQueryFetchFn<V>)
where
    V: Serialize + Clone + 'static,
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    let (data, set_data) = signal(None);
    let (error, set_error) = signal(None);
    let (loading, set_loading) = signal(false);
    let (_refetch_trigger, set_refetch_trigger) = signal(0u32);

    let fetch: LazyQueryFetchFn<V> = Box::new(move |variables: Option<V>| {
        set_loading.set(true);
        set_error.set(None);

        let endpoint = endpoint.clone();
        let query = query.clone();
        let token = token.clone();
        let tenant = tenant.clone();

        spawn_local(async move {
            let request = GraphqlRequest::new(query, variables);

            match execute::<V, T>(&endpoint, request, token, tenant, get_locale()).await {
                Ok(response) => {
                    set_data.set(Some(response));
                    set_loading.set(false);
                }
                Err(err) => {
                    set_error.set(Some(err));
                    set_loading.set(false);
                }
            }
        });
    });

    let result = QueryResult {
        data,
        error,
        loading,
        refetch_trigger: set_refetch_trigger,
    };

    (result, fetch)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_query_result_creation() {
        // Test that QueryResult can be created
        // (actual functionality requires runtime context)
    }

    #[test]
    fn test_mutation_result_creation() {
        // Test that MutationResult can be created
        // (actual functionality requires runtime context)
    }
}
