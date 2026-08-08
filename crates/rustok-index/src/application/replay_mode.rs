use crate::{EntityKey, IndexSourceError, IndexSourceLoadRequest};

/// Operator-visible rebuild intent.
///
/// Mode identity is deliberately separate from locale and future partition scope. A mode selects
/// an execution surface; it does not create another replay ownership or retry state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayMode {
    /// Cursor-based durable source scan through the fenced replay job/checkpoint runner.
    Full,
    /// Bounded exact-key load through `IndexSource::load`.
    Targeted,
    /// Side-effect-free cursor scan through the replay dry-run runtime.
    Shadow,
}

/// Existing execution surface selected by one replay mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayExecutionSurface {
    DurableScan,
    TargetedLoad,
    SideEffectFreeScan,
}

/// Validated mode selection.
///
/// Targeted mode owns the canonical bounded `IndexSourceLoadRequest`, so target count,
/// tenant/schema scope and uniqueness cannot drift from the source contract. Full and shadow
/// carry no target list and therefore cannot accidentally reinterpret exact keys as scan scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexReplayModeSelection {
    Full,
    Targeted(IndexSourceLoadRequest),
    Shadow,
}

impl IndexReplayModeSelection {
    pub fn full() -> Self {
        Self::Full
    }

    pub fn targeted(keys: Vec<EntityKey>) -> Result<Self, IndexSourceError> {
        Ok(Self::Targeted(IndexSourceLoadRequest::new(keys)?))
    }

    pub fn shadow() -> Self {
        Self::Shadow
    }

    pub fn mode(&self) -> IndexReplayMode {
        match self {
            Self::Full => IndexReplayMode::Full,
            Self::Targeted(_) => IndexReplayMode::Targeted,
            Self::Shadow => IndexReplayMode::Shadow,
        }
    }

    pub fn execution_surface(&self) -> IndexReplayExecutionSurface {
        match self {
            Self::Full => IndexReplayExecutionSurface::DurableScan,
            Self::Targeted(_) => IndexReplayExecutionSurface::TargetedLoad,
            Self::Shadow => IndexReplayExecutionSurface::SideEffectFreeScan,
        }
    }

    pub fn targeted_load_request(&self) -> Option<&IndexSourceLoadRequest> {
        match self {
            Self::Targeted(request) => Some(request),
            Self::Full | Self::Shadow => None,
        }
    }

    /// The current `PostgresIndexReplayRunner` remains the durable full-scan executor only.
    /// Targeted and shadow modes must use their separate execution surfaces instead of aliasing
    /// the full job/checkpoint identity.
    pub fn is_admitted_to_durable_scan_runner(&self) -> bool {
        matches!(self, Self::Full)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{EntityName, LocaleKey, ModuleName, SchemaRef, SchemaVersion};

    use super::*;

    fn schema() -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("mode-owner").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn key(entity_id: u128, locale: Option<&str>) -> EntityKey {
        EntityKey {
            tenant_id: Uuid::from_u128(1),
            schema: schema(),
            entity_id: Uuid::from_u128(entity_id),
            locale: locale.map(|value| LocaleKey::new(value).unwrap()),
        }
    }

    #[test]
    fn full_is_the_only_mode_admitted_to_the_existing_durable_scan_runner() {
        let selection = IndexReplayModeSelection::full();
        assert_eq!(selection.mode(), IndexReplayMode::Full);
        assert_eq!(
            selection.execution_surface(),
            IndexReplayExecutionSurface::DurableScan
        );
        assert!(selection.targeted_load_request().is_none());
        assert!(selection.is_admitted_to_durable_scan_runner());
    }

    #[test]
    fn targeted_reuses_the_canonical_bounded_load_request() {
        let selection = IndexReplayModeSelection::targeted(vec![
            key(10, Some("en-US")),
            key(11, Some("de")),
        ])
        .unwrap();
        assert_eq!(selection.mode(), IndexReplayMode::Targeted);
        assert_eq!(
            selection.execution_surface(),
            IndexReplayExecutionSurface::TargetedLoad
        );
        assert_eq!(selection.targeted_load_request().unwrap().keys().len(), 2);
        assert!(!selection.is_admitted_to_durable_scan_runner());
    }

    #[test]
    fn targeted_rejects_empty_duplicate_and_mixed_scope_keys() {
        assert!(IndexReplayModeSelection::targeted(Vec::new()).is_err());
        let duplicate = key(10, None);
        assert!(
            IndexReplayModeSelection::targeted(vec![duplicate.clone(), duplicate]).is_err()
        );

        let mut mixed = key(11, None);
        mixed.tenant_id = Uuid::from_u128(2);
        assert!(IndexReplayModeSelection::targeted(vec![key(10, None), mixed]).is_err());
    }

    #[test]
    fn shadow_routes_only_to_the_side_effect_free_scan_surface() {
        let selection = IndexReplayModeSelection::shadow();
        assert_eq!(selection.mode(), IndexReplayMode::Shadow);
        assert_eq!(
            selection.execution_surface(),
            IndexReplayExecutionSurface::SideEffectFreeScan
        );
        assert!(selection.targeted_load_request().is_none());
        assert!(!selection.is_admitted_to_durable_scan_runner());
    }
}
