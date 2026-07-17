use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CalcType {
    Numeric,
    Category,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LookupScope {
    Simple,
    Composite,
}

#[derive(Debug, Clone, Copy, PartialEq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchMode {
    Upper,
    Lower,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CategoryAgg {
    Sum,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoundStatus {
    Open,
    Closed,
    Finalized,
}

#[derive(Debug, Clone, Copy, PartialEq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditAction {
    // 라운드 생명주기
    RoundOpened,
    RoundClosed,
    RoundReopened,
    RoundFinalized,
    // 점수·추천
    ScoresRecalculated,
    RecommendConfirmed,
    RecommendCanceled,
    AutoRecommendRun,
    // 지원
    ApplicationSaved,
    ApplicationDeleted,
    ApplicationAbandoned,
    // 학급
    ClassesImported,
    ClassSaved,
    ClassDeleted,
    // 학생
    StudentsImported,
    StudentAdded,
    StudentDeleted,
    // 전형요소
    AreaCreated,
    AreaUpdated,
    AreaDeleted,
    // 점수 기준·기초데이터
    ScoreTableImported,
    BaseDataImported,
    // 대학·모집단위
    UniversityCreated,
    UniversityUpdated,
    UniversityDeleted,
    TrackCreated,
    TrackUpdated,
    TrackDeleted,
}
