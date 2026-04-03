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
