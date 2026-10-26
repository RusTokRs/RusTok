/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Domain and transactional error catalog for security.

use thiserror::Error;

/// Result alias for the security module.
pub type SecurityResult<T> = Result<T, SecurityError>;

/// Domain and transactional error conditions for security.
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Invalid IP or CIDR specification: {0}")]
    InvalidIpOrCidr(String),

    #[error("IP address {0} is blocked by policy")]
    IpBlocked(String),

    #[error("Rate limit of {limit} req/min exceeded; retry after {retry_after_secs}s")]
    RateLimitExceeded { limit: u32, retry_after_secs: u64 },

    #[error("Account locked due to consecutive failed attempts; retry after {remaining_secs}s")]
    AccountLocked { remaining_secs: u64 },

    #[error("Captcha verification failed: {0}")]
    CaptchaFailed(String),

    #[error("Unauthorized action: {0}")]
    Unauthorized(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

