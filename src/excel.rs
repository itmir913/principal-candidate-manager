/// 공유 Excel/CSV 유틸리티 모듈
use axum::{body::Body, http::{header, StatusCode}, response::Response};
use calamine::{DataType, Reader, Xlsx};
use std::collections::HashMap;
use std::io::Cursor;

// ── 공개 API ──────────────────────────────────────────────────────

/// 파일 바이트 → 헤더 제외 행 목록 (xlsx/CSV 자동 판별)
/// 기존 호환성 유지용 래퍼 — 테스트에서만 사용
#[allow(dead_code)]
pub fn parse_file_rows(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    let (_, rows) = parse_file_rows_with_headers(bytes)?;
    Ok(rows)
}

/// 파일 바이트 → (헤더 행, 데이터 행 목록)
/// 헤더 이름 기반 파싱에 사용
pub fn parse_file_rows_with_headers(
    bytes: &[u8],
) -> anyhow::Result<(Vec<String>, Vec<Vec<String>>)> {
    if is_xls(bytes) {
        anyhow::bail!("'.xls' 형식은 지원하지 않습니다. Excel에서 '다른 이름으로 저장 → .xlsx'로 변환 후 업로드해 주세요.");
    }
    let mut all = if is_xlsx(bytes) {
        parse_xlsx_all_rows(bytes)?
    } else {
        parse_csv_all_rows(bytes)?
    };

    if all.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let headers = all.remove(0);

    // 중복 헤더 감지 — 동일한 컬럼명이 두 번 이상이면 어느 열을 사용하는지 불명확
    let mut seen_headers = std::collections::HashSet::new();
    let mut duplicates: Vec<String> = Vec::new();
    for h in &headers {
        let trimmed = h.trim().to_string();
        if trimmed.is_empty() { continue; }
        if !seen_headers.insert(trimmed.clone()) && !duplicates.contains(&trimmed) {
            duplicates.push(trimmed);
        }
    }
    if !duplicates.is_empty() {
        anyhow::bail!("헤더 행에 중복된 열 이름이 있습니다: {}", duplicates.join(", "));
    }

    let data = all
        .into_iter()
        .filter(|row| !row.iter().all(|c| c.is_empty()))
        .collect();
    Ok((headers, data))
}

/// 헤더 목록 → 열이름:인덱스 맵 (공백 trim 적용)
pub fn col_map(headers: &[String]) -> HashMap<String, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim().to_string(), i))
        .collect()
}

/// col_map으로 열 값 추출 (trim 적용, 열 없으면 빈 문자열)
pub fn get_col<'a>(
    cols: &'a [String],
    map: &HashMap<String, usize>,
    name: &str,
) -> &'a str {
    map.get(name)
        .and_then(|&i| cols.get(i))
        .map(|s| s.trim())
        .unwrap_or("")
}

/// 필수 열이 모두 존재하는지 검증 — 누락 시 오류 문자열 반환
pub fn require_cols(map: &HashMap<String, usize>, required: &[&str]) -> Result<(), String> {
    let missing: Vec<&str> = required
        .iter()
        .filter(|&&n| !map.contains_key(n))
        .copied()
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("필수 열 누락: {}", missing.join(", ")))
    }
}

/// xlsx 여부 판별 (ZIP PK 매직 바이트)
pub fn is_xlsx(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK")
}

/// xls 여부 판별 (OLE2 매직 바이트)
pub fn is_xls(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\xD0\xCF\x11\xE0")
}

/// xlsx 전체 행 (빈 행 필터 없음 — 외부 양식 파싱용)
pub fn parse_xlsx_all_rows_raw(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut wb: Xlsx<_> = Xlsx::new(cursor)?;
    let sheet = wb
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("시트가 없습니다"))?;
    let range = wb
        .worksheet_range(&sheet)
        .ok_or_else(|| anyhow::anyhow!("시트를 열 수 없습니다"))??;
    range
        .rows()
        .map(|row| {
            row.iter()
                .map(cell_to_str)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!(e))
        })
        .collect()
}

/// xlsx 특정 시트의 전체 행. 다중 시트 통합문서(내보내기 요약 등) 검증용 —
/// `parse_xlsx_all_rows_raw` 는 첫 시트만 읽으므로 두 번째 이후 시트에는 쓸 수 없다.
/// 시트가 없으면 Err (조용히 빈 결과를 돌려주면 검증이 통과해버린다).
pub fn parse_xlsx_sheet_rows(bytes: &[u8], sheet_name: &str) -> anyhow::Result<Vec<Vec<String>>> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut wb: Xlsx<_> = Xlsx::new(cursor)?;
    let range = wb
        .worksheet_range(sheet_name)
        .ok_or_else(|| anyhow::anyhow!("시트 '{}' 를 찾을 수 없습니다", sheet_name))??;
    range
        .rows()
        .map(|row| {
            row.iter()
                .map(cell_to_str)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!(e))
        })
        .collect()
}

