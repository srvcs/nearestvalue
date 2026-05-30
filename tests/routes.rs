use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use srvcs_nearestvalue::{health, router, telemetry};
use tower::ServiceExt;

fn app() -> axum::Router {
    router(telemetry::metrics_handle_for_tests())
}

async fn status_of(uri: &str) -> StatusCode {
    app()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

/// POST `body` to `/` and return (status, parsed JSON).
async fn eval(body: Value) -> (StatusCode, Value) {
    let res = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

// --- Standard srvcs service surface ---

#[tokio::test]
async fn index_ok() {
    assert_eq!(status_of("/").await, StatusCode::OK);
}

#[tokio::test]
async fn healthz_ok() {
    assert_eq!(status_of("/healthz").await, StatusCode::OK);
}

#[tokio::test]
async fn readyz_reflects_state() {
    health::set_ready(true);
    assert_eq!(status_of("/readyz").await, StatusCode::OK);
}

#[tokio::test]
async fn metrics_ok() {
    assert_eq!(status_of("/metrics").await, StatusCode::OK);
}

#[tokio::test]
async fn openapi_ok() {
    assert_eq!(status_of("/openapi.json").await, StatusCode::OK);
}

// --- Asserted algorithm cases ---

#[tokio::test]
async fn nearest_below_value() {
    let (status, body) = eval(json!({ "value": 5, "values": [1, 4, 9] })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["value"], 5);
    assert_eq!(body["values"], json!([1, 4, 9]));
    assert_eq!(body["result"], 4);
}

#[tokio::test]
async fn nearest_above_value() {
    let (status, body) = eval(json!({ "value": 7, "values": [1, 4, 9] })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 9);
}

#[tokio::test]
async fn tie_returns_first() {
    let (status, body) = eval(json!({ "value": 5, "values": [4, 6] })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 4);
}

#[tokio::test]
async fn singleton_returns_only_element() {
    let (status, body) = eval(json!({ "value": 100, "values": [-3] })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], -3);
}

#[tokio::test]
async fn handles_negative_values() {
    let (status, body) = eval(json!({ "value": -8, "values": [-10, -1, 3] })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], -10);
}

// --- Error / edge cases ---

#[tokio::test]
async fn non_integer_element_is_422() {
    let (status, _body) = eval(json!({ "value": 5, "values": [1, 3.5] })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn non_integer_value_is_422() {
    let (status, _body) = eval(json!({ "value": 5.5, "values": [1, 4, 9] })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn string_element_is_422() {
    let (status, _body) = eval(json!({ "value": 5, "values": [1, "4"] })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn empty_values_is_422() {
    let (status, _body) = eval(json!({ "value": 5, "values": [] })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn missing_values_field_is_422() {
    let res = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "value": 5 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn generates_request_id_when_absent() {
    let res = app()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        res.headers().contains_key("x-request-id"),
        "response must carry a generated x-request-id"
    );
}
