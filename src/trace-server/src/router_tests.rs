use super::*;

use axum::{body::Body, http::Request};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

/// A pool that is never actually connected. Enough to build the router, and
/// enough for the routes below, which answer before touching the database.
fn test_state() -> Arc<AppState> {
    Arc::new(AppState {
        db: PgPoolOptions::new()
            .connect_lazy("postgres://postgres@localhost/postgres")
            .expect("the test connection string parses"),
    })
}

#[tokio::test]
async fn health_returns_ok() {
    let response = app(test_state())
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn create_user_rejects_an_invalid_payload_before_hitting_the_database() {
    let response = app(test_state())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"first_name":"","last_name":"Lovelace","password":"short"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["error"], "validation_failed");
    assert_eq!(body["details"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn create_journey_rejects_an_invalid_payload_before_opening_a_transaction() {
    let response = app(test_state())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/journeys")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "user_id": "00000000-0000-0000-0000-000000000001",
                        "title": "",
                        "segments": [
                            {"mode": "flight"},
                            {"mode": "walk", "drive": {"role": "driver"}}
                        ]
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["error"], "validation_failed");

    // A blank title, a flight segment with no flight, and drive details on
    // a walk — all three, from one request, before any connection is taken.
    let details = body["details"].as_array().unwrap();
    assert_eq!(details.len(), 3);
    assert!(details
        .iter()
        .any(|d| d.as_str().unwrap().contains("title")));
    assert!(details
        .iter()
        .any(|d| d.as_str().unwrap().starts_with("segments[0].flight")));
    assert!(details
        .iter()
        .any(|d| d.as_str().unwrap().starts_with("segments[1].drive")));
}

#[tokio::test]
async fn unknown_routes_are_not_found() {
    let response = app(test_state())
        .oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
