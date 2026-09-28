/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Framework-agnostic design system primitives, variants, contracts, and styling resolvers.

pub mod classes;
pub mod contracts;
pub mod tokens;
pub mod types;

#[cfg(test)]
mod tests;

pub use classes::*;
pub use contracts::*;
pub use tokens::*;
pub use types::*;
