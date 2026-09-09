//! 이슈 #30 — 자동 실행 레지스트리 값 이름의 인스턴스별 유일성.
//!
//! 값 이름이 `"PCM"` 고정이던 동안, 두 인스턴스가 같은 자리에 써서 나중에 켠 쪽이
//! 앞의 등록을 덮어썼다(재부팅하면 한 개만 자동 실행). 이 파일은 "다른 exe면 다른 이름,
//! 같은 exe면 같은 이름"을 고정한다.

use principal_candidate_manager::paths::{autostart_value_name, LEGACY_AUTOSTART_VALUE_NAME};

/// 핵심 회귀 — 인원제한 O/X 전용 인스턴스를 두 폴더에 두고 함께 돌리는 실제 운영 형태.
#[test]
fn different_exe_paths_get_different_names() {
    let a = autostart_value_name(r"C:\pcm-limited\principal-candidate-manager.exe");
    let b = autostart_value_name(r"C:\pcm-unlimited\principal-candidate-manager.exe");
    assert_ne!(a, b, "인스턴스가 다르면 레지스트리 자리도 달라야 한다");
}

#[test]
fn same_exe_path_is_stable_across_calls() {
    let p = r"C:\pcm\principal-candidate-manager.exe";
    assert_eq!(autostart_value_name(p), autostart_value_name(p));
}

/// 켤 때와 끌 때 이름이 어긋나면 해제가 남의 항목을 지우거나 자기 항목을 못 지운다.
/// Windows 경로는 대소문자를 가리지 않으므로 같은 exe로 취급해야 한다.
#[test]
fn case_differences_resolve_to_same_name() {
    let lower = autostart_value_name(r"c:\pcm\principal-candidate-manager.exe");
    let upper = autostart_value_name(r"C:\PCM\Principal-Candidate-Manager.exe");
    assert_eq!(lower, upper);
}

/// `current_exe()`가 어떤 구분자로 주든 같은 인스턴스로 봐야 한다.
#[test]
fn separator_differences_resolve_to_same_name() {
    let back = autostart_value_name(r"C:\pcm\app.exe");
    let fwd = autostart_value_name("C:/pcm/app.exe");
    assert_eq!(back, fwd);
}

/// 레거시 항목 정리는 "이 exe를 가리킬 때만" 지운다 — 그 판단이 이 함수의 결과로 이뤄진다.
/// 같은 exe를 다른 표기로 만난 경우를 놓치면 유령 항목이 남아 껐는데도 부팅 시 실행된다.
#[test]
fn legacy_name_is_distinct_from_generated_names() {
    let generated = autostart_value_name(r"C:\pcm\app.exe");
    assert_ne!(generated, LEGACY_AUTOSTART_VALUE_NAME);
    assert!(generated.starts_with("PCM-"), "실제 이름: {generated}");
}

/// 레지스트리 값 이름으로 쓸 수 있어야 한다 — 경로 문자가 그대로 새어 나가면 안 된다.
#[test]
fn name_is_safe_for_registry_value() {
    let name = autostart_value_name(r"C:\pcm 한글 폴더\app (1).exe");
    assert!(
        name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "이름에 경로 문자가 섞였다: {name}"
    );
    assert!(name.len() < 64, "레지스트리 값 이름 길이: {}", name.len());
}

/// 한 글자 차이도 갈라야 한다 — 폴더 이름만 다른 두 설치가 흔하다.
#[test]
fn one_character_path_difference_changes_name() {
    let a = autostart_value_name(r"C:\pcm1\app.exe");
    let b = autostart_value_name(r"C:\pcm2\app.exe");
    assert_ne!(a, b);
}
