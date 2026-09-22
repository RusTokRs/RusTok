use rustok_api::{Action, Permission, Resource};
use rustok_core::{Rbac, UserRole};

#[test]
fn manage_implies_specific_action() {
    let permission = Permission::new(Resource::Products, Action::Read);
    assert!(Rbac::has_permission(&UserRole::Admin, &permission));
}
