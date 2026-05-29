/// 공유 Excel/CSV 유틸리티 모듈
use axum::{body::Body, http::{header, StatusCode}, response::Response};
use calamine::{DataType, Reader, Xlsx};
use std::io::Cursor;

// ── 공개 API ──────────────────────────────────────────────────────

/// 파일 바이트 → 헤더 제외 행 목록 (xlsx/CSV 자동 판별)
pub fn parse_file_rows(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    if is_xlsx(bytes) {
        parse_xlsx_rows(bytes)
    } else {
        parse_csv_rows(bytes)
    }
}

/// xlsx 여부 판별 (ZIP PK 매직 바이트)
pub fn is_xlsx(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK")
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

fn parse_xlsx_rows(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
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
        .skip(1)
        .filter(|row| !row.iter().all(|c| matches!(c, DataType::Empty)))
        .map(|row| row.iter().map(cell_to_str).collect())
        .collect();
    Ok(rows)
}

fn parse_csv_rows(bytes: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    let content = decode_bytes(bytes);
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
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
}
