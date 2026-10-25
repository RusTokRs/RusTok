/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure domain model, state machines, invariants, and value objects for Security.
//!
//! In accordance with FBA, this module MUST NOT depend on database or transport frameworks.

use serde::{Deserialize, Serialize};
use crate::error::{SecurityError, SecurityResult};

/// Domain lifecycle state for Security.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecurityStatus {
    #[default]
    Draft,
    Active,
    Archived,
}

impl SecurityStatus {
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
    pub fn transition_to(&mut self, target: Self) -> SecurityResult<()> {
        if !self.can_transition_to(target) {
            return Err(SecurityError::Validation(format!(
                "Invalid state transition from {:?} to {:?}",
                self, target
            )));
        }
        *self = target;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_lifecycle_transitions() {
        let mut status = SecurityStatus::Draft;
        assert!(status.transition_to(SecurityStatus::Active).is_ok());
        assert_eq!(status, SecurityStatus::Active);

        // Invalid transition: Active -> Draft
        assert!(status.transition_to(SecurityStatus::Draft).is_err());

        // Valid transition: Active -> Archived
        assert!(status.transition_to(SecurityStatus::Archived).is_ok());
        assert_eq!(status, SecurityStatus::Archived);
    }
}
