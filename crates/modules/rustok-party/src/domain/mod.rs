/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure domain model, state machines, invariants, and value objects for Counterparty & Legal Entities.
//!
//! In accordance with FBA, this module MUST NOT depend on database or transport frameworks.

pub mod bank_account;
pub mod contract;
pub mod party;
pub mod tax_details;

pub use bank_account::{BankAccount, validate_account_number, validate_bik};
pub use contract::{Contract, ContractStatus};
pub use party::{Party, PartyKind, PartyStatus};
pub use tax_details::{
    TaxDetails, validate_inn_10, validate_inn_12, validate_kpp, validate_ogrn_13,
    validate_ogrnip_15,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_lifecycle_transitions() {
        let mut status = PartyStatus::Draft;
        assert!(status.transition_to(PartyStatus::Active).is_ok());
        assert_eq!(status, PartyStatus::Active);

        assert!(status.transition_to(PartyStatus::Archived).is_ok());
        assert_eq!(status, PartyStatus::Archived);

        assert!(status.transition_to(PartyStatus::Draft).is_ok());
        assert_eq!(status, PartyStatus::Draft);
    }
}
