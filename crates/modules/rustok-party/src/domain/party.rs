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

use super::tax_details::{
    TaxDetails, validate_inn_10, validate_inn_12, validate_kpp, validate_ogrn_13,
    validate_ogrnip_15,
};
use crate::error::{PartyError, PartyResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        if tenant_id.is_nil() {
            return Err(PartyError::Validation("Tenant ID cannot be nil".into()));
        }

        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err(PartyError::Validation("Party name cannot be empty".into()));
        }

        let legal_name = legal_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        if let Some(ref details) = tax_details {
            validate_tax_details_for_kind(kind, details)?;
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
        if self.kind == PartyKind::LegalEntity {
            let Some(ref details) = self.tax_details else {
                return Err(PartyError::Validation(
                    "Legal entities must have verified tax details before activation".into(),
                ));
            };
            validate_tax_details_for_kind(self.kind, details)?;
        } else if let Some(ref details) = self.tax_details {
            validate_tax_details_for_kind(self.kind, details)?;
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

fn validate_tax_details_for_kind(kind: PartyKind, details: &TaxDetails) -> PartyResult<()> {
    let inn = details.inn.trim();
    let has_vat = details
        .vat_code
        .as_deref()
        .is_some_and(|code| !code.trim().is_empty());

    if inn.is_empty() && !has_vat {
        return Err(PartyError::Validation(
            "Tax details must include an INN or VAT code".into(),
        ));
    }

    if !inn.is_empty() || !has_vat {
        match kind {
            PartyKind::LegalEntity if !has_vat => {
                validate_inn_10(inn)?;
                if let Some(ref kpp) = details.kpp {
                    validate_kpp(kpp.trim())?;
                }
                if let Some(ref ogrn) = details.ogrn {
                    validate_ogrn_13(ogrn.trim())?;
                }
            }
            PartyKind::SoleProprietor if !has_vat => {
                validate_inn_12(inn)?;
                if let Some(ref ogrnip) = details.ogrn {
                    validate_ogrnip_15(ogrnip.trim())?;
                }
            }
            PartyKind::Individual if !inn.is_empty() => {
                validate_inn_12(inn)?;
            }
            _ => {}
        }
    }

    Ok(())
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
        )
        .unwrap();

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

    #[test]
    fn test_party_rejects_nil_tenant_and_empty_tax_details() {
        assert!(Party::new(Uuid::nil(), PartyKind::Individual, "John Doe", None, None).is_err());

        let mut entity = Party::new(
            Uuid::new_v4(),
            PartyKind::LegalEntity,
            "Empty Tax Entity",
            None,
            None,
        )
        .unwrap();
        entity.tax_details = Some(TaxDetails::default());
        assert!(entity.activate().is_err());
    }
}
