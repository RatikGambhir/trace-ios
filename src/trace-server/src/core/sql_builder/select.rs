//! `SELECT`.

use sqlx::{Encode, Postgres, Type};

use super::{params::Params, query::runs_as_a_query};

/// A `SELECT`, written in the order you would say it out loud.
///
/// ```ignore
/// let airport: Option<ResolvedAirport> = Select::from("airports")
///     .columns(&["id", "latitude::float8 AS latitude"])
///     .where_eq("iata_code", iata_code)
///     .fetch_optional(conn)
///     .await?;
/// ```
///
/// Clauses render in SQL's order no matter which order they are called in, so a
/// `where_eq` can sit next to the value it filters on rather than at the bottom
/// of the statement.
pub struct Select {
    from: &'static str,
    columns: Vec<&'static str>,
    joins: Vec<String>,
    conditions: Vec<String>,
    order_by: Option<&'static str>,
    params: Params,
}

impl Select {
    /// Start a `SELECT` against `table`, which may carry an alias:
    /// `Select::from("places p")`.
    pub fn from(table: &'static str) -> Self {
        Select {
            from: table,
            columns: Vec::new(),
            joins: Vec::new(),
            conditions: Vec::new(),
            order_by: None,
            params: Params::default(),
        }
    }

    /// The columns to read. Each entry is any SQL expression — `"p.id"`,
    /// `"latitude::float8 AS latitude"` — and repeated calls append, so a shared
    /// column constant can be extended at a single call site.
    pub fn columns(mut self, columns: &[&'static str]) -> Self {
        self.columns.extend_from_slice(columns);
        self
    }

    /// `JOIN table ON predicate`.
    pub fn join(mut self, table: &'static str, on: &'static str) -> Self {
        self.joins.push(format!("JOIN {table} ON {on}"));
        self
    }

    /// `LEFT JOIN table ON predicate`, for a row that may have no match.
    pub fn left_join(mut self, table: &'static str, on: &'static str) -> Self {
        self.joins.push(format!("LEFT JOIN {table} ON {on}"));
        self
    }

    /// `column = value`. Call it once per column; the conditions are `AND`ed.
    pub fn where_eq<'a, T>(mut self, column: &'static str, value: T) -> Self
    where
        T: 'a + Encode<'a, Postgres> + Type<Postgres>,
    {
        let placeholder = self.params.bind(value);
        self.conditions.push(format!("{column} = {placeholder}"));
        self
    }

    /// `column = ANY(values)` — one row per id, in one round trip, instead of a
    /// query per id.
    pub fn where_any_of<'a, T>(mut self, column: &'static str, values: T) -> Self
    where
        T: 'a + Encode<'a, Postgres> + Type<Postgres>,
    {
        let placeholder = self.params.bind(values);
        self.conditions
            .push(format!("{column} = ANY({placeholder})"));
        self
    }

    /// `ORDER BY column`.
    pub fn order_by(mut self, column: &'static str) -> Self {
        self.order_by = Some(column);
        self
    }

    fn render(&self) -> String {
        let mut sql = format!("SELECT {}\nFROM {}", self.columns.join(", "), self.from,);

        for join in &self.joins {
            sql.push('\n');
            sql.push_str(join);
        }

        if !self.conditions.is_empty() {
            sql.push_str("\nWHERE ");
            sql.push_str(&self.conditions.join(" AND "));
        }

        if let Some(column) = self.order_by {
            sql.push_str("\nORDER BY ");
            sql.push_str(column);
        }

        sql
    }
}

runs_as_a_query!(Select);

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
