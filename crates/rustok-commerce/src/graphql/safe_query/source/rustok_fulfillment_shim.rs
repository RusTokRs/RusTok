use std::sync::Arc;

use ::rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use ::rustok_fulfillment::{
    FindLatestFulfillmentByOrderProjectionRequest, FulfillmentReadPort, FulfillmentResponse,
    ListAllShippingOptionProjectionsRequest, ListFulfillmentProjectionsRequest,
    ListFulfillmentsInput, ListShippingOptionProjectionsRequest, ReadFulfillmentProjectionRequest,
    ReadShippingOptionProjectionRequest, ShippingOptionAdminReadPort, ShippingOptionReadPort,
    ShippingOptionResponse,
};
use ::sea_orm::DatabaseConnection;
use ::uuid::Uuid;

use super::super::query_error_boundary::BoundaryError;

pub(crate) mod error {
    use ::uuid::Uuid;

    use super::BoundaryError;

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    pub enum FulfillmentError {
        ShippingOptionNotFound(Uuid),
        FulfillmentNotFound(Uuid),
        Public(BoundaryError),
    }

    impl FulfillmentError {
        #[allow(clippy::inherent_to_string, clippy::wrong_self_convention)]
        pub(crate) fn to_string(self) -> BoundaryError {
            match self {
                Self::ShippingOptionNotFound(_) | Self::FulfillmentNotFound(_) => {
                    BoundaryError::Public {
                        message: "Fulfillment resource was not found",
                        code: "FULFILLMENT_RESOURCE_NOT_FOUND",
                        retryable: false,
                    }
                }
                Self::Public(error) => error,
            }
        }
    }
}

use error::FulfillmentError;

pub type FulfillmentResult<T> = Result<T, FulfillmentError>;

const GRAPHQL_QUERY_FULFILLMENT_BOUNDARY: &str = "commerce_graphql_query_fulfillment_facade";

pub(crate) struct ShippingOptionAdminQueryError(BoundaryError);

impl ShippingOptionAdminQueryError {
    #[allow(clippy::inherent_to_string, clippy::wrong_self_convention)]
    pub(crate) fn to_string(self) -> BoundaryError {
        self.0
    }
}

include!("fulfillment_query_service.rs");
include!("fulfillment_query_boundary.rs");
