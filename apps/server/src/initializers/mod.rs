//! # RusToK Server Initializers
//!
//! Third-party service initialization and setup.
//! Run during application startup before routes are mounted.

use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};
use std::vec::Vec;

pub mod superadmin;

/// Create and return all initializers
pub async fn create(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
    let initializers: Vec<Box<dyn Initializer>> = vec![
        Box::new(superadmin::SuperAdminInitializer),
    ];

    Ok(initializers)
}
