use crate::builder::PagesBuilderFacade;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn pages_builder_facade_is_send_sync() {
    assert_send_sync::<PagesBuilderFacade>();
}
