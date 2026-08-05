use principal_candidate_manager::auth::{
    decode_admin_token, decode_teacher_token, encode_admin_token, encode_teacher_token,
    encode_token, AdminClaims,
};

const SECRET: &str = "test_secret_for_unit_tests";

/// 현재 시각 기준 오프셋(초)으로 exp를 만든다. 음수면 과거.
fn exp_at(offset_secs: i64) -> usize {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    (now + offset_secs) as usize
}

#[test]
fn admin_token_roundtrip() {
    let token = encode_admin_token(SECRET).unwrap();
    let claims = decode_admin_token(&token, SECRET).unwrap();
    assert_eq!(claims.role, "admin");
}

#[test]
fn teacher_token_roundtrip() {
    let token = encode_teacher_token(2, 3, SECRET).unwrap();
    let claims = decode_teacher_token(&token, SECRET).unwrap();
    assert_eq!(claims.role, "teacher");
    assert_eq!(claims.grade, 2);
    assert_eq!(claims.class_no, 3);
}

#[test]
fn wrong_secret_fails() {
    let token = encode_admin_token(SECRET).unwrap();
    assert!(decode_admin_token(&token, "wrong_secret").is_err());
}

#[test]
fn admin_token_rejected_as_teacher() {
    let token = encode_admin_token(SECRET).unwrap();
    assert!(decode_teacher_token(&token, SECRET).is_err());
}

/// 권한 상승 차단. `AdminClaims` 는 `{role, exp}` 뿐이라 담임 토큰 JSON의
/// `grade`/`class_no` 를 serde 가 미지 필드로 무시하고 역직렬화에 성공한다 —
/// 즉 `decode_admin_token` 의 `role == "admin"` 검사가 유일한 방어선이고,
/// 이 방향(담임→관리자)이 그 검사를 실제로 요구하는 쪽이다.
/// 반대 방향은 `TeacherClaims` 에 `grade` 가 없어 역직렬화 실패로도 거부되므로
/// role 검사가 사라져도 통과한다.
#[test]
fn teacher_token_rejected_as_admin() {
    let token = encode_teacher_token(2, 3, SECRET).unwrap();
    assert!(
        decode_admin_token(&token, SECRET).is_err(),
        "담임 토큰이 관리자 토큰으로 통과하면 안 됨",
    );
}

/// 만료 검증. 서명이 유효해도 exp가 지났으면 거부해야 한다.
///
/// 이 자리에 테스트가 없었다. jsonwebtoken 상향(9 → 10)의 근거가 된 취약점이
/// 바로 만료 검증 우회(GHSA-h395-gr6q-cpjc)인데, 정작 만료 동작을 아무도
/// 확인하지 않고 있었다. 라이브러리 기본값(`Validation::default()`)에 기대는
/// 동작이라 버전을 올릴 때마다 조용히 바뀔 수 있는 자리다.
#[test]
fn expired_token_is_rejected() {
    let expired = AdminClaims {
        role: "admin".to_string(),
        exp: exp_at(-3600),
    };
    let token = encode_token(&expired, SECRET).unwrap();

    assert!(
        decode_admin_token(&token, SECRET).is_err(),
        "1시간 전에 만료된 토큰이 통과했다",
    );
}

/// 위 테스트가 "만료라서" 거부한 것인지 확인하는 짝. 같은 경로로 만든
/// 유효 기간 내 토큰은 통과해야 한다 — 통과하지 않으면 위 단언은
/// 만료가 아니라 다른 이유(서명·역직렬화)를 잡고 있는 것이다.
#[test]
fn unexpired_token_from_same_path_is_accepted() {
    let valid = AdminClaims {
        role: "admin".to_string(),
        exp: exp_at(3600),
    };
    let token = encode_token(&valid, SECRET).unwrap();

    let claims = decode_admin_token(&token, SECRET).expect("유효 기간 내 토큰이 거부됐다");
    assert_eq!(claims.role, "admin");
}
