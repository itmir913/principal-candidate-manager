//! Fail-Fast(CLAUDE.md 규칙 2) — `unwrap_or*` 가 허용 목록 밖에서 늘지 않는지 대조한다.
//!
//! `src/docs/silent_fallback_allowed.md` 는 "새 예외는 이 파일에 **먼저** 기록한다"고
//! 적어 두었지만, 그 규칙을 지키는 기계가 없었다. 실제로 `src/handlers/app_info.rs` 의
//! 예외 2건이 목록 없이 코드에 들어와 있었고 **아무것도 알려 주지 않았다**(2026-09-21).
//!
//! 이 저장소는 프론트에서 같은 교훈을 여섯 번 반복해 배웠다 — 규칙을 문서로만 두면
//! 조용히 어긋난다. 여기서는 **파일 단위**로 대조한다:
//!   ① `src/` 안에서 `unwrap_or*` 를 쓰는 파일은 전부 목록에 이름이 있어야 한다.
//!   ② 파일별 **개수**도 고정한다 — 이미 허용된 파일에 하나가 더 붙는 것을 잡는다.
//!
//! **한계(정직하게)**: 줄 단위가 아니라 파일 단위다. 허용된 파일 안에서 기존 예외를
//! 지우고 다른 곳에 같은 수만큼 새로 넣으면 통과한다. 목록이 파일 단위로만 위치를
//! 적고 있어 그 이상은 이 문서 형식으로 지탱되지 않는다.
//!
//! ③ `unwrap_or*` 밖의 형태 — `Result` 를 `.ok()` 로 Option 으로 바꿔 오류를 버리거나
//! `let _ =` 로 결과를 버리는 것 — 도 파일별 개수를 고정한다(아래
//! `ok_and_discard_counts_are_pinned`). 허용 목록에 이 형태들(#6·#7·#18·#38~41)이
//! 있는데 앞의 두 검사는 `unwrap_or*` 만 세서 보지 못했다(2026-10-06 감사 지적).
//! 이 둘은 정당한 쓰임(`Option` 을 바로 `match` 하는 파싱 등)도 많아 "목록에 이름이
//! 있는가"는 보지 않고 개수만 고정한다 — 늘면 사람이 보게 하는 것이 목적이다.
//!
//! 그리고 이것은 **소스 텍스트 검사**다 — 주석 안의 `unwrap_or(` 도 세고, 매크로로
//! 감추면 못 본다. 규칙 2 의 진짜 방어선은 점수 계산 경로의 단위·오라클 테스트다.

use std::collections::BTreeMap;

/// 허용 목록이 이름을 대는 파일들. `### N. \`경로\` — 설명` 에서 경로만 뽑고,
/// `::함수` 꼬리표는 떼어 파일 단위로 만든다.
fn allowed_files() -> Vec<String> {
    let doc = include_str!("../src/docs/silent_fallback_allowed.md");
    let mut out = Vec::new();
    for line in doc.lines() {
        let Some(rest) = line.strip_prefix("### ") else { continue };
        let Some(start) = rest.find('`') else { continue };
        let Some(len) = rest[start + 1..].find('`') else { continue };
        let path = &rest[start + 1..start + 1 + len];
        let file = path.split("::").next().unwrap_or(path);
        out.push(file.replace('\\', "/"));
    }
    out
}

/// `src/` 를 훑어 파일별 `unwrap_or*` 개수를 센다.
fn fallbacks_in_src() -> BTreeMap<String, usize> {
    count_in_src(count_unwrap_or)
}

