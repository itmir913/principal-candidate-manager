use anyhow::Result;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdminClaims {
    pub role: String,
    pub exp: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeacherClaims {
    pub role: String,
    pub grade: i64,
    pub class_no: i64,
    pub exp: usize,
}

fn expiry_secs(hours: u64) -> usize {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    (now + hours * 3600) as usize
}

pub fn encode_token<T: Serialize>(claims: &T, secret: &str) -> Result<String> {
    let token = encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn decode_admin_token(token: &str, secret: &str) -> Result<AdminClaims> {
    let data = decode::<AdminClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    anyhow::ensure!(data.claims.role == "admin", "not admin token");
    Ok(data.claims)
}

pub fn decode_teacher_token(token: &str, secret: &str) -> Result<TeacherClaims> {
    let data = decode::<TeacherClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    anyhow::ensure!(data.claims.role == "teacher", "not teacher token");
    Ok(data.claims)
}

pub fn encode_admin_token(secret: &str) -> Result<String> {
    encode_token(
        &AdminClaims { role: "admin".into(), exp: expiry_secs(12) },
        secret,
    )
}

pub fn encode_teacher_token(grade: i64, class_no: i64, secret: &str) -> Result<String> {
    encode_token(
        &TeacherClaims { role: "teacher".into(), grade, class_no, exp: expiry_secs(12) },
        secret,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
