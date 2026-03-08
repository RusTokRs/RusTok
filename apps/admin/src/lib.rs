#![recursion_limit = "512"]

pub mod app;
pub mod entities;
pub mod features;
pub mod pages;
pub mod shared;
pub mod widgets;

include!(concat!(env!("OUT_DIR"), "/i18n/mod.rs"));
pub use i18n::*;
