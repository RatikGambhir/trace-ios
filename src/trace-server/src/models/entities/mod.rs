//! Rows as the database stores them.
//!
//! An entity is what a repository returns. It carries no validation and no
//! notion of a request — those live in `requests` and `responses`.

pub mod flight;
pub mod journey;
pub mod user;