/// `src/` 를 훑어 파일별로 `count` 가 돌려준 개수를 모은다(0 인 파일은 뺀다).
fn count_in_src(count: fn(&str) -> usize) -> BTreeMap<String, usize> {
    // 경로는 **저장소 루트를 떼어** 만든다. 앞서 `split_once("src/")` 를 썼는데,
    // 저장소가 경로에 `src/` 를 이미 포함한 곳에 있으면(예: `~/src/pcm/`) 첫 번째
    // `src/` 에서 잘려 키가 어긋난다. 이식성 문제이자 silent fallback 이었다.
    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        count: fn(&str) -> usize,
        out: &mut BTreeMap<String, usize>,
    ) {
        for e in std::fs::read_dir(dir).expect("src 를 읽지 못했다") {
            let p = e.expect("디렉터리 항목").path();
            if p.is_dir() {
                walk(root, &p, count, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let src = std::fs::read_to_string(&p).expect("소스를 읽지 못했다");
                let n = count(&src);
                if n > 0 {
                    let rel = p
                        .strip_prefix(root)
                        .expect("탐색 결과가 저장소 루트 밖이다")
                        .to_string_lossy()
                        .replace(char::from(92), "/");
                    out.insert(rel, n);
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = BTreeMap::new();
    walk(root, root.join("src").as_path(), count, &mut out);
    out
}

/// `unwrap_or`, `unwrap_or_else`, `unwrap_or_default` 의 호출 횟수.
fn count_unwrap_or(src: &str) -> usize {
    let mut n = 0;
    let bytes = src.as_bytes();
    let needle = b"unwrap_or";
    let mut i = 0;
    while let Some(pos) = src[i..].find("unwrap_or") {
        let at = i + pos;
        // 뒤에 이어지는 식별자 문자(`_else`, `_default`)를 건너뛰고 `(` 인지 본다.
        let mut j = at + needle.len();
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
            j += 1;
        }
        if bytes.get(j) == Some(&b'(') {
            n += 1;
        }
        i = at + needle.len();
    }
    n
}

#[test]
fn every_silent_fallback_is_on_the_allowed_list() {
    let allowed = allowed_files();
    let found = fallbacks_in_src();

    assert!(!found.is_empty(), "탐색이 깨졌다 — src 에서 unwrap_or 를 하나도 못 찾았다");

    let missing: Vec<_> = found
        .keys()
        .filter(|f| !allowed.iter().any(|a| a == *f))
        .collect();

    assert!(
        missing.is_empty(),
        "허용 목록에 없는 silent fallback: {missing:?}\n\
         `src/docs/silent_fallback_allowed.md` 에 위치·이유·조건을 **먼저** 적어라.\n\
         (CLAUDE.md 규칙 2 — 적을 수 없으면 그 fallback 을 쓰면 안 된다는 뜻이다.)"
    );
}

/// 이미 허용된 파일에 **하나가 더** 붙는 것을 잡는다.
/// 개수가 바뀌면 목록도 함께 고쳐야 한다 — 그 강제가 이 테스트의 값이다.
#[test]
fn fallback_counts_are_pinned() {
    let found = fallbacks_in_src();

    // 2026-09-21 실측. 늘리려면 silent_fallback_allowed.md 에 항목을 먼저 추가하라.
    let expected: BTreeMap<&str, usize> = [
        ("src/excel.rs", 1),
        ("src/handlers/app_info.rs", 2),
        ("src/handlers/area_data.rs", 1),
        ("src/handlers/auth.rs", 3),
        ("src/handlers/classes.rs", 1),
        ("src/handlers/external_import.rs", 7),
        ("src/handlers/scoring.rs", 3),
        ("src/handlers/teacher_areas.rs", 1),
        ("src/handlers/teacher_export.rs", 1),
        ("src/handlers/universities.rs", 6),
        ("src/main.rs", 4),
    ]
    .into_iter()
    .collect();

    let actual: BTreeMap<&str, usize> =
        found.iter().map(|(k, v)| (k.as_str(), *v)).collect();

    assert_eq!(
        actual, expected,
        "\nsilent fallback 개수가 달라졌다.\n\
         늘었다면 `src/docs/silent_fallback_allowed.md` 에 항목을 먼저 적고 이 수를 고쳐라.\n\
         줄었다면(좋은 일이다) 목록에서 해당 항목을 지우고 이 수를 고쳐라."
    );
}

fn count_ok_call(src: &str) -> usize {
    src.matches(".ok()").count()
}

fn count_let_discard(src: &str) -> usize {
    src.matches("let _ =").count()
}

/// `.ok()` 와 `let _ =` 의 파일별 개수. 위 `fallback_counts_are_pinned` 와 같은 방식이다.
/// 주석 안의 것도 센다(`src/excel.rs` 의 `.ok()` 1 건은 주석이다).
/// `src/main.rs` 의 `let _ = &desc_item;` 은 결과를 버리는 것이 아니라 미사용 경고를
/// 끄는 빌림이다 — 세기는 하지만 fallback 은 아니다.
#[test]
fn ok_and_discard_counts_are_pinned() {
    // 2026-10-06 실측. 늘리려면 silent_fallback_allowed.md 에 항목을 먼저 추가하라.
    let expected_ok: BTreeMap<&str, usize> = [
        ("src/excel.rs", 1),
        ("src/handlers/classes.rs", 2),
        ("src/handlers/students.rs", 4),
        ("src/main.rs", 1),
        ("src/middleware.rs", 1),
    ]
    .into_iter()
    .collect();
    let expected_discard: BTreeMap<&str, usize> = [("src/main.rs", 4)].into_iter().collect();

    let ok = count_in_src(count_ok_call);
    let discard = count_in_src(count_let_discard);
    let ok: BTreeMap<&str, usize> = ok.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let discard: BTreeMap<&str, usize> = discard.iter().map(|(k, v)| (k.as_str(), *v)).collect();

    assert_eq!(
        ok, expected_ok,
        "
`.ok()` 개수가 달라졌다. Result 의 오류를 버리는 것이라면          `src/docs/silent_fallback_allowed.md` 에 먼저 적고 이 수를 고쳐라."
    );
    assert_eq!(
        discard, expected_discard,
        "
`let _ =` 개수가 달라졌다. 결과를 버리는 것이라면          `src/docs/silent_fallback_allowed.md` 에 먼저 적고 이 수를 고쳐라."
    );
}
