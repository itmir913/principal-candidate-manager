//! 데이터 폴더·파일 이름 상수.
//!
//! 백업 zip은 압축을 푼 모습이 실제 데이터 폴더와 같아야 복원이 "폴더 통째 교체"
//! 한 가지 절차로 끝난다. 그래서 zip 내부 경로와 실제 경로가 **같은 상수**를
//! 봐야 한다 — 한쪽만 바뀌면 사용자가 압축을 풀어도 폴더 이름이 달라져 복원이
//! 조용히 어긋난다.

/// exe 옆에 만드는 데이터 폴더 이름. 백업 zip의 최상위 폴더 이름이기도 하다.
pub const DATA_DIR_NAME: &str = "pcm";

/// SQLite 데이터베이스 파일 이름.
pub const DB_FILENAME: &str = "data.db";

/// 포트 설정 파일 이름.
pub const CONFIG_FILENAME: &str = "config.json";
