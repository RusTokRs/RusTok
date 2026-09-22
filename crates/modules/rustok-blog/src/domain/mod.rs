//! Transport- and persistence-independent Blog domain policies.

pub mod richtext;
pub mod state_machine;

#[cfg(test)]
mod state_machine_proptest;
