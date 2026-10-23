/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Party aggregate root, legal types, and corporate invariants.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{PartyError, PartyResult};
use super::tax_details::TaxDetails;

/// Legal form/kind of the counterparty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PartyKind {
    /// Commercial or non-profit corporate legal entity (ООО, АО, ПАО, Ltd, Corp).
    #[default]
    LegalEntity,
    /// Registered individual entrepreneur / sole proprietor (ИП).
    SoleProprietor,
    /// Private individual buyer or contractor (Физлицо).
    Individual,
}

/// Lifecycle status for counterparty accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PartyStatus {
    #[default]
    Draft,
    Active,
    Archived,
}

impl PartyStatus {
    /// Validates whether a state transition is permitted according to domain invariants.
    pub fn can_transition_to(&self, target: Self) -> bool {
        match (self, target) {
            (Self::Draft, Self::Active) => true,
            (Self::Active, Self::Archived) => true,
            (Self::Archived, Self::Draft) => true,
            (current, target) if *current == target => true,
            _ => false,
        }
    }

    /// Attempts state transition with typed domain invariant guard.
    pub fn transition_to(&mut self, target: Self) -> PartyResult<()> {
        if !self.can_transition_to(target) {
            return Err(PartyError::Validation(format!(
                "Invalid state transition from {:?} to {:?}",
                self, target
            )));
        }
        *self = target;
        Ok(())
    }
}

/// Master counterparty aggregate root representing customers, suppliers, partners, or vendors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Party {
    /// Unique identifier for this counterparty.
    pub id: Uuid,
    /// Tenant boundary owning this counterparty.
    pub tenant_id: Uuid,
    /// Classification of the counterparty legal form.
    pub kind: PartyKind,
    /// Display or trading name (e.g. "Ромашка Трейд").
    pub name: String,
    /// Full official registered legal name (e.g. "ООО «Ромашка Трейдинг Групп»").
    pub legal_name: Option<String>,
    /// Current operational status.
    pub status: PartyStatus,
    /// Structured corporate tax identification codes (ИНН, КПП, ОГРН).
    pub tax_details: Option<TaxDetails>,
    /// Official registered legal address.
    pub legal_address: Option<String>,
    /// Physical warehouse, office, or shipping address.
    pub actual_address: Option<String>,
    /// Primary business contact email.
    pub email: Option<String>,
    /// Primary business contact telephone number.
    pub phone: Option<String>,
    /// Timestamp when this master record was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of last modification.
    pub updated_at: DateTime<Utc>,
}

impl Party {
    /// Constructs and validates a new counterparty aggregate in Draft status.
    pub fn new(
        tenant_id: Uuid,
        kind: PartyKind,
        name: impl Into<String>,
        legal_name: Option<String>,
        tax_details: Option<TaxDetails>,
    ) -> PartyResult<Self> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err(PartyError::Validation("Party name cannot be empty".into()));
        }

        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            tenant_id,
            kind,
            name,
            legal_name,
            status: PartyStatus::Draft,
            tax_details,
            legal_address: None,
            actual_address: None,
            email: None,
            phone: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Activates counterparty for commercial operations, verifying required master data.
    pub fn activate(&mut self) -> PartyResult<()> {
        if self.kind == PartyKind::LegalEntity && self.tax_details.is_none() {
            return Err(PartyError::Validation(
                "Legal entities must have verified tax details before activation".into(),
            ));
        }

        self.status.transition_to(PartyStatus::Active)?;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Archives counterparty, preventing new orders or deals while preserving audit trail.
    pub fn archive(&mut self) -> PartyResult<()> {
        self.status.transition_to(PartyStatus::Archived)?;
        self.updated_at = Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_party_creation_and_activation_guard() {
        let tenant_id = Uuid::new_v4();
        let mut legal_entity = Party::new(
            tenant_id,
            PartyKind::LegalEntity,
            "Ромашка",
            Some("ООО Ромашка".into()),
            None,
        ).unwrap();

        assert_eq!(legal_entity.status, PartyStatus::Draft);

        // Cannot activate legal entity without tax details
        assert!(legal_entity.activate().is_err());

        // Attach tax details and retry
        legal_entity.tax_details = Some(TaxDetails {
            inn: "7707083893".into(),
            kpp: Some("773601001".into()),
            ogrn: Some("1027700132195".into()),
            vat_code: None,
            okpo: None,
        });

        assert!(legal_entity.activate().is_ok());
        assert_eq!(legal_entity.status, PartyStatus::Active);
    }
}
