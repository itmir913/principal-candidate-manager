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
