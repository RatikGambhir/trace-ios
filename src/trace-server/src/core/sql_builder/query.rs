//! A rendered statement, and the handful of ways to run one.

use sqlx::{postgres::PgQueryResult, FromRow, PgConnection, Postgres};

use super::params::Params;

/// SQL text plus the arguments it refers to — what every builder in this module
/// turns into, and the only thing that talks to sqlx.
pub struct Query {
    sql: String,
    params: Params,
}

impl Query {
    pub(super) fn new(sql: String, params: Params) -> Self {
        Query { sql, params }
    }

    /// Exactly one row, or [`sqlx::Error::RowNotFound`].
    pub async fn fetch_one<T>(self, conn: &mut PgConnection) -> Result<T, sqlx::Error>
    where
        T: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        let args = self.params.finish()?;
        sqlx::query_as_with::<Postgres, T, _>(&self.sql, args)
            .fetch_one(conn)
            .await
    }

    /// The row if there is one — the shape every "does this already exist?"
    /// read wants.
    pub async fn fetch_optional<T>(self, conn: &mut PgConnection) -> Result<Option<T>, sqlx::Error>
    where
        T: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        let args = self.params.finish()?;
        sqlx::query_as_with::<Postgres, T, _>(&self.sql, args)
            .fetch_optional(conn)
            .await
    }

    /// Every matching row.
    pub async fn fetch_all<T>(self, conn: &mut PgConnection) -> Result<Vec<T>, sqlx::Error>
    where
        T: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        let args = self.params.finish()?;
        sqlx::query_as_with::<Postgres, T, _>(&self.sql, args)
            .fetch_all(conn)
            .await
    }

    /// The first column of exactly one row, for a statement selecting a single
    /// value — an id, a count — without a `(T,)` tuple at the call site.
    pub async fn fetch_one_scalar<T>(self, conn: &mut PgConnection) -> Result<T, sqlx::Error>
    where
        T: Send + Unpin,
        (T,): for<'r> FromRow<'r, sqlx::postgres::PgRow>,
    {
        let args = self.params.finish()?;
        sqlx::query_scalar_with::<Postgres, T, _>(&self.sql, args)
            .fetch_one(conn)
            .await
    }

    /// [`Query::fetch_one_scalar`], for a row that may not be there.
    pub async fn fetch_optional_scalar<T>(
        self,
        conn: &mut PgConnection,
    ) -> Result<Option<T>, sqlx::Error>
    where
        T: Send + Unpin,
        (T,): for<'r> FromRow<'r, sqlx::postgres::PgRow>,
    {
        let args = self.params.finish()?;
        sqlx::query_scalar_with::<Postgres, T, _>(&self.sql, args)
            .fetch_optional(conn)
            .await
    }

    /// Run the statement for its effect, returning no rows.
    pub async fn execute(self, conn: &mut PgConnection) -> Result<PgQueryResult, sqlx::Error> {
        let args = self.params.finish()?;
        sqlx::query_with::<Postgres, _>(&self.sql, args)
            .execute(conn)
            .await
    }
}

/// Give a builder the terminal calls that run it.
///
/// Every statement type renders to a [`Query`] and is executed the same way, so
/// the six `fetch_*` methods live here once rather than once per builder. The
/// type this is invoked on supplies a `render(&self) -> String` and a `params`
/// field; everything else is generated.
macro_rules! runs_as_a_query {
    ($statement:ident) => {
        // Every builder gets the whole set for the sake of one uniform way to
        // run a statement; no one builder is expected to be executed six ways.
        #[allow(dead_code)]
        impl $statement {
            /// The SQL this builder renders, for tests and for reading.
            pub fn to_sql(&self) -> String {
                self.render()
            }

            fn into_query(self) -> $crate::core::sql_builder::Query {
                let sql = self.render();
                $crate::core::sql_builder::Query::new(sql, self.params)
            }

            /// Exactly one row, or [`sqlx::Error::RowNotFound`].
            pub async fn fetch_one<T>(self, conn: &mut sqlx::PgConnection) -> Result<T, sqlx::Error>
            where
                T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
            {
                self.into_query().fetch_one(conn).await
            }

            /// The row if there is one.
            pub async fn fetch_optional<T>(
                self,
                conn: &mut sqlx::PgConnection,
            ) -> Result<Option<T>, sqlx::Error>
            where
                T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
            {
                self.into_query().fetch_optional(conn).await
            }

            /// Every matching row.
            pub async fn fetch_all<T>(
                self,
                conn: &mut sqlx::PgConnection,
            ) -> Result<Vec<T>, sqlx::Error>
            where
                T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
            {
                self.into_query().fetch_all(conn).await
            }

            /// The first column of exactly one row.
            pub async fn fetch_one_scalar<T>(
                self,
                conn: &mut sqlx::PgConnection,
            ) -> Result<T, sqlx::Error>
            where
                T: Send + Unpin,
                (T,): for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
            {
                self.into_query().fetch_one_scalar(conn).await
            }

            /// The first column of the row, if there is one.
            pub async fn fetch_optional_scalar<T>(
                self,
                conn: &mut sqlx::PgConnection,
            ) -> Result<Option<T>, sqlx::Error>
            where
                T: Send + Unpin,
                (T,): for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
            {
                self.into_query().fetch_optional_scalar(conn).await
            }

            /// Run the statement for its effect, returning no rows.
            pub async fn execute(
                self,
                conn: &mut sqlx::PgConnection,
            ) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
                self.into_query().execute(conn).await
            }
        }
    };
}

pub(super) use runs_as_a_query;
