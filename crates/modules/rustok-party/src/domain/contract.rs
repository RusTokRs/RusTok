/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Commercial contracts, credit limits, and deferred payment terms.

use crate::error::{PartyError, PartyResult};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lifecycle state for commercial counterparty contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    #[default]
    Draft,
    Active,
    Suspended,
    Terminated,
}

impl ContractStatus {
    pub fn can_transition_to(&self, target: Self) -> bool {
        match (self, target) {
            (Self::Draft, Self::Active) => true,
            (Self::Active, Self::Suspended) => true,
            (Self::Suspended, Self::Active) => true,
            (Self::Active, Self::Terminated) => true,
            (Self::Suspended, Self::Terminated) => true,
            (current, target) if *current == target => true,
            _ => false,
        }
    }

    pub fn transition_to(&mut self, target: Self) -> PartyResult<()> {
        if !self.can_transition_to(target) {
            return Err(PartyError::Validation(format!(
                "Invalid contract state transition from {:?} to {:?}",
                self, target
            )));
        }
        *self = target;
        Ok(())
    }
}

/// Commercial contract defining credit terms, deferred payment periods, and validity intervals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contract {
    /// Unique identifier for this contract.
    pub id: Uuid,
    /// Tenant scope owning this contract.
    pub tenant_id: Uuid,
    /// Reference to the party bound by this contract.
    pub party_id: Uuid,
    /// Human-readable contract number (e.g. "D-2026/04-01").
    pub contract_number: String,
    /// Descriptive contract title (e.g. "General Supply Agreement 2026").
    pub title: String,
    /// Current lifecycle status.
    pub status: ContractStatus,
    /// Contract start date.
    pub valid_from: NaiveDate,
    /// Contract expiration date (None if indefinite).
    pub valid_to: Option<NaiveDate>,
    /// Maximum allowed commercial credit / debt limit (in tenant currency).
    pub credit_limit: Option<Decimal>,
    /// Payment deferral terms in calendar days (e.g. 14, 30, 45 days after invoice).
    pub payment_term_days: Option<u32>,
}

impl Contract {
    /// Creates and validates a new contract draft.
    pub fn new(
        tenant_id: Uuid,
        party_id: Uuid,
        contract_number: impl Into<String>,
        title: impl Into<String>,
        valid_from: NaiveDate,
        valid_to: Option<NaiveDate>,
        credit_limit: Option<Decimal>,
        payment_term_days: Option<u32>,
    ) -> PartyResult<Self> {
        let contract_number = contract_number.into().trim().to_string();
        let title = title.into().trim().to_string();

        if contract_number.is_empty() {
            return Err(PartyError::Validation(
                "Contract number cannot be empty".into(),
            ));
        }
        if title.is_empty() {
            return Err(PartyError::Validation(
                "Contract title cannot be empty".into(),
            ));
        }

        if let Some(to) = valid_to {
            if to < valid_from {
                return Err(PartyError::Validation(format!(
                    "Contract valid_to date ({to}) cannot precede valid_from date ({valid_from})"
                )));
            }
        }

        if let Some(limit) = credit_limit {
            if limit < Decimal::ZERO {
                return Err(PartyError::Validation(format!(
                    "Credit limit cannot be negative, got {limit}"
                )));
            }
        }

        Ok(Self {
            id: Uuid::new_v4(),
            tenant_id,
            party_id,
            contract_number,
            title,
            status: ContractStatus::Draft,
            valid_from,
            valid_to,
            credit_limit,
            payment_term_days,
        })
    }

    /// Verifies if the contract is active and valid for a specified business date.
    pub fn is_effective_on(&self, date: NaiveDate) -> bool {
        if self.status != ContractStatus::Active {
            return false;
        }
        if date < self.valid_from {
            return false;
        }
        if let Some(to) = self.valid_to {
            if date > to {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_lifecycle_and_effectiveness() {
        let tenant_id = Uuid::new_v4();
        let party_id = Uuid::new_v4();
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();

        let mut contract = Contract::new(
            tenant_id,
            party_id,
            "CNT-001",
            "Annual Supply",
            start,
            Some(end),
            Some(Decimal::new(1_000_000, 0)),
            Some(30),
        )
        .unwrap();

        assert_eq!(contract.status, ContractStatus::Draft);
        assert!(!contract.is_effective_on(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()));

        contract
            .status
            .transition_to(ContractStatus::Active)
            .unwrap();
        assert!(contract.is_effective_on(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()));
        assert!(!contract.is_effective_on(NaiveDate::from_ymd_opt(2027, 1, 1).unwrap()));
    }

    #[test]
    fn test_invalid_contract_dates() {
        let tenant_id = Uuid::new_v4();
        let party_id = Uuid::new_v4();
        let start = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();

        let result = Contract::new(
            tenant_id,
            party_id,
            "CNT-002",
            "Invalid Dates",
            start,
            Some(end),
            None,
            None,
        );
        assert!(result.is_err());
    }
}
