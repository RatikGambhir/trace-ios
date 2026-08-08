//! Shared across every layer: error shape, geometry, validation helpers,
//! password/key primitives, the SQL builder, and the handle to the database.
//!
//! Nothing here knows about HTTP or about any one table. If a thing is used by
//! two features, it belongs here; if only one feature needs it, it does not.

pub mod crypto;
pub mod error;
pub mod geo;
pub mod sql_builder;
pub mod state;
pub mod validation;
