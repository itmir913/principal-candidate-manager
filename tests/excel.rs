use principal_candidate_manager::excel::{
    col_map, decode_bytes, get_col, is_xlsx, parse_file_rows, parse_file_rows_with_headers,
    require_cols,
};

#[test]
fn is_xlsx_true() {
    assert!(is_xlsx(b"PK\x03\x04some_zip_content"));
}

#[test]
fn is_xlsx_false() {
    assert!(!is_xlsx(b"name,score\nalice,100"));
}

#[test]
fn decode_bytes_strips_utf8_bom() {
    let bytes: Vec<u8> = b"\xef\xbb\xbfhello".to_vec();
    assert_eq!(decode_bytes(&bytes), "hello");
}

#[test]
fn decode_bytes_plain_utf8() {
    assert_eq!(decode_bytes(b"hello world"), "hello world");
}

#[test]
fn parse_file_rows_csv_skips_header() {
    let csv = b"name,score\nalice,100\nbob,200";
    let rows = parse_file_rows(csv).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["alice", "100"]);
    assert_eq!(rows[1], vec!["bob", "200"]);
}

#[test]
fn parse_file_rows_with_headers_csv() {
    let csv = b"name,score\nalice,100\nbob,200";
    let (headers, rows) = parse_file_rows_with_headers(csv).unwrap();
    assert_eq!(headers, vec!["name", "score"]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["alice", "100"]);
}

#[test]
fn col_map_and_get_col() {
    let headers = vec!["이름".to_string(), "학년".to_string(), "반".to_string()];
    let map = col_map(&headers);
    let row = vec!["홍길동".to_string(), "1".to_string(), "3".to_string()];
    assert_eq!(get_col(&row, &map, "이름"), "홍길동");
    assert_eq!(get_col(&row, &map, "학년"), "1");
    assert_eq!(get_col(&row, &map, "없는열"), "");
}

#[test]
fn require_cols_ok() {
    let headers = vec!["학번".to_string(), "이름".to_string()];
    let map = col_map(&headers);
    assert!(require_cols(&map, &["학번", "이름"]).is_ok());
}

#[test]
fn require_cols_missing() {
    let headers = vec!["이름".to_string()];
    let map = col_map(&headers);
    let err = require_cols(&map, &["학번", "이름"]).unwrap_err();
    assert!(err.contains("학번"));
}
