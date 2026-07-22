mod graphql_adapter;
mod native_server_adapter;

use crate::core::StorefrontPricingQuery;
use crate::model::StorefrontPricingData;
use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};

pub(crate) type TransportResult<T> = UiTransportResult<T>;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub(crate) async fn fetch_storefront_pricing(
    query: StorefrontPricingQuery,
) -> TransportResult<StorefrontPricingData> {
    let native_query = query.clone();
    execute_selected_transport(
        "pricing",
        selected_transport_path(),
        move || native_server_adapter::fetch_storefront_pricing(native_query),
        move || graphql_adapter::fetch_storefront_pricing(query),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PricingProductList;
    use native_server_adapter::ApiError;
    use rustok_ui_transport::UiTransportPath;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);

        loop {
            match Pin::new(&mut future).poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn noop_waker() -> Waker {
        unsafe fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        unsafe fn wake(_: *const ()) {}
        unsafe fn wake_by_ref(_: *const ()) {}
        unsafe fn drop(_: *const ()) {}

        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);

        unsafe { Waker::from_raw(raw_waker) }
    }

    fn sample_query() -> StorefrontPricingQuery {
        StorefrontPricingQuery {
            selected_handle: Some("sample".to_string()),
            locale: Some("en".to_string()),
            currency_code: Some("EUR".to_string()),
            ..StorefrontPricingQuery::default()
        }
    }

    fn sample_data(handle: &str) -> StorefrontPricingData {
        StorefrontPricingData {
            products: PricingProductList {
                items: Vec::new(),
                total: 0,
                page: 1,
                per_page: 8,
                has_next: false,
            },
            selected_product: None,
            selected_handle: Some(handle.to_string()),
            resolution_context: None,
            available_channels: Vec::new(),
            active_price_lists: Vec::new(),
        }
    }

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }

    #[test]
    fn shared_selected_transport_helper_returns_native_success_without_graphql() {
        let query = sample_query();
        let native_query = query.clone();
        let result = block_on(execute_selected_transport(
            "pricing",
            UiTransportPath::NativeServer,
            move || async move {
                assert_eq!(native_query.selected_handle.as_deref(), Some("sample"));
                Ok::<_, ApiError>(sample_data("native"))
            },
            move || async move {
                let _ = query;
                panic!("GraphQL transport must not run in native mode");
                #[allow(unreachable_code)]
                Err::<StorefrontPricingData, ApiError>(ApiError::Graphql("unreachable".into()))
            },
        ))
        .expect("native success should be returned");

        assert_eq!(result.selected_handle.as_deref(), Some("native"));
    }

    #[test]
    fn shared_selected_transport_helper_keeps_selected_path_error_evidence() {
        let error = block_on(execute_selected_transport(
            "pricing",
            UiTransportPath::NativeServer,
            || async {
                Err::<StorefrontPricingData, _>(ApiError::ServerFn(
                    "native unavailable".to_string(),
                ))
            },
            || async {
                Err::<StorefrontPricingData, _>(ApiError::Graphql(
                    "graphql unavailable".to_string(),
                ))
            },
        ))
        .expect_err("both paths should fail");

        assert_eq!(error.failed_path, UiTransportPath::NativeServer);
        assert!(!error.fallback_attempted);
        assert_eq!(error.native_error.as_deref(), Some("native unavailable"));
        assert_eq!(error.graphql_error, None);
    }

    #[test]
    fn shared_selected_transport_helper_uses_graphql_with_original_query() {
        let query = sample_query();
        let native_query = query.clone();
        let result = block_on(execute_selected_transport(
            "pricing",
            UiTransportPath::Graphql,
            move || async move {
                assert_eq!(native_query.selected_handle.as_deref(), Some("sample"));
                Err::<StorefrontPricingData, _>(ApiError::ServerFn(
                    "native unavailable".to_string(),
                ))
            },
            move || async move {
                assert_eq!(query.selected_handle.as_deref(), Some("sample"));
                assert_eq!(query.locale.as_deref(), Some("en"));
                assert_eq!(query.currency_code.as_deref(), Some("EUR"));
                Ok::<_, ApiError>(sample_data("graphql"))
            },
        ))
        .expect("graphql transport should return data in graphql mode");

        assert_eq!(result.selected_handle.as_deref(), Some("graphql"));
    }
}
