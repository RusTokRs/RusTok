use std::collections::BTreeSet;
use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_REACTION_SOURCE_SLUG_BYTES: usize = 64;
pub const MAX_REACTION_SUBJECT_KIND_BYTES: usize = 64;
pub const MAX_REACTION_KEY_BYTES: usize = 64;
pub const MAX_REACTION_CATALOG_SIZE: usize = 64;
pub const MAX_REACTIONS_PER_ACTOR: usize = 64;

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum ReactionContractError {
    #[error("reaction source slug is invalid")]
    InvalidSourceSlug,
    #[error("reaction subject kind is invalid")]
    InvalidSubjectKind,
    #[error("reaction key is invalid")]
    InvalidReactionKey,
    #[error("reaction identity contains a nil UUID")]
    NilIdentity,
    #[error("reaction subject revision must be positive")]
    InvalidSubjectRevision,
    #[error("reaction catalog must contain at least one entry")]
    EmptyCatalog,
    #[error("reaction catalog exceeds the bounded size")]
    CatalogTooLarge,
    #[error("reaction catalog contains a duplicate key")]
    DuplicateCatalogKey,
    #[error("reaction selection limit is invalid")]
    InvalidSelectionLimit,
    #[error("actor reaction state exceeds the bounded size")]
    ActorStateTooLarge,
    #[error("actor reaction state contains a duplicate key")]
    DuplicateActorReaction,
    #[error("reaction aggregate contains a duplicate key")]
    DuplicateAggregate,
    #[error("reaction state references a key outside the catalog")]
    KeyOutsideCatalog,
    #[error("reaction subject provider returned a different subject identity")]
    ProviderSubjectMismatch,
}

fn valid_segment(
    value: &str,
    maximum_bytes: usize,
    allow_hyphen: bool,
    allow_underscore: bool,
) -> bool {
    if value.is_empty()
        || value.trim() != value
        || value.len() > maximum_bytes
        || value.starts_with('-')
        || value.starts_with('_')
        || value.ends_with('-')
        || value.ends_with('_')
    {
        return false;
    }

    value.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || (allow_hyphen && byte == b'-')
            || (allow_underscore && byte == b'_')
    })
}

macro_rules! string_key {
    ($name:ident, $max:expr, $allow_hyphen:expr, $allow_underscore:expr, $error:expr) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ReactionContractError> {
                let value = value.into();
                if !valid_segment(&value, $max, $allow_hyphen, $allow_underscore) {
                    return Err($error);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(D::Error::custom)
            }
        }
    };
}

string_key!(
    ReactionSourceSlug,
    MAX_REACTION_SOURCE_SLUG_BYTES,
    true,
    true,
    ReactionContractError::InvalidSourceSlug
);
string_key!(
    ReactionSubjectKind,
    MAX_REACTION_SUBJECT_KIND_BYTES,
    false,
    true,
    ReactionContractError::InvalidSubjectKind
);
string_key!(
    ReactionKey,
    MAX_REACTION_KEY_BYTES,
    true,
    true,
    ReactionContractError::InvalidReactionKey
);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ReactionSelectionPolicy {
    Single,
    Multiple { max_selected: u8 },
}

impl ReactionSelectionPolicy {
    pub fn multiple(max_selected: u8) -> Result<Self, ReactionContractError> {
        if max_selected < 2 || usize::from(max_selected) > MAX_REACTIONS_PER_ACTOR {
            return Err(ReactionContractError::InvalidSelectionLimit);
        }
        Ok(Self::Multiple { max_selected })
    }

    pub const fn maximum_selected(self) -> usize {
        match self {
            Self::Single => 1,
            Self::Multiple { max_selected } => max_selected as usize,
        }
    }

    fn validate(self) -> Result<Self, ReactionContractError> {
        match self {
            Self::Single => Ok(self),
            Self::Multiple { max_selected } => Self::multiple(max_selected),
        }
    }
}

impl<'de> Deserialize<'de> for ReactionSelectionPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "mode", rename_all = "snake_case")]
        enum RawPolicy {
            Single,
            Multiple { max_selected: u8 },
        }

        let policy = match RawPolicy::deserialize(deserializer)? {
            RawPolicy::Single => Self::Single,
            RawPolicy::Multiple { max_selected } => Self::Multiple { max_selected },
        };
        policy.validate().map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReactionCatalog {
    selection: ReactionSelectionPolicy,
    keys: Vec<ReactionKey>,
}

impl ReactionCatalog {
    pub fn try_new(
        selection: ReactionSelectionPolicy,
        keys: Vec<ReactionKey>,
    ) -> Result<Self, ReactionContractError> {
        let selection = selection.validate()?;
        if keys.is_empty() {
            return Err(ReactionContractError::EmptyCatalog);
        }
        if keys.len() > MAX_REACTION_CATALOG_SIZE {
            return Err(ReactionContractError::CatalogTooLarge);
        }
        if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
            return Err(ReactionContractError::DuplicateCatalogKey);
        }
        if selection.maximum_selected() > keys.len() {
            return Err(ReactionContractError::InvalidSelectionLimit);
        }
        Ok(Self { selection, keys })
    }

    pub const fn selection(&self) -> ReactionSelectionPolicy {
        self.selection
    }

    pub fn keys(&self) -> &[ReactionKey] {
        &self.keys
    }

    pub fn contains(&self, key: &ReactionKey) -> bool {
        self.keys.contains(key)
    }
}

