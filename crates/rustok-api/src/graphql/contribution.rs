/// Manifest-backed description of a typed GraphQL contribution.
///
/// The deployment build script consumes these symbolic paths to generate
/// compile-time `MergedObject`/`MergedSubscription` composition. The host
/// therefore never maps capability slugs to GraphQL types by hand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphqlContributionDescriptor {
    pub query: Option<&'static str>,
    pub mutation: Option<&'static str>,
    pub subscription: Option<&'static str>,
    pub runtime_data_factory: Option<&'static str>,
}

impl GraphqlContributionDescriptor {
    pub const fn new(
        query: Option<&'static str>,
        mutation: Option<&'static str>,
        subscription: Option<&'static str>,
        runtime_data_factory: Option<&'static str>,
    ) -> Self {
        Self {
            query,
            mutation,
            subscription,
            runtime_data_factory,
        }
    }
}
