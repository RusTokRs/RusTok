/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pluggable, lightweight adapters implementing Security Suite SPI ports.

pub mod brute_force;
pub mod captcha;
pub mod rate_limiter;

pub use brute_force::MokaBruteForceTracker;
#[cfg(feature = "captcha-http")]
pub use captcha::http_adapters::{RecaptchaVerifier, TurnstileVerifier};
pub use captcha::{HoneypotVerifier, NoopCaptchaVerifier};
pub use rate_limiter::MokaRateLimiter;
