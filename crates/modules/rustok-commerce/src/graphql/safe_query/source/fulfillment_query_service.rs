use std::sync::Arc;

use super::FulfillmentResult;
use super::fulfillment_query_boundary::{fulfillment_query_context, map_fulfillment_port_error};
use ::rustok_fulfillment::{
    FindLatestFulfillmentByOrderProjectionRequest, FulfillmentReadPort, FulfillmentResponse,
    ListFulfillmentProjectionsRequest, ListFulfillmentsInput, ReadFulfillmentProjectionRequest,
};
use ::sea_orm::DatabaseConnection;
use ::uuid::Uuid;

pub struct FulfillmentService {
    fulfillment_reads: Arc<dyn FulfillmentReadPort>,
}

impl FulfillmentService {
    pub fn new(db: DatabaseConnection) -> Self {
        let fulfillment_lifecycle_runtime =
            crate::graphql_runtime::fulfillment_lifecycle_read_runtime_for_current_graphql_scope(
                db,
            );
        Self {
            fulfillment_reads: fulfillment_lifecycle_runtime.fulfillment_read_port(),
        }
    }

    pub async fn get_fulfillment(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let context = fulfillment_query_context(
            tenant_id,
            "fulfillment",
            "read_fulfillment_projection",
            Some(id),
            None,
        );
        self.fulfillment_reads
            .read_fulfillment_projection(
                context.clone(),
                ReadFulfillmentProjectionRequest { fulfillment_id: id },
            )
            .await
            .map_err(|error| {
                map_fulfillment_port_error(
                    error,
                    &context,
                    "fulfillment",
                    "read_fulfillment_projection",
                    Some(id),
                    None,
                )
            })
    }

    pub async fn list_fulfillments(
        &self,
        tenant_id: Uuid,
        input: ListFulfillmentsInput,
    ) -> FulfillmentResult<(Vec<FulfillmentResponse>, u64)> {
        let ListFulfillmentsInput {
            page,
            per_page,
            status,
            order_id,
            customer_id,
        } = input;
        let context = fulfillment_query_context(
            tenant_id,
            "fulfillments",
            "list_fulfillment_projections",
            None,
            order_id,
        );
        let page_result = self
            .fulfillment_reads
            .list_fulfillment_projections(
                context.clone(),
                ListFulfillmentProjectionsRequest {
                    page,
                    per_page,
                    status,
                    order_id,
                    customer_id,
                },
            )
            .await
            .map_err(|error| {
                map_fulfillment_port_error(
                    error,
                    &context,
                    "fulfillments",
                    "list_fulfillment_projections",
                    None,
                    order_id,
                )
            })?;
        Ok((page_result.items, page_result.total))
    }

    pub async fn find_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> FulfillmentResult<Option<FulfillmentResponse>> {
        let context = fulfillment_query_context(
            tenant_id,
            "order",
            "find_latest_fulfillment_by_order_projection",
            None,
            Some(order_id),
        );
        self.fulfillment_reads
            .find_latest_fulfillment_by_order_projection(
                context.clone(),
                FindLatestFulfillmentByOrderProjectionRequest { order_id },
            )
            .await
            .map_err(|error| {
                map_fulfillment_port_error(
                    error,
                    &context,
                    "order",
                    "find_latest_fulfillment_by_order_projection",
                    None,
                    Some(order_id),
                )
            })
    }
}