impl<'de> Deserialize<'de> for ReactionCatalog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawCatalog {
            selection: ReactionSelectionPolicy,
            keys: Vec<ReactionKey>,
        }

        let raw = RawCatalog::deserialize(deserializer)?;
        Self::try_new(raw.selection, raw.keys).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReactionSubjectRef {
    tenant_id: Uuid,
    source: ReactionSourceSlug,
    kind: ReactionSubjectKind,
    subject_id: Uuid,
    subject_revision: u64,
}

impl ReactionSubjectRef {
    pub fn new(
        tenant_id: Uuid,
        source: ReactionSourceSlug,
        kind: ReactionSubjectKind,
        subject_id: Uuid,
        subject_revision: u64,
    ) -> Result<Self, ReactionContractError> {
        if tenant_id.is_nil() || subject_id.is_nil() {
            return Err(ReactionContractError::NilIdentity);
        }
        if subject_revision == 0 {
            return Err(ReactionContractError::InvalidSubjectRevision);
        }
        Ok(Self {
            tenant_id,
            source,
            kind,
            subject_id,
            subject_revision,
        })
    }

    pub const fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn source(&self) -> &ReactionSourceSlug {
        &self.source
    }

    pub fn kind(&self) -> &ReactionSubjectKind {
        &self.kind
    }

    pub const fn subject_id(&self) -> Uuid {
        self.subject_id
    }

    pub const fn subject_revision(&self) -> u64 {
        self.subject_revision
    }

    pub fn has_same_identity(&self, other: &Self) -> bool {
        self.tenant_id == other.tenant_id
            && self.source == other.source
            && self.kind == other.kind
            && self.subject_id == other.subject_id
    }
}

