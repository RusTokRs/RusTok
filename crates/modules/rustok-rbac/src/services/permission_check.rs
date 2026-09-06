use rustok_api::Permission;

#[derive(Debug, Clone, Copy)]
pub enum PermissionCheck<'a> {
    Single(&'a Permission),
    Any(&'a [Permission]),
    All(&'a [Permission]),
}
