use super::Insert;

#[test]
fn renders_columns_and_values_from_the_same_calls() {
    let sql = Insert::into("airlines")
        .set("iata_code", "AA")
        .set("icao_code", "AAL")
        .set("name", "American Airlines")
        .to_sql();

    assert_eq!(
        sql,
        "INSERT INTO airlines (iata_code, icao_code, name)\n\
         VALUES ($1, $2, $3)"
    );
}

/// The property the builder exists for: the n-th column and the n-th argument
/// are written by one call, so they cannot be given different positions.
#[test]
fn keeps_columns_and_placeholders_aligned_across_a_wide_insert() {
    let mut insert = Insert::into("flights");
    for column in ["a", "b", "c", "d", "e", "f", "g", "h"] {
        insert = insert.set(column, column);
    }

    assert_eq!(
        insert.to_sql(),
        "INSERT INTO flights (a, b, c, d, e, f, g, h)\n\
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    );
}

#[test]
fn casts_a_value_without_disturbing_the_placeholder_numbering() {
    let sql = Insert::into("places")
        .set("name", "Home")
        .set_cast("latitude", 37.7749_f64, "float8::numeric")
        .set_cast("longitude", -122.4194_f64, "float8::numeric")
        .set("timezone", "America/Los_Angeles")
        .to_sql();

    assert_eq!(
        sql,
        "INSERT INTO places (name, latitude, longitude, timezone)\n\
         VALUES ($1, $2::float8::numeric, $3::float8::numeric, $4)"
    );
}

#[test]
fn renders_on_conflict_do_nothing() {
    let sql = Insert::into("airports")
        .set("iata_code", "SFO")
        .on_conflict_do_nothing(&["iata_code"])
        .returning(&["id"])
        .to_sql();

    assert_eq!(
        sql,
        "INSERT INTO airports (iata_code)\n\
         VALUES ($1)\n\
         ON CONFLICT (iata_code) DO NOTHING\n\
         RETURNING id"
    );
}

#[test]
fn renders_a_composite_conflict_target() {
    let sql = Insert::into("flights")
        .set("airline_id", 1_i64)
        .set("flight_number", "100")
        .on_conflict_do_nothing(&["airline_id", "flight_number", "scheduled_departure_at"])
        .to_sql();

    assert!(
        sql.ends_with("ON CONFLICT (airline_id, flight_number, scheduled_departure_at) DO NOTHING"),
        "{sql}"
    );
}

/// A partial unique index only matches a conflict target that repeats its
/// predicate, so the builder has to render it.
#[test]
fn renders_the_predicate_of_a_partial_unique_index() {
    let sql = Insert::into("journeys")
        .set("user_id", 1_i64)
        .set("idempotency_key", "abc")
        .on_conflict_do_nothing_where(
            &["user_id", "idempotency_key"],
            "idempotency_key IS NOT NULL",
        )
        .returning(&["id"])
        .to_sql();

    assert_eq!(
        sql,
        "INSERT INTO journeys (user_id, idempotency_key)\n\
         VALUES ($1, $2)\n\
         ON CONFLICT (user_id, idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING\n\
         RETURNING id"
    );
}

#[test]
fn returns_the_same_column_list_a_select_would_read() {
    const COLUMNS: &[&str] = &["id", "airline_id", "flight_number"];

    let sql = Insert::into("flights").set("airline_id", 1_i64).to_sql();
    assert!(!sql.contains("RETURNING"), "{sql}");

    let sql = Insert::into("flights")
        .set("airline_id", 1_i64)
        .returning(COLUMNS)
        .to_sql();
    assert!(
        sql.ends_with("RETURNING id, airline_id, flight_number"),
        "{sql}"
    );
}
