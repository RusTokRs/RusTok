/// Trusted authenticated principal classification shared by authorization boundaries.
///
/// This type separates direct interactive users from OAuth-delegated users and
/// service principals. Authorization owners should consume this typed value
/// instead of reinterpreting grant strings, client identifiers, or session shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthPrincipalKind {
    DirectUser,
    DelegatedUser,
    Service,
}
