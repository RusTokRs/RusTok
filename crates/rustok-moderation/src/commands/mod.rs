mod case_assign;
mod case_decide;
mod case_open;
mod report;

use serde::Serialize;

use crate::error::ModerationResult;
use crate::receipts::{complete, rollback, NewModerationReceipt};

pub(crate) async fn finish<R>(
    receipt: NewModerationReceipt,
    result: ModerationResult<R>,
) -> ModerationResult<R>
where
    R: Clone + Serialize,
{
    match result {
        Ok(response) => complete(receipt, &response).await,
        Err(error) => rollback(receipt, error).await,
    }
}
