/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

#![recursion_limit = "512"]

pub mod app;
pub mod entities;
pub mod features;
pub mod pages;
pub mod shared;
pub mod widgets;

mod generated_i18n {
    #![allow(clippy::new_ret_no_self)]
    #![allow(non_snake_case)]
    include!(concat!(env!("OUT_DIR"), "/i18n/mod.rs"));
}

pub use generated_i18n::i18n;
pub use generated_i18n::i18n::*;
