/// 공유 Excel/CSV 유틸리티 모듈
use axum::{body::Body, http::{header, StatusCode}, response::Response};
use calamine::{DataType, Reader, Xlsx};
use std::collections::HashMap;
use std::io::Cursor;

// ── 공개 API ──────────────────────────────────────────────────────

/// 파일 바이트 → 헤더 제외 행 목록 (xlsx/CSV 자동 판별)
/// 기존 호환성 유지용 래퍼 — 내부적으로 parse_file_rows_with_headers 사용
pub fn parse_file_rows(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    let (_, rows) = parse_file_rows_with_headers(bytes)?;
    Ok(rows)
}

/// 파일 바이트 → (헤더 행, 데이터 행 목록)
/// 헤더 이름 기반 파싱에 사용
pub fn parse_file_rows_with_headers(
    bytes: &[u8],
) -> anyhow::Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut all = if is_xlsx(bytes) {
        parse_xlsx_all_rows(bytes)?
    } else {
        parse_csv_all_rows(bytes)?
    };

    if all.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let headers = all.remove(0);
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

    let rows = range
        .rows()
        .filter(|row| !row.iter().all(|c| matches!(c, DataType::Empty)))
        .map(|row| row.iter().map(cell_to_str).collect())
        .collect();
    Ok(rows)
}

/// CSV 전체 행 (헤더 포함)
fn parse_csv_all_rows(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    let content = decode_bytes(bytes);
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
pub fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(b"\xef\xbb\xbf") {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let (cow, _, _) = encoding_rs::EUC_KR.decode(bytes);
    cow.into_owned()
}

fn cell_to_str(cell: &DataType) -> String {
    match cell {
        DataType::String(s) => s.trim().to_string(),
        DataType::Float(f) => {
            if f.fract() == 0.0 {
                (*f as i64).to_string()
            } else {
                f.to_string()
            }
        }
        DataType::Int(i) => i.to_string(),
        DataType::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