impl<'de> Deserialize<'de> for ReactionSubjectRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawSubject {
            tenant_id: Uuid,
            source: ReactionSourceSlug,
            kind: ReactionSubjectKind,
            subject_id: Uuid,
            subject_revision: u64,
        }

        let raw = RawSubject::deserialize(deserializer)?;
        Self::new(
            raw.tenant_id,
            raw.source,
            raw.kind,
            raw.subject_id,
            raw.subject_revision,
        )
        .map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionAction {
    Add,
    Remove,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReactionCommandIdentity {
    command_id: Uuid,
    actor_id: Uuid,
}

impl ReactionCommandIdentity {
    pub fn new(command_id: Uuid, actor_id: Uuid) -> Result<Self, ReactionContractError> {
        if command_id.is_nil() || actor_id.is_nil() {
            return Err(ReactionContractError::NilIdentity);
        }
        Ok(Self {
            command_id,
            actor_id,
        })
    }

    pub const fn command_id(&self) -> Uuid {
        self.command_id
    }

    pub const fn actor_id(&self) -> Uuid {
        self.actor_id
    }
}

impl<'de> Deserialize<'de> for ReactionCommandIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawIdentity {
            command_id: Uuid,
            actor_id: Uuid,
        }
        let raw = RawIdentity::deserialize(deserializer)?;
        Self::new(raw.command_id, raw.actor_id).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApplyReactionCommand {
    identity: ReactionCommandIdentity,
    subject: ReactionSubjectRef,
    reaction: ReactionKey,
    action: ReactionAction,
}

impl ApplyReactionCommand {
    pub fn new(
        identity: ReactionCommandIdentity,
        subject: ReactionSubjectRef,
        reaction: ReactionKey,
        action: ReactionAction,
    ) -> Self {
        Self {
            identity,
            subject,
            reaction,
            action,
        }
    }

    pub fn identity(&self) -> &ReactionCommandIdentity {
        &self.identity
    }

    pub fn subject(&self) -> &ReactionSubjectRef {
        &self.subject
    }

    pub fn reaction(&self) -> &ReactionKey {
        &self.reaction
    }

    pub const fn action(&self) -> ReactionAction {
        self.action
    }
}

impl<'de> Deserialize<'de> for ApplyReactionCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawCommand {
            identity: ReactionCommandIdentity,
            subject: ReactionSubjectRef,
            reaction: ReactionKey,
            action: ReactionAction,
        }
        let raw = RawCommand::deserialize(deserializer)?;
        Ok(Self::new(
            raw.identity,
            raw.subject,
            raw.reaction,
            raw.action,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReactionActorState {
    revision: u64,
    selected: Vec<ReactionKey>,
}

impl ReactionActorState {
    pub fn try_new(
        revision: u64,
        selected: Vec<ReactionKey>,
    ) -> Result<Self, ReactionContractError> {
        if selected.len() > MAX_REACTIONS_PER_ACTOR {
            return Err(ReactionContractError::ActorStateTooLarge);
        }
        if selected.iter().collect::<BTreeSet<_>>().len() != selected.len() {
            return Err(ReactionContractError::DuplicateActorReaction);
        }
        Ok(Self { revision, selected })
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn selected(&self) -> &[ReactionKey] {
        &self.selected
    }
}

impl<'de> Deserialize<'de> for ReactionActorState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawState {
            revision: u64,
            selected: Vec<ReactionKey>,
        }
        let raw = RawState::deserialize(deserializer)?;
        Self::try_new(raw.revision, raw.selected).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReactionAggregate {
    pub reaction: ReactionKey,
    pub count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReactionSnapshot {
    subject: ReactionSubjectRef,
    catalog: ReactionCatalog,
    actor_state: Option<ReactionActorState>,
    aggregates: Vec<ReactionAggregate>,
}

impl ReactionSnapshot {
    pub fn try_new(
        subject: ReactionSubjectRef,
        catalog: ReactionCatalog,
        actor_state: Option<ReactionActorState>,
        aggregates: Vec<ReactionAggregate>,
    ) -> Result<Self, ReactionContractError> {
        let mut aggregate_keys = BTreeSet::new();
        for aggregate in &aggregates {
            if !catalog.contains(&aggregate.reaction) {
                return Err(ReactionContractError::KeyOutsideCatalog);
            }
            if !aggregate_keys.insert(aggregate.reaction.clone()) {
                return Err(ReactionContractError::DuplicateAggregate);
            }
        }
        if let Some(state) = &actor_state {
            if state.selected().iter().any(|key| !catalog.contains(key)) {
                return Err(ReactionContractError::KeyOutsideCatalog);
            }
            if state.selected().len() > catalog.selection().maximum_selected() {
                return Err(ReactionContractError::InvalidSelectionLimit);
            }
        }
        Ok(Self {
            subject,
            catalog,
            actor_state,
            aggregates,
        })
    }

    pub fn subject(&self) -> &ReactionSubjectRef {
        &self.subject
    }

    pub fn catalog(&self) -> &ReactionCatalog {
        &self.catalog
    }

    pub fn actor_state(&self) -> Option<&ReactionActorState> {
        self.actor_state.as_ref()
    }

    pub fn aggregates(&self) -> &[ReactionAggregate] {
        &self.aggregates
    }
}

impl<'de> Deserialize<'de> for ReactionSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawSnapshot {
            subject: ReactionSubjectRef,
            catalog: ReactionCatalog,
            actor_state: Option<ReactionActorState>,
            aggregates: Vec<ReactionAggregate>,
        }
        let raw = RawSnapshot::deserialize(deserializer)?;
        Self::try_new(raw.subject, raw.catalog, raw.actor_state, raw.aggregates)
            .map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReactionReadRequest {
    subject: ReactionSubjectRef,
    actor_id: Option<Uuid>,
}

impl ReactionReadRequest {
    pub fn new(
        subject: ReactionSubjectRef,
        actor_id: Option<Uuid>,
    ) -> Result<Self, ReactionContractError> {
        if actor_id.is_some_and(|actor_id| actor_id.is_nil()) {
            return Err(ReactionContractError::NilIdentity);
        }
        Ok(Self { subject, actor_id })
    }

    pub fn subject(&self) -> &ReactionSubjectRef {
        &self.subject
    }

    pub const fn actor_id(&self) -> Option<Uuid> {
        self.actor_id
    }
}

impl<'de> Deserialize<'de> for ReactionReadRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawRequest {
            subject: ReactionSubjectRef,
            actor_id: Option<Uuid>,
        }
        let raw = RawRequest::deserialize(deserializer)?;
        Self::new(raw.subject, raw.actor_id).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReactionWriteReceipt {
    command_id: Uuid,
    actor_id: Uuid,
    subject: ReactionSubjectRef,
    reaction: ReactionKey,
    action: ReactionAction,
    changed: bool,
    actor_state_revision: u64,
}

impl ReactionWriteReceipt {
    pub fn new(
        command_id: Uuid,
        actor_id: Uuid,
        subject: ReactionSubjectRef,
        reaction: ReactionKey,
        action: ReactionAction,
        changed: bool,
        actor_state_revision: u64,
    ) -> Result<Self, ReactionContractError> {
        if command_id.is_nil() || actor_id.is_nil() {
            return Err(ReactionContractError::NilIdentity);
        }
        Ok(Self {
            command_id,
            actor_id,
            subject,
            reaction,
            action,
            changed,
            actor_state_revision,
        })
    }

    pub const fn command_id(&self) -> Uuid {
        self.command_id
    }

    pub const fn changed(&self) -> bool {
        self.changed
    }
}

impl<'de> Deserialize<'de> for ReactionWriteReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawReceipt {
            command_id: Uuid,
            actor_id: Uuid,
            subject: ReactionSubjectRef,
            reaction: ReactionKey,
            action: ReactionAction,
            changed: bool,
            actor_state_revision: u64,
        }
        let raw = RawReceipt::deserialize(deserializer)?;
        Self::new(
            raw.command_id,
            raw.actor_id,
            raw.subject,
            raw.reaction,
            raw.action,
            raw.changed,
            raw.actor_state_revision,
        )
        .map_err(D::Error::custom)
    }
}
