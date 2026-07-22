use principal_candidate_manager::excel::{
    col_map, decode_bytes, get_col, is_xls, is_xlsx, now_tag, parse_file_rows,
    parse_file_rows_with_headers, parse_xls_all_rows_raw, require_cols, xlsx_err,
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
    assert_eq!(decode_bytes(&bytes).unwrap(), "hello");
}

#[test]
fn decode_bytes_plain_utf8() {
    assert_eq!(decode_bytes(b"hello world").unwrap(), "hello world");
}

#[test]
fn decode_bytes_euc_kr() {
    // "한글" in EUC-KR: C7 D1 B1 DB
    let bytes: Vec<u8> = vec![0xC7, 0xD1, 0xB1, 0xDB];
    assert_eq!(decode_bytes(&bytes).unwrap(), "한글");
}

#[test]
fn decode_bytes_unknown_encoding_rejected() {
    // UTF-8도 EUC-KR도 아닌 바이트열 → 과거에는 �로 조용히 손상된 채 통과
    let bytes: Vec<u8> = vec![0xFF, 0xFF, 0xFF];
    let err = decode_bytes(&bytes);
    assert!(err.is_err(), "인식 불가 인코딩은 거부되어야 함");
    assert!(err.unwrap_err().to_string().contains("인코딩"));
}

#[test]
fn decode_bytes_bom_with_invalid_utf8_rejected() {
    // BOM 뒤에 깨진 UTF-8 → 과거에는 lossy 변환으로 조용히 통과
    let bytes: Vec<u8> = vec![0xEF, 0xBB, 0xBF, 0xFF, 0xFE];
    assert!(decode_bytes(&bytes).is_err());
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

// ── 다운로드 부가 유틸 (2026-07-23 전수 커버리지 마무리) ──────────

#[test]
fn is_xls_recognises_ole2_and_rejects_others() {
    // OLE2(BIFF .xls) 매직 바이트. 이 판별이 무너지면 .xls 업로드가
    // "지원하지 않는 형식" 안내 대신 파서 내부 오류로 튄다.
    assert!(is_xls(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1rest"));
    assert!(!is_xls(b"PK\x03\x04"), "xlsx를 xls로 보면 안 된다");
    assert!(!is_xls(b"name,score\n"), "CSV를 xls로 보면 안 된다");
    assert!(!is_xls(b""), "빈 파일");
    assert!(!is_xls(b"\xD0\xCF"), "매직이 잘린 파일");
}

#[test]
fn parse_xls_all_rows_raw_errors_on_corrupt_file() {
    // 매직만 흉내 낸 손상 파일이 "0행"으로 조용히 통과하면,
    // 석차연명부를 올렸는데 아무 일도 안 일어난 것처럼 보인다 (Fail-Fast 위반).
    let mut fake = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1".to_vec();
    fake.extend_from_slice(&[0u8; 1024]);
    assert!(parse_xls_all_rows_raw(&fake).is_err(), "손상된 .xls는 Err여야 한다");
    assert!(parse_xls_all_rows_raw(b"not an xls at all").is_err());
}

#[test]
fn now_tag_is_a_sortable_timestamp() {
    // 내보내기 파일명이 이 값으로만 구분된다 — 형식이 흔들리면 같은 초에
    // 만든 파일끼리 덮어쓰거나, 파일명 정렬이 시간순이 아니게 된다.
    let tag = now_tag();
    assert_eq!(tag.len(), 15, "YYYYMMDD_HHMMSS: {}", tag);
    let (date, time) = tag.split_once('_').expect("구분자 '_' 없음");
    assert_eq!(date.len(), 8);
    assert_eq!(time.len(), 6);
    assert!(date.chars().all(|c| c.is_ascii_digit()), "{}", tag);
    assert!(time.chars().all(|c| c.is_ascii_digit()), "{}", tag);
    let year: i32 = date[..4].parse().unwrap();
    assert!((2020..2100).contains(&year), "연도가 이상하다: {}", tag);
    let month: u32 = date[4..6].parse().unwrap();
    let day: u32 = date[6..8].parse().unwrap();
    assert!((1..=12).contains(&month) && (1..=31).contains(&day), "{}", tag);
}

#[test]
fn xlsx_err_maps_writer_failure_to_500() {
    // 셀 쓰기 실패를 `.ok()`로 삼키면 그 셀만 조용히 빈 채 내보내진다.
    // 실제 XlsxError를 만들어 500 + 원인 문자열로 승격되는지 고정한다.
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    let e = match ws.write_string(1_048_577, 0, "행 한도 초과") {
        Err(e) => e,
        Ok(_) => panic!("행 한도를 넘기면 XlsxError여야 한다"),
    };
    let (code, msg) = xlsx_err(e);
    assert_eq!(code, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    assert!(msg.contains("xlsx 생성 오류"), "메시지: {}", msg);
    assert!(msg.len() > "xlsx 생성 오류: ".len(), "원인이 비어 있다: {}", msg);
}

#[test]
fn migration_sqls_covers_every_schema_version_fragment() {
    // full_schema_sql()은 이 함수 위에 서 있고, 모든 테스트의 in-memory DB가
    // 그 결과로 만들어진다. 조각 하나가 배열에서 빠져도 "그 테이블을 안 쓰는"
    // 테스트는 전부 초록이므로, 개수와 필수 객체를 여기서 직접 고정한다.
    let sqls = principal_candidate_manager::db::migration_sqls();
    assert_eq!(
        sqls.len(),
        principal_candidate_manager::db::SCHEMA_VERSION as usize,
        "버전 수와 마이그레이션 수가 어긋났다"
    );
    let all = principal_candidate_manager::db::full_schema_sql();
    // `full_schema_sql() == sqls.join("\n")` 는 단언하지 않는다 — 버전이 하나뿐인
    // 지금은 구분자가 무엇이든 결과가 같아서 아무것도 지키지 못하는 항상-참 단언이다
    // (변이 검사에서 실제로 구분자를 바꿔도 잡히지 않았다).
    // 대신 "각 버전 SQL이 비어 있지 않고 전문에 그대로 들어 있다"를 고정한다.
    for (i, sql) in sqls.iter().enumerate() {
        assert!(!sql.trim().is_empty(), "v{} 마이그레이션 SQL이 비었다", i + 1);
        assert!(all.contains(sql.as_str()), "v{} SQL이 전체 스키마에서 빠졌다", i + 1);
    }
    for t in [
        "classes", "students", "rounds", "areas", "universities", "univ_tracks",
        "numeric_table", "category_map", "base_data", "applications", "results",
        "audit_log", "round_confirmations",
    ] {
        assert!(
            all.contains(&format!("CREATE TABLE IF NOT EXISTS {}", t)),
            "'{}' 테이블 생성 SQL이 빠졌다 — 조각 파일이 V1_FRAGMENTS에 등록됐는지 확인",
            t
        );
    }
}
