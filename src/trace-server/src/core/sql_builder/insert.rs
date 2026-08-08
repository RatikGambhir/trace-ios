//! `INSERT`.

use sqlx::{Encode, Postgres, Type};

use super::{params::Params, query::runs_as_a_query};

/// An `INSERT`, with every column written next to the value going into it.
///
/// ```ignore
/// let id: Option<i64> = Insert::into("airlines")
///     .set("iata_code", &airline.iata_code)
///     .set("icao_code", &airline.icao_code)
///     .set("name", name)
///     .on_conflict_do_nothing(&["iata_code"])
///     .returning(&["id"])
///     .fetch_optional_scalar(conn)
///     .await?;
/// ```
///
/// The column list, the `VALUES` list, and the arguments are all produced by the
/// same `set` calls, which is the whole point: adding a column is one line, and
/// there is no second list to keep in step with the first.
pub struct Insert {
    table: &'static str,
    columns: Vec<&'static str>,
    values: Vec<String>,
    on_conflict: Option<String>,
    returning: Vec<&'static str>,
    params: Params,
}

impl Insert {
    /// Start an `INSERT INTO table`.
    pub fn into(table: &'static str) -> Self {
        Insert {
            table,
            columns: Vec::new(),
            values: Vec::new(),
            on_conflict: None,
            returning: Vec::new(),
            params: Params::default(),
        }
    }

    /// Write `value` to `column`.
    pub fn set<'a, T>(mut self, column: &'static str, value: T) -> Self
    where
        T: 'a + Encode<'a, Postgres> + Type<Postgres>,
    {
        let placeholder = self.params.bind(value);
        self.columns.push(column);
        self.values.push(placeholder);
        self
    }

    /// [`Insert::set`], for a value Postgres needs told the type of.
    ///
    /// `set_cast("latitude", value, "float8::numeric")` renders `$3::float8::numeric`
    /// — the bridge for a column whose type has no native Rust mapping in this
    /// build of sqlx.
    pub fn set_cast<'a, T>(mut self, column: &'static str, value: T, cast: &'static str) -> Self
    where
        T: 'a + Encode<'a, Postgres> + Type<Postgres>,
    {
        let placeholder = self.params.bind(value);
        self.columns.push(column);
        self.values.push(format!("{placeholder}::{cast}"));
        self
    }

    /// `ON CONFLICT (columns) DO NOTHING` — the row is already there, and the
    /// stored one wins.
    ///
    /// Paired with [`Insert::returning`] this is the get-or-create shape: a row
    /// back means this call created it, no row means someone else did.
    pub fn on_conflict_do_nothing(mut self, columns: &[&'static str]) -> Self {
        self.on_conflict = Some(format!("ON CONFLICT ({}) DO NOTHING", columns.join(", ")));
        self
    }

    /// [`Insert::on_conflict_do_nothing`] against a *partial* unique index, whose
    /// `WHERE` has to be repeated here for Postgres to recognise the target.
    pub fn on_conflict_do_nothing_where(
        mut self,
        columns: &[&'static str],
        index_predicate: &'static str,
    ) -> Self {
        self.on_conflict = Some(format!(
            "ON CONFLICT ({}) WHERE {index_predicate} DO NOTHING",
            columns.join(", ")
        ));
        self
    }

    /// `RETURNING columns`. Takes the same column list a `SELECT` of this row
    /// would, so the two can share one constant.
    pub fn returning(mut self, columns: &[&'static str]) -> Self {
        self.returning.extend_from_slice(columns);
        self
    }

    fn render(&self) -> String {
        let mut sql = format!(
            "INSERT INTO {} ({})\nVALUES ({})",
            self.table,
            self.columns.join(", "),
            self.values.join(", "),
        );

        if let Some(on_conflict) = &self.on_conflict {
            sql.push('\n');
            sql.push_str(on_conflict);
        }

        if !self.returning.is_empty() {
            sql.push_str("\nRETURNING ");
            sql.push_str(&self.returning.join(", "));
        }

        sql
    }
}

runs_as_a_query!(Insert);

#[cfg(test)]
#[path = "insert_tests.rs"]
mod tests;
