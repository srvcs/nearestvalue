use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::{OpenApi, ToSchema};

/// This service's identity. `srvcs-nearestvalue` is a leaf: it depends on no
/// other service. It answers a single range question over a list of integers
/// entirely with local logic.
pub const SERVICE: &str = "srvcs-nearestvalue";
pub const CONCERN: &str = "range: the element of a list nearest to value";
pub const DEPENDS_ON: &[&str] = &[];

#[derive(Serialize, ToSchema)]
pub struct Info {
    pub service: &'static str,
    pub concern: &'static str,
    pub depends_on: Vec<&'static str>,
}

/// `GET /` — service identity (srvcs service standard).
#[utoipa::path(get, path = "/", responses((status = 200, body = Info)))]
pub async fn index() -> Json<Info> {
    Json(Info {
        service: SERVICE,
        concern: CONCERN,
        depends_on: DEPENDS_ON.to_vec(),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct EvalRequest {
    /// The reference value. Must be a JSON integer (i64).
    #[schema(value_type = Object)]
    pub value: Value,
    /// The non-empty list of candidates. Every element must be a JSON integer.
    #[schema(value_type = Object)]
    pub values: Vec<Value>,
}

#[derive(Serialize, ToSchema)]
pub struct NearestValueResponse {
    pub value: i64,
    pub values: Vec<i64>,
    pub result: i64,
}

/// Read a JSON value as an `i64`, rejecting any non-integer number or non-number.
fn as_i64(v: &Value) -> Option<i64> {
    v.as_i64()
}

/// The single concern: which element of `values` is nearest to `value`?
///
/// Returns `None` if `value` or any element is not a JSON integer, or if
/// `values` is empty. Otherwise returns the `(value, values, result)` tuple
/// where `result` minimizes `(element - value).abs()`. On a tie the element
/// that appears first wins.
pub fn nearest_value(value: &Value, values: &[Value]) -> Option<(i64, Vec<i64>, i64)> {
    let value = as_i64(value)?;
    if values.is_empty() {
        return None;
    }
    let parsed: Vec<i64> = values.iter().map(as_i64).collect::<Option<Vec<i64>>>()?;

    let mut best = parsed[0];
    let mut best_dist = (best as i128 - value as i128).abs();
    for &candidate in &parsed[1..] {
        let dist = (candidate as i128 - value as i128).abs();
        if dist < best_dist {
            best = candidate;
            best_dist = dist;
        }
    }
    Some((value, parsed, best))
}

/// `POST /` — the element of `values` nearest to `value`.
///
/// Reads `value` and each element of `values` as a JSON integer (`i64`). If any
/// is not an integer, or `values` is empty, the request is rejected with `422`.
/// Otherwise the element minimizing `(element - value).abs()` is returned; ties
/// resolve to the element that appears first.
#[utoipa::path(
    post,
    path = "/",
    request_body = EvalRequest,
    responses(
        (status = 200, body = NearestValueResponse),
        (status = 422, description = "value or an element is not an integer, or values is empty")
    )
)]
pub async fn evaluate(Json(req): Json<EvalRequest>) -> Response {
    match nearest_value(&req.value, &req.values) {
        Some((value, values, result)) => (
            StatusCode::OK,
            Json(json!({ "value": value, "values": values, "result": result })),
        )
            .into_response(),
        None => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "value and all values must be integers, and values must be non-empty"
            })),
        )
            .into_response(),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(index, evaluate),
    components(schemas(Info, EvalRequest, NearestValueResponse))
)]
pub struct ApiDoc;

/// Serve OpenAPI document
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_documents_routes() {
        let doc = ApiDoc::openapi();
        let root = doc.paths.paths.get("/").expect("path / present");
        assert!(root.get.is_some(), "GET / documented");
        assert!(root.post.is_some(), "POST / documented");
    }

    #[test]
    fn index_reports_identity() {
        assert_eq!(SERVICE, "srvcs-nearestvalue");
        assert_eq!(CONCERN, "range: the element of a list nearest to value");
        assert!(DEPENDS_ON.is_empty());
    }

    #[test]
    fn asserted_cases() {
        assert_eq!(
            nearest_value(&json!(5), &[json!(1), json!(4), json!(9)]),
            Some((5, vec![1, 4, 9], 4))
        );
        assert_eq!(
            nearest_value(&json!(7), &[json!(1), json!(4), json!(9)]),
            Some((7, vec![1, 4, 9], 9))
        );
    }

    #[test]
    fn tie_returns_first() {
        // distance to 4 and 6 from 5 is equal; the first one wins.
        assert_eq!(
            nearest_value(&json!(5), &[json!(4), json!(6)]),
            Some((5, vec![4, 6], 4))
        );
        assert_eq!(
            nearest_value(&json!(5), &[json!(6), json!(4)]),
            Some((5, vec![6, 4], 6))
        );
    }

    #[test]
    fn singleton_returns_only_element() {
        assert_eq!(
            nearest_value(&json!(100), &[json!(-3)]),
            Some((100, vec![-3], -3))
        );
    }

    #[test]
    fn empty_values_is_rejected() {
        assert_eq!(nearest_value(&json!(5), &[]), None);
    }

    #[test]
    fn non_integer_value_is_rejected() {
        for bad in [json!(3.5), json!("5"), json!(true), json!(null), json!([1])] {
            assert_eq!(nearest_value(&bad, &[json!(1)]), None, "{bad} value");
        }
    }

    #[test]
    fn non_integer_element_is_rejected() {
        for bad in [json!(3.5), json!("4"), json!(null), json!(true)] {
            assert_eq!(
                nearest_value(&json!(5), &[json!(1), bad.clone()]),
                None,
                "{bad} element"
            );
        }
    }

    #[tokio::test]
    async fn evaluate_returns_200_with_result() {
        let resp = evaluate(Json(EvalRequest {
            value: json!(5),
            values: vec![json!(1), json!(4), json!(9)],
        }))
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn evaluate_returns_422_for_non_integer() {
        let resp = evaluate(Json(EvalRequest {
            value: json!(5),
            values: vec![json!(1), json!(3.5)],
        }))
        .await;
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn index_reports_identity_over_http() {
        let Json(info) = index().await;
        assert_eq!(info.service, "srvcs-nearestvalue");
        assert!(info.depends_on.is_empty());
    }
}
