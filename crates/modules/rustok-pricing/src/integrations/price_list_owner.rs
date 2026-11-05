use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::ports::PriceListOwnerPort;
use crate::services::PriceListOwnerService;

pub fn in_process_price_list_owner_port(db: DatabaseConnection) -> Arc<dyn PriceListOwnerPort> {
    Arc::new(PriceListOwnerService::new(db))
}
