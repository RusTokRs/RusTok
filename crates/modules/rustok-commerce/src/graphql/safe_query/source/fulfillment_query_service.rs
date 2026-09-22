pub struct FulfillmentService {
    shipping_option_reads: Arc<dyn ShippingOptionReadPort>,
    shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>,
    fulfillment_reads: Arc<dyn FulfillmentReadPort>,
}

impl FulfillmentService {
    pub fn new(db: DatabaseConnection) -> Self {
        let shipping_option_runtime =
            crate::graphql_runtime::shipping_option_read_runtime_for_current_graphql_scope(
                db.clone(),
            );
        let fulfillment_lifecycle_runtime =
            crate::graphql_runtime::fulfillment_lifecycle_read_runtime_for_current_graphql_scope(
                db,
            );
        Self {
            shipping_option_reads: shipping_option_runtime.shipping_option_read_port(),
            shipping_option_admin_reads: shipping_option_runtime.shipping_option_admin_read_port(),
            fulfillment_reads: fulfillment_lifecycle_runtime.fulfillment_read_port(),
        }
    }

    pub async fn get_shipping_option(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        let context = shipping_option_query_context(
            tenant_id,
            "shipping_option",
            Some(id),
            requested_locale,
            tenant_default_locale,
        );
        self.shipping_option_reads
            .read_shipping_option_projection(
                context.clone(),
                ReadShippingOptionProjectionRequest {
                    shipping_option_id: id,
                    requested_locale: requested_locale.map(str::to_owned),
                    tenant_default_locale: tenant_default_locale.map(str::to_owned),
                },
            )
            .await
            .map_err(|error| {
                map_shipping_option_lookup_port_error(
                    error,
                    &context,
                    "shipping_option",
                    "read_shipping_option_projection",
                    id,
                    requested_locale,
                    tenant_default_locale,
                )
            })
    }

    pub async fn list_shipping_options(
        &self,
        tenant_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> Result<Vec<ShippingOptionResponse>, BoundaryError> {
        let context = shipping_option_query_context(
            tenant_id,
            "storefront_shipping_options",
            None,
            requested_locale,
            tenant_default_locale,
        );
        self.shipping_option_reads
            .list_shipping_option_projections(
                context.clone(),
                ListShippingOptionProjectionsRequest {
                    requested_locale: requested_locale.map(str::to_owned),
                    tenant_default_locale: tenant_default_locale.map(str::to_owned),
                },
            )
            .await
            .map_err(|error| {
                map_shipping_option_port_error(
                    error,
                    &context,
                    "storefront_shipping_options",
                    "list_shipping_option_projections",
                    None,
                    requested_locale,
                    tenant_default_locale,
                )
            })
    }

    pub async fn list_all_shipping_options(
        &self,
        tenant_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> Result<Vec<ShippingOptionResponse>, ShippingOptionAdminQueryError> {
        let context = shipping_option_query_context(
            tenant_id,
            "shipping_options",
            None,
            requested_locale,
            tenant_default_locale,
        );
        self.shipping_option_admin_reads
            .list_all_shipping_option_projections(
                context.clone(),
                ListAllShippingOptionProjectionsRequest {
                    requested_locale: requested_locale.map(str::to_owned),
                    tenant_default_locale: tenant_default_locale.map(str::to_owned),
                },
            )
            .await
            .map_err(|error| {
                ShippingOptionAdminQueryError(map_shipping_option_port_error(
                    error,
                    &context,
                    "shipping_options",
                    "list_all_shipping_option_projections",
                    None,
                    requested_locale,
                    tenant_default_locale,
                ))
            })
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