/// xls 전체 행 (빈 행 필터 없음 — 외부 양식 파싱용)
pub fn parse_xls_all_rows_raw(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    use calamine::Xls;
    let cursor = Cursor::new(bytes.to_vec());
    let mut wb: Xls<_> = Xls::new(cursor)?;
    let sheet = wb
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("시트가 없습니다"))?;
    let range = wb
        .worksheet_range(&sheet)
        .ok_or_else(|| anyhow::anyhow!("시트를 열 수 없습니다"))??;
    range
        .rows()
        .map(|row| {
            row.iter()
                .map(cell_to_str)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!(e))
        })
        .collect()
}

/// rust_xlsxwriter 셀 쓰기 오류 → 500 ApiError.
/// `.ok()`로 무시하면 해당 셀만 조용히 비어 내보내기 파일에 점수·데이터가
/// 누락된다 (Fail-Fast 정책 위반) — 반드시 이 헬퍼로 전파할 것.
pub fn xlsx_err(e: rust_xlsxwriter::XlsxError) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("xlsx 생성 오류: {}", e))
}

/// 현재 로컬 시각을 `YYYYMMDD_HHMMSS` 형식으로 반환
pub fn now_tag() -> String {
    chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()
}

/// xlsx 다운로드 응답 생성
pub fn xlsx_response(buf: Vec<u8>, filename: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(buf))
        .unwrap()
}

// ── 내부 구현 ─────────────────────────────────────────────────────

/// xlsx 전체 행 (헤더 포함, 빈 행 제외)
fn parse_xlsx_all_rows(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut wb: Xlsx<_> = Xlsx::new(cursor)?;
    let sheet = wb
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("시트가 없습니다"))?;
    let range = wb
        .worksheet_range(&sheet)
        .ok_or_else(|| anyhow::anyhow!("시트를 열 수 없습니다"))??;

    range
        .rows()
        .filter(|row| !row.iter().all(|c| matches!(c, DataType::Empty)))
        .map(|row| {
            row.iter()
                .map(cell_to_str)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!(e))
        })
        .collect()
}

/// CSV 전체 행 (헤더 포함)
fn parse_csv_all_rows(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    let content = decode_bytes(bytes)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(content.as_bytes());
    rdr.records()
        .map(|r| {
            r.map(|sr| sr.iter().map(str::to_string).collect())
                .map_err(Into::into)
        })
        .collect()
}

/// 인코딩 감지: UTF-8 BOM → UTF-8 → EUC-KR(CP949)
/// 어느 인코딩으로도 깨끗하게 해석되지 않으면 Err — lossy 변환으로
/// 학생 이름 등이 �로 조용히 손상된 채 저장되는 것을 막는다.
pub fn decode_bytes(bytes: &[u8]) -> anyhow::Result<String> {
    if bytes.starts_with(b"\xef\xbb\xbf") {
        return match std::str::from_utf8(&bytes[3..]) {
            Ok(s) => Ok(s.to_string()),
            Err(_) => anyhow::bail!("UTF-8 BOM이 있으나 내용이 올바른 UTF-8이 아닙니다"),
        };
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok(s.to_string());
    }
    let (cow, _, had_errors) = encoding_rs::EUC_KR.decode(bytes);
    if had_errors {
        anyhow::bail!(
            "파일 인코딩을 인식할 수 없습니다 (UTF-8 또는 EUC-KR CSV만 지원). Excel에서 'CSV UTF-8'로 다시 저장해 주세요"
        );
    }
    Ok(cow.into_owned())
}

