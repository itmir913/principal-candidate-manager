//! 감사 산출물 — Excel 입력 케이스 판정 고정 (P2 "Excel 케이스 판정").
//!
//! 프롬프트가 요구한 케이스별로 **거부/통과**를 실제 파일을 만들어 확인한다.
//! 판정 근거를 코드 읽기가 아니라 실행으로 남기기 위한 테스트 (증거 등급 E2).
//! 각 테스트 첫 줄 주석 = 그 테스트가 고정하는 불변식.

mod common;

use principal_candidate_manager::excel::{col_map, parse_file_rows_with_headers, require_cols};
use principal_candidate_manager::handlers::area_data::parse_display_value;
use rust_xlsxwriter::{Format, Workbook};

/// 헤더 `기준값,점수` + 주어진 데이터 행으로 xlsx 바이트 생성
fn xlsx_simple(rows: &[[&str; 2]]) -> Vec<u8> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "기준값").unwrap();
    ws.write_string(0, 1, "점수").unwrap();
    for (i, r) in rows.iter().enumerate() {
        ws.write_string(i as u32 + 1, 0, r[0]).unwrap();
        ws.write_string(i as u32 + 1, 1, r[1]).unwrap();
    }
    wb.save_to_buffer().unwrap()
}

// ── 셀 내용 케이스 ───────────────────────────────────────────────

/// 불변식: 문자열로 저장된 숫자·앞뒤 공백/개행/탭은 정상 통과하고,
/// 전각 숫자·지수 표기·소수 6자리·±10억 초과는 Fail-Fast 로 거부된다.
#[test]
fn cell_content_cases_accept_or_reject() {
    // 통과해야 하는 것
    for (s, want) in [
        ("85", 8_500_000i64),
        ("  85  ", 8_500_000),
        ("\t85\n", 8_500_000),
        ("85.5", 8_550_000),
        ("-3.25", -325_000),
        ("0.00001", 1),
        ("1000000000", 100_000_000_000_000),
    ] {
        assert_eq!(parse_display_value(s), Ok(want), "'{s}' 는 통과해야 한다");
    }

    // 거부해야 하는 것
    for s in [
        "８５",          // 전각 숫자
        "1e5",           // 지수 표기
        "2E-3",
        "0.000001",      // 소수 6자리
        "1000000001",    // ±10억 초과
        "-1000000001",
        "nan",
        "inf",
        "",              // 빈 셀
        "8 5",           // 중간 공백
        "85점",
    ] {
        assert!(parse_display_value(s).is_err(), "'{s}' 는 거부되어야 한다");
    }
}

// ── 파일 구조 케이스 ─────────────────────────────────────────────

/// 불변식: 헤더 중복은 즉시 오류, 열 순서 변경은 무해(헤더 이름 기반),
/// 데이터 0행은 헤더만 남고 빈 목록이 된다.
#[test]
fn header_structure_cases() {
    // 헤더 중복 → Err
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in ["기준값", "점수", "점수"].iter().enumerate() {
        ws.write_string(0, i as u16, *h).unwrap();
    }
    ws.write_string(1, 0, "1").unwrap();
    let bytes = wb.save_to_buffer().unwrap();
    let err = parse_file_rows_with_headers(&bytes).unwrap_err().to_string();
    assert!(err.contains("중복된 열 이름"), "헤더 중복은 오류: {err}");

    // 열 순서 변경 → 무해
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "점수").unwrap();
    ws.write_string(0, 1, "기준값").unwrap();
    ws.write_string(1, 0, "10").unwrap();
    ws.write_string(1, 1, "3").unwrap();
    let bytes = wb.save_to_buffer().unwrap();
    let (headers, rows) = parse_file_rows_with_headers(&bytes).unwrap();
    let map = col_map(&headers);
    assert!(require_cols(&map, &["기준값", "점수"]).is_ok());
    assert_eq!(principal_candidate_manager::excel::get_col(&rows[0], &map, "기준값"), "3");
    assert_eq!(principal_candidate_manager::excel::get_col(&rows[0], &map, "점수"), "10");

    // 데이터 0행 → 헤더만, 행 0개 (호출자가 400 으로 막는다)
    let bytes = xlsx_simple(&[]);
    let (headers, rows) = parse_file_rows_with_headers(&bytes).unwrap();
    assert_eq!(headers.len(), 2);
    assert!(rows.is_empty());

    // 헤더 누락 → require_cols 오류
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "기준값").unwrap();
    ws.write_string(1, 0, "3").unwrap();
    let bytes = wb.save_to_buffer().unwrap();
    let (headers, _) = parse_file_rows_with_headers(&bytes).unwrap();
    assert!(require_cols(&col_map(&headers), &["기준값", "점수"]).is_err());
}

