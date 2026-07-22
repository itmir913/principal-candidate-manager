use principal_candidate_manager::auth::{
    decode_admin_token, decode_teacher_token, encode_admin_token, encode_teacher_token,
};

const SECRET: &str = "test_secret_for_unit_tests";

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