/// calamine DataType → 문자열 변환. 예상하지 못한 variant는 오류로 승격한다.
///
/// 이전에는 wildcard `_ => String::new()`로 DateTime/Duration/DateTimeIso/DurationIso/Error
/// variant를 조용히 빈 문자열로 만들었다. 학번·점수·대학명 셀에 사용자가 실수로 날짜
/// 서식을 적용하거나 `#REF!` 같은 수식 오류가 있으면 downstream `is_empty()` 체크에
/// 우연히 걸리기만 하고, 특히 `resolve_track`의 `(true, true) => Some(None)` 경로에서는
/// COMPOSITE 트랙 값이 공통 테이블로 조용히 강등 저장되는 실질 사고까지 났었다
/// (2차 감사 B 발견 1). CLAUDE.md §2 Fail-Fast 위반이므로 명시 오류로 승격.
///
/// Empty는 정당한 "빈 셀" 의미이므로 빈 문자열 반환 (호출자가 헤더 인덱스 정렬을
/// 유지할 수 있어야 함 — 실제 값 유무는 downstream이 판정).
fn cell_to_str(cell: &DataType) -> Result<String, String> {
    match cell {
        DataType::String(s) => Ok(s.trim().to_string()),
        DataType::Float(f) => Ok(if f.fract() == 0.0 {
            (*f as i64).to_string()
        } else {
            f.to_string()
        }),
        DataType::Int(i) => Ok(i.to_string()),
        DataType::Bool(b) => Ok(if *b { "1" } else { "0" }.to_string()),
        DataType::Empty => Ok(String::new()),
        DataType::DateTime(_) | DataType::DateTimeIso(_) => Err(
            "날짜 서식 셀은 지원되지 않습니다. 해당 셀을 텍스트나 숫자 서식으로 바꾸고 다시 업로드하세요".to_string()
        ),
        DataType::Duration(_) | DataType::DurationIso(_) => Err(
            "시간(duration) 서식 셀은 지원되지 않습니다. 해당 셀을 텍스트나 숫자 서식으로 바꾸고 다시 업로드하세요".to_string()
        ),
        DataType::Error(e) => Err(
            format!("셀에 수식 오류({:?})가 있습니다. 원본 파일에서 오류를 수정한 후 다시 업로드하세요", e)
        ),
    }
}

#[cfg(test)]
mod cell_to_str_tests {
    //! DataType variant fail-fast 검증 (2차 감사 B 발견 1 소유자 라운드 #2).
    //! `parse_xlsx_all_rows_raw` 경로로는 rust_xlsxwriter가 셀 서식만 지정하고
    //! 값은 f64로 저장하므로 calamine이 `DataType::DateTime`을 만들 수 없다.
    //! 따라서 실제 사용자가 만든 xlsx에서 발생하는 DateTime/Error variant는
    //! `cell_to_str`에 직접 주입해 검증한다.
    use super::*;
    use calamine::CellErrorType;

    #[test]
    fn string_ok() {
        assert_eq!(cell_to_str(&DataType::String("hi".into())).unwrap(), "hi");
    }

    #[test]
    fn integer_float_bool_ok() {
        assert_eq!(cell_to_str(&DataType::Int(42)).unwrap(), "42");
        assert_eq!(cell_to_str(&DataType::Float(3.5)).unwrap(), "3.5");
        assert_eq!(cell_to_str(&DataType::Float(5.0)).unwrap(), "5"); // fract==0 → 정수
        assert_eq!(cell_to_str(&DataType::Bool(true)).unwrap(), "1");
        assert_eq!(cell_to_str(&DataType::Bool(false)).unwrap(), "0");
    }

    #[test]
    fn empty_returns_empty_string() {
        // Empty는 "빈 셀" 정당한 의미 — 오류가 아니라 빈 문자열
        assert_eq!(cell_to_str(&DataType::Empty).unwrap(), "");
    }

    #[test]
    fn datetime_variants_return_error() {
        // 이전에는 wildcard로 조용히 "" 반환 → resolve_track 공통 강등 사고
        let err = cell_to_str(&DataType::DateTime(45672.0)).unwrap_err();
        assert!(err.contains("날짜"), "메시지: {}", err);
        let err = cell_to_str(&DataType::DateTimeIso("2025-01-15".into())).unwrap_err();
        assert!(err.contains("날짜"), "메시지: {}", err);
    }

    #[test]
    fn duration_variants_return_error() {
        let err = cell_to_str(&DataType::Duration(1.5)).unwrap_err();
        assert!(err.contains("시간") || err.contains("duration"), "메시지: {}", err);
        let err = cell_to_str(&DataType::DurationIso("PT1H".into())).unwrap_err();
        assert!(err.contains("시간") || err.contains("duration"), "메시지: {}", err);
    }

    #[test]
    fn cell_error_variant_returns_error() {
        // #REF!, #DIV/0! 같은 수식 오류 셀
        let err = cell_to_str(&DataType::Error(CellErrorType::Ref)).unwrap_err();
        assert!(err.contains("수식") || err.contains("오류"), "메시지: {}", err);
        let err = cell_to_str(&DataType::Error(CellErrorType::Div0)).unwrap_err();
        assert!(err.contains("수식") || err.contains("오류"), "메시지: {}", err);
    }
}
