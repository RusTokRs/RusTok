/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Domain and transactional error catalog for calendar.

use thiserror::Error;

/// Result alias for the calendar module.
pub type CalendarResult<T> = Result<T, CalendarError>;

/// Domain and transactional error conditions for calendar.
#[derive(Debug, Error)]
pub enum CalendarError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
