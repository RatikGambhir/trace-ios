//! Inbound bodies, and the validation that checks them.
//!
//! Each request type validates into a `Validated*` twin that lives beside it:
//! the request is what arrived, the validated form is what the rest of the
//! server is allowed to act on. Nothing downstream ever sees the raw body.

pub mod flight;
pub mod journey;
pub mod user;
