//! Request bodies, response shapes, and the rows in between — plus the
//! validation that turns one into the other.
//!
//! Types here are inert: they parse, check, and normalise, and never touch the
//! database. That is what lets every validation rule be tested without one.

pub mod flight;
pub mod journey;
pub mod user;
