mod graphql_adapter;
mod native_server_adapter;

use std::future::Future;

use crate::api::ApiError;
use crate::core::StorefrontPricingQuery;
use crate::model::StorefrontPricingData;

pub(crate) async fn fetch_storefront_pricing(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ApiError> {
    fetch_with_native_first_fallback(
        query,
        native_server_adapter::fetch_storefront_pricing,
        graphql_adapter::fetch_storefront_pricing,
    )
    .await
}

async fn fetch_with_native_first_fallback<N, NFut, G, GFut>(
    query: StorefrontPricingQuery,
    native_fetch: N,
    graphql_fetch: G,
) -> Result<StorefrontPricingData, ApiError>
where
    N: FnOnce(StorefrontPricingQuery) -> NFut,
    NFut: Future<Output = Result<StorefrontPricingData, ApiError>>,
    G: FnOnce(StorefrontPricingQuery) -> GFut,
    GFut: Future<Output = Result<StorefrontPricingData, ApiError>>,
{
    match native_fetch(query.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => graphql_fetch(query).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PricingProductList;
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
    fn native_first_facade_returns_native_success_without_graphql() {
        let result = block_on(fetch_with_native_first_fallback(
            sample_query(),
            |_| async { Ok(sample_data("native")) },
            |_| async { panic!("GraphQL fallback must not run after native success") },
        ))
        .expect("native success should be returned");

        assert_eq!(result.selected_handle.as_deref(), Some("native"));
    }

    #[test]
    fn native_first_facade_falls_back_to_graphql_with_original_query() {
        let result = block_on(fetch_with_native_first_fallback(
            sample_query(),
            |_| async { Err(ApiError::ServerFn("native unavailable".to_string())) },
            |query| async move {
                assert_eq!(query.selected_handle.as_deref(), Some("sample"));
                assert_eq!(query.locale.as_deref(), Some("en"));
                assert_eq!(query.currency_code.as_deref(), Some("EUR"));
                Ok(sample_data("graphql"))
            },
        ))
        .expect("graphql fallback should recover native errors");

        assert_eq!(result.selected_handle.as_deref(), Some("graphql"));
    }
}
