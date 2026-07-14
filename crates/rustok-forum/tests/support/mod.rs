#![allow(dead_code)]

pub mod category_tree;
pub mod event_contract;
pub mod postgres;
pub mod read_model;

pub type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
