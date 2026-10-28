/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Tax identification value objects and mathematical checksum validators.

use crate::error::{PartyError, PartyResult};
use serde::{Deserialize, Serialize};

/// Canonical tax details for legal entities, sole proprietors, and individuals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaxDetails {
    /// Taxpayer Identification Number (ИНН in Russia, VAT/TIN internationally).
    pub inn: String,
    /// Tax Registration Reason Code (КПП for Russian legal entities).
    pub kpp: Option<String>,
    /// Primary State Registration Number (ОГРН/ОГРНИП).
    pub ogrn: Option<String>,
    /// Value Added Tax registration number (for international or EU VAT).
    pub vat_code: Option<String>,
    /// All-Russian Classifier of Enterprises and Organizations (ОКПО).
    pub okpo: Option<String>,
}

impl TaxDetails {
    /// Creates and validates Russian legal entity tax details.
    pub fn new_legal_entity(
        inn: impl Into<String>,
        kpp: Option<String>,
        ogrn: Option<String>,
    ) -> PartyResult<Self> {
        let inn = inn.into();
        validate_inn_10(&inn)?;

        if let Some(ref k) = kpp {
            validate_kpp(k)?;
        }
        if let Some(ref o) = ogrn {
            validate_ogrn_13(o)?;
        }

        Ok(Self {
            inn,
            kpp,
            ogrn,
            vat_code: None,
            okpo: None,
        })
    }

    /// Creates and validates Russian sole proprietor (ИП) tax details.
    pub fn new_sole_proprietor(
        inn: impl Into<String>,
        ogrnip: Option<String>,
    ) -> PartyResult<Self> {
        let inn = inn.into();
        validate_inn_12(&inn)?;

        if let Some(ref o) = ogrnip {
            validate_ogrnip_15(o)?;
        }

        Ok(Self {
            inn,
            kpp: None,
            ogrn: ogrnip,
            vat_code: None,
            okpo: None,
        })
    }
}

/// Validates a 10-digit INN for Russian legal entities using Russian Federal Tax Service checksum formula.
pub fn validate_inn_10(inn: &str) -> PartyResult<()> {
    if inn.len() != 10 || !inn.chars().all(|c| c.is_ascii_digit()) {
        return Err(PartyError::Validation(format!(
            "Legal entity INN must be exactly 10 numeric digits, got '{inn}'"
        )));
    }

    let digits: Vec<u32> = inn.chars().map(|c| c.to_digit(10).unwrap()).collect();
    let weights = [2, 4, 10, 3, 5, 9, 4, 6, 8];

    let sum: u32 = digits[..9]
        .iter()
        .zip(weights.iter())
        .map(|(d, w)| d * w)
        .sum();
    let checksum = (sum % 11) % 10;

    if digits[9] != checksum {
        return Err(PartyError::Validation(format!(
            "Invalid 10-digit INN checksum: expected {checksum}, got {}",
            digits[9]
        )));
    }

    Ok(())
}

/// Validates a 12-digit INN for individuals and sole proprietors using Russian Federal Tax Service checksum formula.
pub fn validate_inn_12(inn: &str) -> PartyResult<()> {
    if inn.len() != 12 || !inn.chars().all(|c| c.is_ascii_digit()) {
        return Err(PartyError::Validation(format!(
            "Individual/sole proprietor INN must be exactly 12 numeric digits, got '{inn}'"
        )));
    }

    let digits: Vec<u32> = inn.chars().map(|c| c.to_digit(10).unwrap()).collect();

    // 11th digit check
    let weights_11 = [7, 2, 4, 10, 3, 5, 9, 4, 6, 8];
    let sum_11: u32 = digits[..10]
        .iter()
        .zip(weights_11.iter())
        .map(|(d, w)| d * w)
        .sum();
    let checksum_11 = (sum_11 % 11) % 10;

    if digits[10] != checksum_11 {
        return Err(PartyError::Validation(format!(
            "Invalid 12-digit INN 11th checksum: expected {checksum_11}, got {}",
            digits[10]
        )));
    }

    // 12th digit check
    let weights_12 = [3, 7, 2, 4, 10, 3, 5, 9, 4, 6, 8];
    let sum_12: u32 = digits[..11]
        .iter()
        .zip(weights_12.iter())
        .map(|(d, w)| d * w)
        .sum();
    let checksum_12 = (sum_12 % 11) % 10;

    if digits[11] != checksum_12 {
        return Err(PartyError::Validation(format!(
            "Invalid 12-digit INN 12th checksum: expected {checksum_12}, got {}",
            digits[11]
        )));
    }

    Ok(())
}

