//! The bound values of a statement, and the `$1`, `$2`, … that name them.

use sqlx::{postgres::PgArguments, Arguments, Encode, Postgres, Type};

/// What `Encode` hands back when a value cannot be written to the wire.
type BindError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// A statement's arguments, in the order Postgres will read them.
///
/// The one rule this type exists to enforce: a placeholder is *returned by* the
/// call that binds its value. There is no way to write a `$3` without having
/// just bound the third argument, so the number in the SQL and the position in
/// the argument list cannot drift apart — the failure mode that makes a
/// sixteen-column insert worth rewriting.
#[derive(Default)]
pub struct Params {
    arguments: PgArguments,
    bound: usize,
    /// The first encoding failure, reported when the statement runs. Binding
    /// stays infallible so a builder chain reads as one expression.
    failure: Option<BindError>,
}

impl Params {
    /// Bind `value` and return the placeholder that refers to it.
    pub fn bind<'a, T>(&mut self, value: T) -> String
    where
        T: 'a + Encode<'a, Postgres> + Type<Postgres>,
    {
        if let Err(err) = Arguments::add(&mut self.arguments, value) {
            self.failure.get_or_insert(err);
        }

        self.bound += 1;
        format!("${}", self.bound)
    }

    /// The arguments, or the encoding failure that has been waiting since a
    /// `bind` call further up the chain.
    pub fn finish(self) -> Result<PgArguments, sqlx::Error> {
        match self.failure {
            Some(err) => Err(sqlx::Error::Encode(err)),
            None => Ok(self.arguments),
        }
    }
}
