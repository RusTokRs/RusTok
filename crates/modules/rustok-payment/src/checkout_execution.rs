use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{
    AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput,
    PaymentCollectionResponse, PaymentCollectionStatusKind,
};
use crate::providers::{
    MANUAL_PAYMENT_PROVIDER_ID, PaymentProviderOperationRequest, PaymentProviderOperationResult,
    PaymentProviderRegistry,
};
use crate::{
    BeginProviderOperation, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED, PaymentError,
    PaymentProviderOperationJournal, PaymentService,
};

const PREPARE_CHECKOUT_COLLECTION_OPERATION: &str = "prepare_checkout_collection";
const AUTHORIZE_CHECKOUT_COLLECTION_OPERATION: &str = "authorize_checkout_collection";
const CAPTURE_CHECKOUT_COLLECTION_OPERATION: &str = "capture_checkout_collection";
const READ_CHECKOUT_COLLECTION_OPERATION: &str = "read_checkout_collection";

mod types;
mod diagnostic_safety;
mod prepare_authorize;
mod capture_provider;
mod provider_helpers;
mod port_impl;
mod validation_identity;
mod validation_errors;

pub use types::*;
pub use port_impl::in_process_checkout_payment_execution_port;

use diagnostic_safety::*;
use prepare_authorize::*;
use capture_provider::*;
use provider_helpers::*;
use validation_identity::*;
use validation_errors::*;