/// 불변식: 첫 시트만 읽는다 — 시트 **순서**가 바뀌면 다른 시트를 읽고,
/// 시트 **이름**은 파싱에 영향이 없다.
#[test]
fn only_first_sheet_is_read() {
    let mut wb = Workbook::new();
    let ws1 = wb.add_worksheet().set_name("표지").unwrap();
    ws1.write_string(0, 0, "이 문서는 안내문입니다").unwrap();
    let ws2 = wb.add_worksheet().set_name("기준표").unwrap();
    ws2.write_string(0, 0, "기준값").unwrap();
    ws2.write_string(0, 1, "점수").unwrap();
    ws2.write_string(1, 0, "3").unwrap();
    ws2.write_string(1, 1, "10").unwrap();
    let bytes = wb.save_to_buffer().unwrap();

    let (headers, _) = parse_file_rows_with_headers(&bytes).unwrap();
    assert_eq!(headers[0], "이 문서는 안내문입니다", "첫 시트만 읽는다");
    assert!(
        require_cols(&col_map(&headers), &["기준값", "점수"]).is_err(),
        "안내문 시트가 앞에 있으면 필수 열 누락으로 거부된다(Fail-Fast)"
    );
}

/// 불변식: 숨김 행·숨김 열은 **보이는 셀과 동일하게 읽힌다** (거부되지 않는다).
/// 관리자가 행을 숨겨 제외했다고 믿어도 값은 그대로 반영된다.
#[test]
fn hidden_rows_and_columns_are_still_imported() {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "기준값").unwrap();
    ws.write_string(0, 1, "점수").unwrap();
    ws.write_string(1, 0, "3").unwrap();
    ws.write_string(1, 1, "10").unwrap();
    ws.write_string(2, 0, "5").unwrap();
    ws.write_string(2, 1, "20").unwrap();
    ws.set_row_hidden(2).unwrap();          // 두 번째 데이터 행 숨김
    let bytes = wb.save_to_buffer().unwrap();

    let (_, rows) = parse_file_rows_with_headers(&bytes).unwrap();
    assert_eq!(rows.len(), 2, "숨긴 행도 그대로 읽힌다");
    assert_eq!(rows[1][0], "5");
}

/// 불변식: 병합 셀은 **앵커 셀에만 값이 있고 나머지는 빈 셀**로 읽힌다.
/// 필수 열이 병합돼 있으면 두 번째 행부터 값 누락으로 거부된다(조용한 승계 없음).
#[test]
fn merged_cells_leave_following_rows_empty() {
    let mut wb = Workbook::new();
    let fmt = Format::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "기준값").unwrap();
    ws.write_string(0, 1, "점수").unwrap();
    ws.merge_range(1, 0, 2, 0, "3", &fmt).unwrap();  // 기준값 열 2행 병합
    ws.write_string(1, 1, "10").unwrap();
    ws.write_string(2, 1, "20").unwrap();
    let bytes = wb.save_to_buffer().unwrap();

    let (headers, rows) = parse_file_rows_with_headers(&bytes).unwrap();
    let map = col_map(&headers);
    assert_eq!(rows.len(), 2);
    assert_eq!(principal_candidate_manager::excel::get_col(&rows[0], &map, "기준값"), "3");
    assert_eq!(
        principal_candidate_manager::excel::get_col(&rows[1], &map, "기준값"),
        "",
        "병합의 두 번째 행은 빈 셀 — 앞 행 값이 조용히 승계되지 않는다"
    );
    assert!(parse_display_value("").is_err(), "빈 값은 거부된다");
}

/// 불변식: 수식 셀은 계산된 캐시 값이 없으면 빈 셀로 읽힌다.
/// (rust_xlsxwriter 가 만든 파일에는 캐시 값이 없다 — 실제 Excel 파일은 캐시 값을 갖는다.)
/// 어느 쪽이든 `#REF!` 같은 오류 셀은 `cell_to_str` 이 명시 Err 로 승격한다
/// (src/excel.rs::cell_to_str, 회귀 테스트 src/excel.rs::cell_to_str_tests).
#[test]
fn formula_cell_without_cached_value_reads_as_empty() {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "기준값").unwrap();
    ws.write_string(0, 1, "점수").unwrap();
    ws.write_string(1, 0, "3").unwrap();
    ws.write_formula(1, 1, "=1+1").unwrap();
    let bytes = wb.save_to_buffer().unwrap();

    let (headers, rows) = parse_file_rows_with_headers(&bytes).unwrap();
    let map = col_map(&headers);
    let v = principal_candidate_manager::excel::get_col(&rows[0], &map, "점수");
    assert!(
        v.is_empty() || parse_display_value(v).is_ok(),
        "수식 셀은 빈 값(→ 거부) 또는 캐시된 숫자(→ 통과) 중 하나여야 한다. 실제: '{v}'"
    );
}

/// 불변식: fract==0 인 거대 Float 셀은 `cell_to_str` 의 `as i64` 로 포화되지만
/// 후속 `parse_display_value` 의 ±10억 검사가 이를 거부한다(값 오염 없음).
#[test]
fn huge_float_cell_saturates_but_is_rejected_downstream() {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "기준값").unwrap();
    ws.write_string(0, 1, "점수").unwrap();
    ws.write_number(1, 0, 1e20).unwrap();
    ws.write_string(1, 1, "10").unwrap();
    let bytes = wb.save_to_buffer().unwrap();

    let (headers, rows) = parse_file_rows_with_headers(&bytes).unwrap();
    let map = col_map(&headers);
    let v = principal_candidate_manager::excel::get_col(&rows[0], &map, "기준값");
    assert_eq!(v, i64::MAX.to_string(), "as i64 포화 (excel.rs::cell_to_str Float 분기)");
    assert!(parse_display_value(v).is_err(), "±10억 검사가 거부한다");
}
