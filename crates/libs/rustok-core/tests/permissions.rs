use rustok_api::{Action, Permission, Resource};
use std::str::FromStr;

#[test]
fn permission_display_format() {
    let permission = Permission::new(Resource::Products, Action::Read);
    assert_eq!(permission.to_string(), "products:read");
}

#[test]
fn permission_parse_from_str() {
    let permission = Permission::from_str("orders:update").unwrap();
    assert_eq!(permission, Permission::ORDERS_UPDATE);
}
