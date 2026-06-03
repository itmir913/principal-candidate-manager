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
    let headers = vec!["학생코드".to_string(), "이름".to_string()];
    let map = col_map(&headers);
    assert!(require_cols(&map, &["학생코드", "이름"]).is_ok());
}

#[test]
fn require_cols_missing() {
    let headers = vec!["이름".to_string()];
    let map = col_map(&headers);
    let err = require_cols(&map, &["학생코드", "이름"]).unwrap_err();
    assert!(err.contains("학생코드"));
}

// ── 추가 엣지케이스 ───────────────────────────────────────────────

#[test]
fn parse_file_rows_with_headers_only_header_returns_empty_data() {
    // 헤더 행만 있고 데이터가 없는 CSV → 빈 데이터 행 목록
    let csv = b"name,score\n";
    let (headers, rows) = parse_file_rows_with_headers(csv).unwrap();
    assert_eq!(headers, vec!["name", "score"]);
    assert_eq!(rows.len(), 0);
}

#[test]
fn parse_file_rows_with_headers_completely_empty_returns_empty() {
    // 완전히 빈 파일 → 헤더·데이터 모두 빈 목록
    let csv = b"";
    let (headers, rows) = parse_file_rows_with_headers(csv).unwrap();
    assert!(headers.is_empty());
    assert!(rows.is_empty());
}

#[test]
fn parse_file_rows_with_headers_windows_crlf() {
    // Windows 줄바꿈(CR+LF) → 정상 파싱
    let csv = b"name,score\r\nalice,100\r\nbob,200\r\n";
    let (headers, rows) = parse_file_rows_with_headers(csv).unwrap();
    assert_eq!(headers, vec!["name", "score"]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0], "alice");
    assert_eq!(rows[0][1], "100");
}

#[test]
fn parse_file_rows_with_headers_duplicate_headers_returns_error() {
    // 동일한 열 이름이 두 번 → 오류 (열 매핑이 불명확)
    let csv = b"name,score,name\nalice,100,extra\n";
    let result = parse_file_rows_with_headers(csv);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("중복"));
}

#[test]
fn parse_file_rows_with_headers_blank_rows_are_filtered() {
    // 중간의 완전 빈 행은 결과에서 제외
    let csv = b"name,score\nalice,100\n,\nbob,200\n";
    let (_, rows) = parse_file_rows_with_headers(csv).unwrap();
    assert_eq!(rows.len(), 2);
}

#[test]
fn col_map_trims_header_spaces() {
    // 헤더 이름의 앞뒤 공백은 trim 후 맵에 등록
    let headers = vec![" 이름 ".to_string(), "\t학년".to_string()];
    let map = col_map(&headers);
    assert!(map.contains_key("이름"), "공백 trim 후 키가 존재해야 함");
    assert!(map.contains_key("학년"), "탭 trim 후 키가 존재해야 함");
    assert!(!map.contains_key(" 이름 "), "trim 전 원본 키는 존재하면 안 됨");
}

#[test]
fn get_col_row_shorter_than_map_returns_empty() {
    // 행의 열 수가 헤더보다 적을 때 → 존재하지 않는 열은 빈 문자열
    let headers = vec!["이름".to_string(), "학년".to_string(), "반".to_string()];
    let map = col_map(&headers);
    let row = vec!["홍길동".to_string()]; // 1열만 존재
    assert_eq!(get_col(&row, &map, "이름"), "홍길동");
    assert_eq!(get_col(&row, &map, "학년"), "");
    assert_eq!(get_col(&row, &map, "반"), "");
}

#[test]
fn require_cols_all_missing_lists_each() {
    // 필수 열 전부 누락 → 오류 메시지에 모든 열 이름 포함
    let headers: Vec<String> = vec![];
    let map = col_map(&headers);
    let err = require_cols(&map, &["학년", "반", "이름"]).unwrap_err();
    assert!(err.contains("학년"), "오류 메시지에 '학년' 포함: {err}");
    assert!(err.contains("반"), "오류 메시지에 '반' 포함: {err}");
    assert!(err.contains("이름"), "오류 메시지에 '이름' 포함: {err}");
}
