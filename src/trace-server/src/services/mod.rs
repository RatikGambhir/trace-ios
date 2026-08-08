//! Orchestration: transactions, idempotency, and the values the server derives
//! rather than accepts.
//!
//! Services own the decisions — what is written, in what order, and what is
//! computed on the way. Handlers translate HTTP; repositories run SQL; the
//! reasoning lives here.

pub mod flights;
pub mod journeys;
pub mod users;
