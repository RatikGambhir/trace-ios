use super::Select;

#[test]
fn renders_a_lookup_by_column() {
    let sql = Select::from("airlines")
        .columns(&["id"])
        .where_eq("iata_code", "AA")
        .to_sql();

    assert_eq!(
        sql,
        "SELECT id\n\
         FROM airlines\n\
         WHERE iata_code = $1"
    );
}

#[test]
fn ands_conditions_in_the_order_they_were_written() {
    let sql = Select::from("vehicles")
        .columns(&["id"])
        .where_eq("id", 7_i64)
        .where_eq("user_id", "a-user")
        .to_sql();

    assert_eq!(
        sql,
        "SELECT id\n\
         FROM vehicles\n\
         WHERE id = $1 AND user_id = $2"
    );
}

#[test]
fn numbers_placeholders_from_one_in_bind_order() {
    let sql = Select::from("t")
        .columns(&["id"])
        .where_eq("a", 1_i32)
        .where_eq("b", 2_i32)
        .where_eq("c", 3_i32)
        .to_sql();

    assert!(sql.ends_with("WHERE a = $1 AND b = $2 AND c = $3"), "{sql}");
}

#[test]
fn renders_a_join_before_the_conditions_whatever_order_it_was_called_in() {
    let sql = Select::from("places p")
        .where_any_of("p.id", &[1_i64, 2][..])
        .columns(&["p.id", "a.iata_code"])
        .left_join("airports a", "a.id = p.airport_id")
        .to_sql();

    assert_eq!(
        sql,
        "SELECT p.id, a.iata_code\n\
         FROM places p\n\
         LEFT JOIN airports a ON a.id = p.airport_id\n\
         WHERE p.id = ANY($1)"
    );
}

#[test]
fn renders_an_inner_join() {
    let sql = Select::from("journey_flights jf")
        .columns(&["jf.flight_id"])
        .join("flights f", "f.id = jf.flight_id")
        .to_sql();

    assert_eq!(
        sql,
        "SELECT jf.flight_id\n\
         FROM journey_flights jf\n\
         JOIN flights f ON f.id = jf.flight_id"
    );
}

#[test]
fn appends_repeated_column_lists_so_a_constant_can_be_extended() {
    const STORED: &[&str] = &["id", "name"];

    let sql = Select::from("t")
        .columns(STORED)
        .columns(&["created_at"])
        .to_sql();

    assert!(sql.starts_with("SELECT id, name, created_at\n"), "{sql}");
}

#[test]
fn renders_order_by_last() {
    let sql = Select::from("journey_legs")
        .columns(&["id", "position"])
        .where_eq("journey_id", 1_i64)
        .order_by("position")
        .to_sql();

    assert_eq!(
        sql,
        "SELECT id, position\n\
         FROM journey_legs\n\
         WHERE journey_id = $1\n\
         ORDER BY position"
    );
}

#[test]
fn omits_the_where_clause_when_there_are_no_conditions() {
    let sql = Select::from("airlines").columns(&["id"]).to_sql();

    assert_eq!(sql, "SELECT id\nFROM airlines");
}
