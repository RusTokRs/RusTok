use crate::dto::BuilderCapabilityKind;

impl std::fmt::Display for BuilderCapabilityKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
