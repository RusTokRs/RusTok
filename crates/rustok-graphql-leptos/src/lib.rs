/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local;
pub use rustok_graphql::{
    ACCEPT_LANGUAGE_HEADER, AUTH_HEADER, GRAPHQL_ENDPOINT, GraphqlError, GraphqlHttpError,
    GraphqlRequest, GraphqlResponse, TENANT_HEADER, execute, persisted_query_extension,
};
use rustok_ui_core::UiRouteContext;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

fn get_locale() -> Option<String> {
    use_context::<UiRouteContext>()
        .and_then(|context| context.locale)
        .map(|locale| locale.trim().to_string())
        .filter(|locale| !locale.is_empty())
}

#[derive(Clone)]
pub struct QueryResult<T> {
    pub data: ReadSignal<Option<T>>,
    pub error: ReadSignal<Option<GraphqlHttpError>>,
    pub loading: ReadSignal<bool>,
    refetch_trigger: WriteSignal<u32>,
}

impl<T> QueryResult<T> {
    pub fn refetch(&self) {
        self.refetch_trigger.update(|value| *value += 1);
    }
}

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
                Err(error) => {
                    set_error.set(Some(error));
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
        self.mutate_fn.with_value(|mutation| mutation(variables));
    }
}

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
                Err(error) => {
                    set_error.set(Some(error));
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
                Err(error) => {
                    set_error.set(Some(error));
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
