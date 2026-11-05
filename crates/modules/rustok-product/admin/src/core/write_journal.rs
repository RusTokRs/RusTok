/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Product caller-recovery envelope. This is a browser projection, not an owner receipt.
//! Operation states validate their own immutable requests and terminal evidence. The common
//! envelope keeps scope, document/generation identity, bounds and forward-only restoration.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub(crate) const MAX_JOURNAL_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProductWriteScope {
    pub tenant_id: String,
    pub actor_id: String,
    pub product_id: String,
    /// Frozen with the request, not an identity authority; replay retains its original locale.
    pub locale: String,
}

impl ProductWriteScope {
    pub fn same_identity(&self, other: &Self) -> bool {
        self.tenant_id == other.tenant_id
            && self.actor_id == other.actor_id
            && self.product_id == other.product_id
    }

    pub fn valid(&self) -> bool {
        valid_uuid(&self.tenant_id)
            && valid_uuid(&self.actor_id)
            && valid_uuid(&self.product_id)
            && rustok_api::TenantLocale::new(self.locale.as_str()).is_ok()
    }

    pub fn storage_key<S: JournalState>(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            S::STORAGE_PREFIX,
            self.tenant_id,
            self.actor_id,
            self.product_id
        )
    }
}

/// Recovery scope, not an authentication provider. Each resource owns its persisted identity.
pub(crate) trait JournalScope:
    Clone + std::fmt::Debug + PartialEq + Eq + Serialize + DeserializeOwned
{
    fn valid(&self) -> bool;
    fn same_identity(&self, other: &Self) -> bool;
    fn storage_key<S: JournalState>(&self) -> String;
    fn for_locale(&self, locale: String) -> Self;
    /// Dictionary scopes additionally fence the raw URL identity, without changing envelopes.
    fn attribute_document_id(&self) -> Option<&str> {
        None
    }
}

impl JournalScope for ProductWriteScope {
    fn valid(&self) -> bool {
        ProductWriteScope::valid(self)
    }
    fn same_identity(&self, other: &Self) -> bool {
        ProductWriteScope::same_identity(self, other)
    }
    fn storage_key<S: JournalState>(&self) -> String {
        ProductWriteScope::storage_key::<S>(self)
    }
    fn for_locale(&self, locale: String) -> Self {
        Self {
            locale,
            ..self.clone()
        }
    }
}

pub(crate) fn valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| !id.is_nil())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryQueryKey {
    Axes,
    VariantCreate,
    AttributeDefinition,
}

impl RecoveryQueryKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Axes => "axis_recovery",
            Self::VariantCreate => "variant_recovery",
            Self::AttributeDefinition => "attribute_recovery",
        }
    }
}

pub(crate) trait JournalState: Clone + PartialEq + Serialize + DeserializeOwned {
    type Scope: JournalScope;
    const STORAGE_PREFIX: &'static str;
    const DOCUMENT_QUERY: RecoveryQueryKey;
    fn ready() -> Self;
    fn is_ready(&self) -> bool;
    fn is_terminal(&self) -> bool;
    fn valid(&self) -> bool;
    fn matches_scope(&self, _scope: &Self::Scope) -> bool {
        true
    }
    /// Same-generation transitions only: ready -> pending -> exact terminal decision.
    fn can_follow(&self, next: &Self) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(serialize = "S: JournalState", deserialize = "S: JournalState")
)]
pub(crate) struct WriteJournal<S: JournalState> {
    pub scope: S::Scope,
    /// A stable URL marker detects externally removed storage even after a reload.
    pub document_intent: Uuid,
    pub intent: Uuid,
    pub state: S,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriteJournalError {
    Unavailable,
    Invalid,
    Changed,
}

impl<S: JournalState> WriteJournal<S> {
    pub fn new(scope: S::Scope) -> Result<Self, WriteJournalError> {
        if !scope.valid() {
            return Err(WriteJournalError::Invalid);
        }
        Ok(Self {
            scope,
            document_intent: Uuid::new_v4(),
            intent: Uuid::new_v4(),
            state: S::ready(),
        })
    }

    pub fn decode(raw: &str, scope: &S::Scope) -> Result<Self, WriteJournalError> {
        if raw.len() > MAX_JOURNAL_BYTES || !scope.valid() {
            return Err(WriteJournalError::Invalid);
        }
        let record: Self = serde_json::from_str(raw).map_err(|_| WriteJournalError::Invalid)?;
        if !record.scope.same_identity(scope) {
            return Err(WriteJournalError::Changed);
        }
        if !record.scope.valid()
            || record.intent.is_nil()
            || record.document_intent.is_nil()
            || !record.state.valid()
            || !record.state.matches_scope(&record.scope)
        {
            return Err(WriteJournalError::Invalid);
        }
        Ok(record)
    }

    pub fn encode(&self) -> Result<String, WriteJournalError> {
        let raw = serde_json::to_string(self).map_err(|_| WriteJournalError::Invalid)?;
        Self::decode(&raw, &self.scope)?;
        Ok(raw)
    }

    pub fn can_transition_to(&self, next: &Self) -> bool {
        if !self.scope.same_identity(&next.scope)
            || self.document_intent != next.document_intent
            || !next.scope.valid()
            || next.intent.is_nil()
            || !next.state.valid()
            || !next.state.matches_scope(&next.scope)
        {
            return false;
        }
        if self.state.is_terminal() && next.state.is_ready() {
            return self.intent != next.intent;
        }
        self.intent == next.intent
            // Locale can bind at first submission, never after pending has been saved.
            && (self.scope == next.scope || self.state.is_ready())
            && self.state.can_follow(&next.state)
    }

    /// A failed readback may have persisted one forward step. Never adopt another body/key,
    /// a backward transition, or a new generation on the old mounted page.
    pub fn can_restore(&self, live: &Self) -> bool {
        self == live || (self.can_transition_to(live) && !live.state.is_ready())
    }

    pub fn fresh(&self, locale: String) -> Result<Self, WriteJournalError> {
        if !self.state.is_terminal() {
            return Err(WriteJournalError::Changed);
        }
        let mut next = Self::new(self.scope.for_locale(locale))?;
        next.document_intent = self.document_intent;
        Ok(next)
    }

    /// Finish an explicit terminal -> fresh write whose readback failed without rebasing fields.
    pub fn fresh_from_live(&self, live: &Self, locale: String) -> Result<Self, WriteJournalError> {
        if self == live {
            return self.fresh(locale);
        }
        if self.can_transition_to(live) && live.state.is_ready() {
            return Ok(live.clone());
        }
        Err(WriteJournalError::Changed)
    }
}
