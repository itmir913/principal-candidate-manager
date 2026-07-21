use axum::{
    body::{to_bytes, Body},
    extract::Multipart,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use principal_candidate_manager::middleware::{multipart_err, with_upload_guards};
use tower::ServiceExt;

/// import 핸들러들이 실제로 쓰는 패턴(`multipart_err`로 변환)을 그대로 재현한다.
fn test_router() -> Router {
    async fn accept(mut mp: Multipart) -> axum::response::Response {
        loop {
            match mp.next_field().await.map_err(multipart_err) {
                Ok(Some(field)) => {
                    if let Err(e) = field.bytes().await.map_err(multipart_err) {
                        return e.into_response();
                    }
                }
                Ok(None) => return StatusCode::OK.into_response(),
                Err(e) => return e.into_response(),
            }
        }
    }

    with_upload_guards(Router::new().route("/upload", post(accept)))
}

fn multipart_body(boundary: &str, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"data.bin\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(payload);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

#[tokio::test]
async fn upload_within_limit_succeeds() {
    let boundary = "boundary42";
    let payload = vec![0u8; 200 * 1024]; // 200KB
    let body = multipart_body(boundary, &payload);

    let req = Request::builder()
        .method("POST")
        .uri("/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let res = test_router().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn upload_exceeding_20mb_returns_413_with_korean_message() {
    let boundary = "boundary42";
    let payload = vec![0u8; 21 * 1024 * 1024]; // 21MB > 20MB 상한
    let body = multipart_body(boundary, &payload);

    let req = Request::builder()
        .method("POST")
        .uri("/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let res = test_router().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("크기"), "한국어 안내 메시지가 포함되어야 함: {text}");
    assert!(text.contains("20MB"), "상한 크기가 명시되어야 함: {text}");
}
