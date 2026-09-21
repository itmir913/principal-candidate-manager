//! 백엔드 enum 변형과 **프론트가 손으로 옮겨 적은 목록**이 어긋나지 않는지 대조한다.
//!
//! `audit_labels_coverage.rs` 가 `AuditAction` 에 대해 하던 것을 나머지 enum 으로 넓혔다.
//! 계기: 2026-09-21 프론트 테스트 감사에서 두 가지가 드러났다.
//!
//! 1. `roundStatus.test.js` 가 "변형이 늘면 여기서 먼저 걸린다"고 적어 두었지만
//!    그 파일은 Rust 를 읽지 않는다. `RoundStatus::Archived` 를 추가해도 조용히 통과한다.
//! 2. `areaSamples.test.js` 는 calc_type·match_mode·category_agg·lookup_scope 를
//!    **손으로 복사한 배열 4벌**로 전수 검사한다. 배열이 낡으면 새 조합을 검사하지 않고도
//!    초록이 뜬다 — "손복사를 걷어낸다"던 작업이 새 파일에서 손복사를 다시 만든 셈이다.
//!
//! 여기서 막는 것: 백엔드에 변형이 늘었는데 프론트 목록이 그대로인 경우, 그리고 그 반대.
//! 라벨 **문구**는 보지 않는다(사람이 정한다). 보는 것은 키 집합뿐이다.

use std::collections::BTreeSet;

mod common;
use common::enum_variants;

/// 프론트 파일에서 대문자 상수 토큰을 뽑는다. 주석 줄은 건너뛴다.
/// `re` 대신 단순 스캐너를 쓰는 이유: 의존성을 늘리지 않으려는 것이고,
/// 대상이 `'UPPER'` 같은 따옴표 문자열과 `OPEN:` 같은 객체 키뿐이라 충분하다.
fn upper_tokens(src: &str, block: &str) -> BTreeSet<String> {
    let start = src
        .find(block)
        .unwrap_or_else(|| panic!("프론트 파일에서 `{block}` 을 찾지 못했다"));
    // 배열(`= [...]`)이든 객체(`= {...}`)든 받는다. **둘 중 앞선 것**을 열린 괄호로 본다 —
    // `or_else` 로 `[` 를 먼저 찾으면 파일 뒤쪽의 인덱싱(`LABELS[status]`)을 집어
    // 엉뚱한 구간을 읽는다(실제로 그렇게 빈 집합이 나왔다).
    let rest = &src[start..];
    let body_start = start
        + [rest.find('['), rest.find('{')]
            .into_iter()
            .flatten()
            .min()
            .expect("블록의 열린 괄호");
    let open = src.as_bytes()[body_start];
    let close = if open == b'[' { ']' } else { '}' };
    let body_end = body_start + src[body_start..].find(close).expect("블록의 끝");

    let mut out = BTreeSet::new();
    let mut cur = String::new();
    for line in src[body_start..body_end].lines() {
        let line = line.trim();
        if line.starts_with("//") {
            continue;
        }
        for c in line.chars() {
            if c.is_ascii_uppercase() || c == '_' {
                cur.push(c);
            } else {
                if cur.len() > 1 {
                    out.insert(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
        }
        if cur.len() > 1 {
            out.insert(std::mem::take(&mut cur));
        } else {
            cur.clear();
        }
    }
    out
}

fn assert_same(enum_name: &str, front_file: &str, front: BTreeSet<String>, hint: &str) {
    let back = enum_variants(enum_name);
    assert!(!back.is_empty(), "{enum_name} 파싱이 깨졌다");

    let missing: Vec<_> = back.difference(&front).collect();
    assert!(
        missing.is_empty(),
        "{enum_name} 에 있는데 {front_file} 에 없다: {missing:?}\n{hint}"
    );

    let orphans: Vec<_> = front.difference(&back).collect();
    assert!(
        orphans.is_empty(),
        "{front_file} 에 있는데 {enum_name} 에 없다: {orphans:?}\n\
         백엔드에서 사라진 값이다 — 프론트 목록에서 지워라"
    );
}

#[test]
fn round_status_labels_cover_every_variant() {
    let src = include_str!("../frontend/src/data/roundStatus.js");
    assert_same(
        "RoundStatus",
        "frontend/src/data/roundStatus.js",
        upper_tokens(src, "export const ROUND_STATUS_LABELS"),
        "라벨이 없으면 roundStatusLabel 이 영문 원문을 그대로 화면에 뿌린다.\n\
         매뉴얼(frontend/public/sections/*.html)도 같은 표기를 쓰므로 함께 고쳐야 한다.",
    );
}

// ── areaSamples.test.js 의 손복사 배열 4벌 ────────────────────────────────
//
// 이 배열들은 "가능한 모든 전형요소 조합에서 예시가 나오는가"를 전수 검사하는 입력이다.
// 배열이 낡으면 새 조합을 **검사하지 않은 채** 초록이 뜨고, 화면에서는
// getScoreExample 이 폴백 없이 터진다(areaSamples.js).

const AREA_SAMPLES: &str = include_str!("../frontend/src/data/areaSamples.test.js");
const AREA_FILE: &str = "frontend/src/data/areaSamples.test.js";
const AREA_HINT: &str = "areaSamples.test.js 의 배열에 추가하고, \
                         areaSamples.js 의 SCORE_EXAMPLES/BASE_EXAMPLES 에 예시도 넣어라";

#[test]
fn calc_types_match() {
    assert_same("CalcType", AREA_FILE, upper_tokens(AREA_SAMPLES, "const CALC_TYPES"), AREA_HINT);
}

#[test]
fn match_modes_match() {
    assert_same("MatchMode", AREA_FILE, upper_tokens(AREA_SAMPLES, "const MATCH_MODES"), AREA_HINT);
}

#[test]
fn category_aggs_match() {
    assert_same("CategoryAgg", AREA_FILE, upper_tokens(AREA_SAMPLES, "const CATEGORY_AGGS"), AREA_HINT);
}

#[test]
fn lookup_scopes_match() {
    assert_same("LookupScope", AREA_FILE, upper_tokens(AREA_SAMPLES, "const SCOPES"), AREA_HINT);
}
