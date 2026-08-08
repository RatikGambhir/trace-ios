//! SQL, and nothing else.
//!
//! Every function takes a `&mut PgConnection` rather than a pool, so the caller
//! decides the transaction. That is what lets a journey write nine tables
//! atomically: the service opens one transaction and hands the same connection
//! to each repository in turn.

pub mod airlines;
pub mod airports;
pub mod flights;
pub mod journeys;
pub mod places;
pub mod users;
pub mod vehicles;
