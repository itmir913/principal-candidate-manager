//! `AuditAction` 변형과 프론트 감사 라벨(`auditLabels.js`)이 어긋나지 않는지 대조한다.
//!
//! 라벨이 빠지면 조용히 망가진다 — 감사 기록 표는 `APP_INFO_UPDATED` 같은 원문을 그대로
//! 뿌리고(`AUDIT_ACTION_LABELS[row.action] || row.action` 폴백), 액션 필터 드롭다운은
//! 그 항목을 아예 만들지 않아 **해당 행위로 걸러 볼 수가 없다.** 서버는 정상 동작하므로
//! 테스트가 없으면 아무도 모른다. 실제로 `APP_INFO_UPDATED`(이슈 #23)가 그렇게 빠졌다.
//!
//! enum 을 소스에서 읽는 이유는 tests/common/mod.rs 의 `enum_variants` 주석에 있다.
//! 같은 대조를 나머지 enum 에 대해 하는 것은 tests/frontend_enum_sync.rs 다.

use std::collections::BTreeSet;

mod common;
use common::enum_variants;

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
    let actions = enum_variants("AuditAction");
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
    let actions = enum_variants("AuditAction");
    let labels = label_keys();

    let orphans: Vec<_> = labels.difference(&actions).collect();
    assert!(
        orphans.is_empty(),
        "AuditAction 에 없는 라벨: {orphans:?}\n\
         필터 드롭다운에 고를 수는 있지만 결과가 늘 비는 항목이 된다"
    );
}