/// Validates a 9-character Tax Registration Reason Code (КПП).
pub fn validate_kpp(kpp: &str) -> PartyResult<()> {
    if kpp.len() != 9 || !kpp.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(PartyError::Validation(format!(
            "KPP must be exactly 9 alphanumeric characters, got '{kpp}'"
        )));
    }
    Ok(())
}

/// Validates a 13-digit Primary State Registration Number (ОГРН) for Russian legal entities.
pub fn validate_ogrn_13(ogrn: &str) -> PartyResult<()> {
    if ogrn.len() != 13 || !ogrn.chars().all(|c| c.is_ascii_digit()) {
        return Err(PartyError::Validation(format!(
            "OGRN must be exactly 13 numeric digits, got '{ogrn}'"
        )));
    }

    let first_12: u64 = ogrn[..12]
        .parse()
        .map_err(|_| PartyError::Validation("Invalid OGRN prefix".into()))?;
    let check_digit = (first_12 % 11) % 10;
    let actual_last = ogrn.chars().last().unwrap().to_digit(10).unwrap() as u64;

    if actual_last != check_digit {
        return Err(PartyError::Validation(format!(
            "Invalid OGRN checksum: expected {check_digit}, got {actual_last}"
        )));
    }

    Ok(())
}

/// Validates a 15-digit Primary State Registration Number (ОГРНИП) for Russian sole proprietors.
pub fn validate_ogrnip_15(ogrnip: &str) -> PartyResult<()> {
    if ogrnip.len() != 15 || !ogrnip.chars().all(|c| c.is_ascii_digit()) {
        return Err(PartyError::Validation(format!(
            "OGRNIP must be exactly 15 numeric digits, got '{ogrnip}'"
        )));
    }

    let first_14: u64 = ogrnip[..14]
        .parse()
        .map_err(|_| PartyError::Validation("Invalid OGRNIP prefix".into()))?;
    let check_digit = (first_14 % 13) % 10;
    let actual_last = ogrnip.chars().last().unwrap().to_digit(10).unwrap() as u64;

    if actual_last != check_digit {
        return Err(PartyError::Validation(format!(
            "Invalid OGRNIP checksum: expected {check_digit}, got {actual_last}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_inn_10() {
        // Known valid Russian corporate INNs
        assert!(validate_inn_10("7707083893").is_ok()); // Сбербанк
        assert!(validate_inn_10("7736207543").is_ok()); // Яндекс
        assert!(validate_inn_10("7707083894").is_err()); // Wrong checksum
        assert!(validate_inn_10("12345").is_err()); // Wrong length
    }

    #[test]
    fn test_valid_inn_12() {
        // Valid 12-digit individual INN
        assert!(validate_inn_12("500100732259").is_ok());
        assert!(validate_inn_12("500100732250").is_err()); // Wrong checksum
    }

    #[test]
    fn test_valid_kpp() {
        assert!(validate_kpp("773601001").is_ok());
        assert!(validate_kpp("77360100").is_err());
    }

    #[test]
    fn test_valid_ogrn() {
        // Сбербанк OGRN: 1027700132195
        assert!(validate_ogrn_13("1027700132195").is_ok());
        assert!(validate_ogrn_13("1027700132190").is_err());
    }
}
