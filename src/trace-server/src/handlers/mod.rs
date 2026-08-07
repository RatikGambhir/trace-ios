//! HTTP: extract, delegate, choose a status code.
//!
//! Handlers hold no logic of their own. Anything a handler would decide beyond
//! which status code to return belongs in a service.

pub mod health;
pub mod journeys;
pub mod users;
