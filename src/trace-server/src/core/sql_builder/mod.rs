//! A SQL builder for the queries this server actually runs.
//!
//! The repositories were written as string literals with `$1`…`$16` in them and
//! a matching column of `.bind()` calls underneath. That reads badly in one
//! specific way: to answer "what goes in `arrival_gate`?" you count placeholders
//! in the SQL, then count binds, and hope the two lists agree. They agree
//! because someone checked, not because anything makes them.
//!
//! Here the two lists are one list:
//!
//! ```ignore
//! Insert::into("airlines")
//!     .set("iata_code", &airline.iata_code)
//!     .set("icao_code", &airline.icao_code)
//!     .set("name", name)
//!     .on_conflict_do_nothing(&["iata_code"])
//!     .returning(&["id"])
//!     .fetch_optional_scalar(conn)
//!     .await?
//! ```
//!
//! A [`Select`] reads the same way:
//!
//! ```ignore
//! Select::from("places p")
//!     .columns(&["p.id", "p.name", "a.iata_code"])
//!     .left_join("airports a", "a.id = p.airport_id")
//!     .where_any_of("p.id", place_ids)
//!     .fetch_all(conn)
//!     .await?
//! ```
//!
//! # Two rules
//!
//! **Everything that becomes SQL text is `&'static str`.** Table names, column
//! names, join predicates, casts: all of them are literals in this repository's
//! source, which the type system checks — a `String` built from a request body
//! will not compile. There is no escaping to get wrong because there is nothing
//! to escape.
//!
//! **Everything that came from outside is a bind parameter.** [`Params::bind`]
//! is the only thing in this module that writes a `$n`, and it writes one only
//! as it appends the value it names. A placeholder without its argument is not
//! expressible.
//!
//! # Scope
//!
//! `SELECT` and `INSERT`, because that is what a journey write and read need. No
//! `UPDATE`, no `DELETE`, no subqueries, no `GROUP BY` — the moment a query
//! wants one of those, write it out in SQL and hand it to sqlx directly. A
//! builder is worth having while it stays smaller than the SQL it replaces.
//!
//! Every builder can [`Select::to_sql`] itself, which is how the tests beside
//! this module read: assert on the exact statement, no database needed.

mod insert;
mod params;
mod query;
mod select;

pub use insert::Insert;
pub use query::Query;
pub use select::Select;
