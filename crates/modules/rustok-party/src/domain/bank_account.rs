/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Bank account value objects and validation rules.

use crate::error::{PartyError, PartyResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Bank account details associated with a corporate party.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankAccount {
    /// Unique identifier for this bank account record.
    pub id: Uuid,
    /// Reference to the owner party.
    pub party_id: Uuid,
    /// 20-digit Russian settlement account or international IBAN.
    pub account_number: String,
    /// 9-digit Russian Bank Identifier Code (БИК) or international SWIFT/BIC.
    pub bik: Option<String>,
    /// Full commercial bank name.
    pub bank_name: String,
    /// Correspondent account (20 digits in Russia).
    pub corr_account: Option<String>,
    /// 3-letter currency code (ISO 4217, e.g. "RUB", "USD", "EUR", "CNY").
    pub currency: String,
    /// Flag indicating if this is the default payment account for the counterparty.
    pub is_primary: bool,
}

impl BankAccount {
    /// Creates and validates a new bank account.
    pub fn new(
        party_id: Uuid,
        account_number: impl Into<String>,
        bank_name: impl Into<String>,
        bik: Option<String>,
        corr_account: Option<String>,
        currency: impl Into<String>,
        is_primary: bool,
    ) -> PartyResult<Self> {
        if party_id.is_nil() {
            return Err(PartyError::Validation("Party ID cannot be nil".into()));
        }

        let account_number = account_number.into().trim().to_string();
        let bank_name = bank_name.into().trim().to_string();
        let bik = bik.map(|value| value.trim().to_string());
        let corr_account = corr_account.map(|value| value.trim().to_string());
        let currency = currency.into().trim().to_ascii_uppercase();

        if bank_name.is_empty() {
            return Err(PartyError::Validation("Bank name cannot be empty".into()));
        }

        validate_account_number(&account_number)?;

        if let Some(ref b) = bik {
            validate_bik(b)?;
        }
        if let Some(ref ca) = corr_account {
            validate_account_number(ca)?;
        }

        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(PartyError::Validation(format!(
                "Currency must be a 3-letter ISO code, got '{currency}'"
            )));
        }

        Ok(Self {
            id: Uuid::new_v4(),
            party_id,
            account_number,
            bik,
            bank_name,
            corr_account,
            currency,
            is_primary,
        })
    }
}

/// Validates that an account number is either a 20-digit national account or a valid IBAN length.
pub fn validate_account_number(account: &str) -> PartyResult<()> {
    let trimmed = account.trim();
    if trimmed.is_empty() || trimmed.bytes().all(|b| b == b'0') {
        return Err(PartyError::Validation(
            "Account number cannot be empty or all zeros".into(),
        ));
    }

    // Standard 20-digit Russian settlement or correspondent account
    if trimmed.len() == 20 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Ok(());
    }

    // International IBAN format (typically 15 to 34 alphanumeric characters)
    if (15..=34).contains(&trimmed.len()) && trimmed.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(());
    }

    Err(PartyError::Validation(format!(
        "Invalid account number format: expected 20-digit account or IBAN, got '{trimmed}'"
    )))
}

/// Validates a 9-digit Russian Bank Identifier Code (БИК).
pub fn validate_bik(bik: &str) -> PartyResult<()> {
    let trimmed = bik.trim();
    if trimmed.len() != 9
        || !trimmed.chars().all(|c| c.is_ascii_digit())
        || trimmed.bytes().all(|b| b == b'0')
    {
        return Err(PartyError::Validation(format!(
            "BIK must be a valid non-zero 9-digit number, got '{trimmed}'"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_bank_account() {
        let party_id = Uuid::new_v4();
        let account = BankAccount::new(
            party_id,
            "40702810938000012345",
            "СБЕРБАНК",
            Some("044525225".into()),
            Some("30101810400000000225".into()),
            "RUB",
            true,
        );

        assert!(account.is_ok());
        let acc = account.unwrap();
        assert_eq!(acc.currency, "RUB");
        assert!(acc.is_primary);
    }

    #[test]
    fn test_invalid_bik() {
        assert!(validate_bik("04452522").is_err()); // 8 digits
        assert!(validate_bik("044525225A").is_err()); // 10 chars
        assert!(validate_bik("000000000").is_err()); // all zeros
        assert!(validate_bik("044525225").is_ok());
    }

    #[test]
    fn test_bank_account_rejects_invalid_party_bank_name_and_currency() {
        let party_id = Uuid::new_v4();
        assert!(
            BankAccount::new(
                Uuid::nil(),
                "40702810938000012345",
                "Sberbank",
                None,
                None,
                "RUB",
                false
            )
            .is_err()
        );
        assert!(
            BankAccount::new(
                party_id,
                "40702810938000012345",
                "   ",
                None,
                None,
                "RUB",
                false
            )
            .is_err()
        );
        assert!(
            BankAccount::new(
                party_id,
                "40702810938000012345",
                "Sberbank",
                None,
                None,
                "12!",
                false
            )
            .is_err()
        );
    }
}
