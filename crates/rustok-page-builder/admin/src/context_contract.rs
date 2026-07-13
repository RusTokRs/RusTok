use crate::{PageBuilderAdminHostContext, PageBuilderAdminFacade};
use std::sync::Arc;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn host_context_and_shared_facade_are_send_sync() {
    assert_send_sync::<PageBuilderAdminHostContext>();
    assert_send_sync::<Arc<dyn PageBuilderAdminFacade>>();
}
