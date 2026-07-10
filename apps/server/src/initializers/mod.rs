//! # RusToK Server Initializers
//!
//! Host-owned startup actions.
//!
//! These run explicitly from the server bootstrap instead of Loco initializer
//! hooks, keeping lifecycle ordering visible to the host composition root.

pub mod superadmin;
