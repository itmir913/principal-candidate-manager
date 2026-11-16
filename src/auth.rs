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

impl TeacherClaims {
    /// 졸업생 담당 특수 계정(grade 0·class_no 0). 관리자 비밀번호로 로그인한다.
    pub fn is_graduate_manager(&self) -> bool {
        self.grade == 0 && self.class_no == 0
    }
}

/// 이 담임 계정이 이 전형요소 값을 입력할 수 있는가.
///
/// 졸업생 담당은 사실상 관리자 역할이라 `teacher_editable` 이 아닌(관리자 입력 고정)
/// 항목도 입력한다(이슈 #50). 관리자가 넣은 값과 같은 `base_data` 행이라 저장하면
/// 덮어쓴다 — 소유자 결정. 저장 관문과 컨텍스트 응답의 `editable` 이 이 함수를 쓴다.
pub fn can_teacher_edit(claims: &TeacherClaims, teacher_editable: bool) -> bool {
    teacher_editable || claims.is_graduate_manager()
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
