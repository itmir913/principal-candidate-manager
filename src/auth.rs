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
