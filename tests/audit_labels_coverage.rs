//! `AuditAction` 변형과 프론트 감사 라벨(`auditLabels.js`)이 어긋나지 않는지 대조한다.
//!
//! 라벨이 빠지면 조용히 망가진다 — 감사 기록 표는 `APP_INFO_UPDATED` 같은 원문을 그대로
//! 뿌리고(`AUDIT_ACTION_LABELS[row.action] || row.action` 폴백), 액션 필터 드롭다운은
//! 그 항목을 아예 만들지 않아 **해당 행위로 걸러 볼 수가 없다.** 서버는 정상 동작하므로
//! 테스트가 없으면 아무도 모른다. 실제로 `APP_INFO_UPDATED`(이슈 #23)가 그렇게 빠졌다.
//!
//! enum 을 소스에서 읽는 이유: 런타임에 변형을 열거할 방법이 없다(strum 미사용).
//! 라벨 파일도 JS 라 Rust 에서 import 할 수 없으므로 양쪽을 텍스트로 읽어 대조한다.

use std::collections::BTreeSet;

/// `src/enums.rs` 의 `AuditAction` 블록에서 변형 이름을 뽑아 SCREAMING_SNAKE_CASE 로 바꾼다.
/// enum 에 `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` 가 걸려 있어 DB·API 표기가 이것이다.
fn enum_actions() -> BTreeSet<String> {
    let src = include_str!("../src/enums.rs");
    let start = src
        .find("pub enum AuditAction {")
        .expect("AuditAction 선언을 찾지 못했다");
    let body_start = start + src[start..].find('{').unwrap() + 1;
    let body_end = body_start + src[body_start..].find('}').expect("enum 본문의 끝");

    src[body_start..body_end]
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .map(|l| l.trim_end_matches(',').trim())
        .filter(|l| !l.is_empty())
        .map(to_screaming_snake)
        .collect()
}

fn to_screaming_snake(variant: &str) -> String {
    let mut out = String::new();
    for (i, c) in variant.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_uppercase());
    }
    out
}

/// `auditLabels.js` 의 키를 뽑는다. 값(한국어 문구)은 보지 않는다 — 문구 자체는 사람이 정한다.
fn label_keys() -> BTreeSet<String> {
    let src = include_str!("../frontend/src/data/auditLabels.js");
    src.lines()
        .map(|l| l.trim())
        .filter(|l| !l.starts_with("//"))
        .filter_map(|l| l.split_once(':'))
        .map(|(key, _)| key.trim().to_string())
        .filter(|k| !k.is_empty() && k.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .collect()
}

#[test]
fn every_audit_action_has_a_korean_label() {
    let actions = enum_actions();
    let labels = label_keys();

    assert!(actions.len() > 20, "enum 파싱이 깨졌다 (변형 {}개)", actions.len());

    let missing: Vec<_> = actions.difference(&labels).collect();
    assert!(
        missing.is_empty(),
        "감사 라벨이 빠진 액션: {missing:?}\n\
         frontend/src/data/auditLabels.js 에 한국어 문구를 추가하라 — \
         없으면 표에 영문 원문이 뜨고 필터 드롭다운에서도 빠진다"
    );
}

/// 반대 방향 — enum 에서 지운 액션의 라벨이 남아 있으면 필터에 죽은 항목이 뜬다.
#[test]
fn no_label_without_a_matching_audit_action() {
    let actions = enum_actions();
    let labels = label_keys();

    let orphans: Vec<_> = labels.difference(&actions).collect();
    assert!(
        orphans.is_empty(),
        "AuditAction 에 없는 라벨: {orphans:?}\n\
         필터 드롭다운에 고를 수는 있지만 결과가 늘 비는 항목이 된다"
    );
}
