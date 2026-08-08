//! Request bodies, response shapes, and the rows in between.
//!
//! Split by kind rather than by feature, so the direction of a type is
//! obvious from where it lives:
//!
//! * `requests`  — what arrives, plus the validation that checks it
//! * `entities`  — what the database stores
//! * `responses` — what goes back
//!
//! Types here are inert: they parse, check, and normalise, and never touch
//! the database. That is what lets every validation rule be tested without
//! one.

pub mod entities;
pub mod requests;
pub mod responses;
