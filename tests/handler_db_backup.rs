//! DB 백업 다운로드(`GET /api/auth/db-backup`) 무결성 테스트.
//!
//! 이 시스템의 DB는 WAL 저널 모드(`src/db.rs`)이고, 프로그램은 종료 시
//! `std::process::exit`로 나가므로 `data.db-wal`이 남을 수 있다. 그래서
//! **파일 복사 방식 백업은 `data.db`만 가져가면 최근 커밋을 놓친다.**
//! 인앱 백업은 `VACUUM INTO`로 연결을 통해 읽어 이 문제를 피하는데,
//! 아래 테스트가 그 전제를 실제로 검증한다.

use axum::{
    body::to_bytes,
    extract::{ConnectInfo, State},
    Extension,
};
use principal_candidate_manager::{
    auth::AdminClaims, handlers::system::download_db_backup, state::AppState,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

fn test_client() -> ConnectInfo<SocketAddr> {
    ConnectInfo("127.0.0.1:12345".parse().unwrap())
}

fn admin_claims() -> AdminClaims {
    AdminClaims { role: "admin".into(), exp: 9_999_999_999 }
}

/// 테스트마다 고유한 임시 폴더. 실제 배포처럼 `data.db`를 파일로 둔다
/// (`:memory:`로는 WAL도 VACUUM INTO도 검증할 수 없다).
fn temp_dir() -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pcm_backup_test_{}_{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 배포 설정과 같은 WAL 모드 파일 DB 풀을 만든다.
async fn wal_pool(db_path: &Path) -> SqlitePool {
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await.unwrap();
    sqlx::raw_sql(&principal_candidate_manager::db::full_schema_sql())
        .execute(&pool)
        .await
        .unwrap();
    pool
}

fn state_for(pool: SqlitePool, db_path: &Path) -> AppState {
    AppState {
        db: pool,
        jwt_secret: "test".into(),
        db_path: db_path.to_path_buf(),
        server_addr: String::new(),
    }
}

/// zip 산출물에서 특정 엔트리를 꺼낸다. 없으면 None.
fn zip_entry(zip_bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
    let mut file = archive.by_name(name).ok()?;
    let mut out = Vec::new();
    file.read_to_end(&mut out).unwrap();
    Some(out)
}

fn zip_entry_names(zip_bytes: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
    (0..archive.len()).map(|i| archive.by_index(i).unwrap().name().to_owned()).collect()
}

async fn backup_bytes(state: &AppState) -> Vec<u8> {
    let resp = download_db_backup(State(state.clone()), Extension(admin_claims()), test_client())
        .await
        .expect("백업 응답이 Ok여야 한다");
    to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec()
}

/// 백업 zip에서 `pcm/data.db`를 꺼내 독립 파일로 열어 값을 읽는다.
/// `-wal` 없이 단독으로 완결되어 있어야 성공한다.
async fn read_key_from_backup(dir: &Path, zip_bytes: &[u8], key: &str) -> Option<String> {
    let db = zip_entry(zip_bytes, "pcm/data.db").expect("zip에 pcm/data.db가 없다");
    let restored = dir.join("restored.db");
    std::fs::write(&restored, &db).unwrap();

    let opts = SqliteConnectOptions::new().filename(&restored).foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await.unwrap();
    let value: Option<(String,)> = sqlx::query_as("SELECT value FROM app_configs WHERE key = ?")
        .bind(key)
        .fetch_optional(&pool)
        .await
        .unwrap();
    pool.close().await;
    value.map(|(v,)| v)
}

fn temp_files_in(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("backup_tmp_"))
        .collect()
}

/// 핵심 계약: 체크포인트 전(=WAL에만 있는) 커밋도 백업에 포함되어야 한다.
/// 이것이 성립하지 않으면 인앱 백업은 파일 복사와 다를 바 없어진다.
#[tokio::test]
async fn backup_includes_wal_resident_commits() {
    let dir = temp_dir();
    let db_path = dir.join("data.db");
    let pool = wal_pool(&db_path).await;

    // 체크포인트를 부르지 않는다 — 이 커밋은 data.db-wal에 남아 있다.
    sqlx::query("INSERT INTO app_configs (key, value) VALUES ('backup_probe', 'wal_resident')")
        .execute(&pool)
        .await
        .unwrap();

    let state = state_for(pool.clone(), &db_path);
    let bytes = backup_bytes(&state).await;

    assert_eq!(
        read_key_from_backup(&dir, &bytes, "backup_probe").await.as_deref(),
        Some("wal_resident"),
        "WAL에만 있는 커밋이 백업에서 누락됐다"
    );

    pool.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// 백업 후 임시 파일이 남으면 전교생 PII가 담긴 파일이 pcm 폴더에 방치된다.
/// 소유자 안내가 "pcm 폴더를 통째로 압축"이므로 그 파일까지 함께 배포된다.
#[tokio::test]
async fn backup_leaves_no_temp_file() {
    let dir = temp_dir();
    let db_path = dir.join("data.db");
    let pool = wal_pool(&db_path).await;
    let state = state_for(pool.clone(), &db_path);

    let bytes = backup_bytes(&state).await;
    assert!(!bytes.is_empty(), "백업 산출물이 비어 있다");

    assert!(
        temp_files_in(&dir).is_empty(),
        "임시 파일이 남았다: {:?}",
        temp_files_in(&dir)
    );

    pool.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// 임시 파일명이 초 단위 타임스탬프뿐이면 같은 초의 두 번째 요청이
/// "출력 파일이 이미 존재한다"로 실패한다. 관리자가 버튼을 두 번 누르는 것은
/// 흔한 조작이므로 반드시 둘 다 성공해야 한다.
#[tokio::test]
async fn two_backups_within_same_second_both_succeed() {
    let dir = temp_dir();
    let db_path = dir.join("data.db");
    let pool = wal_pool(&db_path).await;

    sqlx::query("INSERT INTO app_configs (key, value) VALUES ('backup_probe', 'twice')")
        .execute(&pool)
        .await
        .unwrap();

    let state = state_for(pool.clone(), &db_path);

    let first = backup_bytes(&state).await;
    let second = backup_bytes(&state).await;

    for (label, bytes) in [("첫 번째", &first), ("두 번째", &second)] {
        assert_eq!(
            read_key_from_backup(&dir, bytes, "backup_probe").await.as_deref(),
            Some("twice"),
            "{label} 백업이 온전하지 않다"
        );
    }
    assert!(temp_files_in(&dir).is_empty(), "임시 파일이 남았다");

    pool.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// 복원 절차를 "폴더 통째 교체" 하나로 유지하려면 압축을 푼 모습이 실제
/// 데이터 폴더와 같아야 한다. 엔트리 경로가 `pcm/` 아래가 아니면 사용자가
/// 파일을 골라 옮겨야 하고, 그 순간 매뉴얼의 복원 절차가 무너진다.
#[tokio::test]
async fn backup_zip_mirrors_pcm_folder_layout() {
    let dir = temp_dir();
    let db_path = dir.join("data.db");
    let pool = wal_pool(&db_path).await;
    std::fs::write(dir.join("config.json"), br#"{"port":9090}"#).unwrap();

    let state = state_for(pool.clone(), &db_path);
    let bytes = backup_bytes(&state).await;

    let mut names = zip_entry_names(&bytes);
    names.sort();
    assert_eq!(
        names,
        vec!["pcm/config.json".to_string(), "pcm/data.db".to_string()],
        "zip 구조가 pcm 폴더 모양이 아니다"
    );

    assert_eq!(
        zip_entry(&bytes, "pcm/config.json").as_deref(),
        Some(&br#"{"port":9090}"#[..]),
        "config.json 내용이 원본과 다르다"
    );

    // 진단용 로그는 복원 대상이 아니므로 넣지 않는다.
    assert!(
        !names.iter().any(|n| n.contains("logs")),
        "logs가 백업에 포함됐다: {:?}",
        names
    );

    pool.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// `config.json`은 포트 설정뿐이라 없어도 데이터 복원은 가능하다.
/// 파일이 없다는 이유로 백업 전체가 실패하면 안 된다.
#[tokio::test]
async fn backup_succeeds_without_config_json() {
    let dir = temp_dir();
    let db_path = dir.join("data.db");
    let pool = wal_pool(&db_path).await;

    sqlx::query("INSERT INTO app_configs (key, value) VALUES ('backup_probe', 'no_config')")
        .execute(&pool)
        .await
        .unwrap();

    let state = state_for(pool.clone(), &db_path);
    let bytes = backup_bytes(&state).await;

    assert_eq!(
        zip_entry_names(&bytes),
        vec!["pcm/data.db".to_string()],
        "config.json이 없을 때는 data.db만 들어가야 한다"
    );
    assert_eq!(
        read_key_from_backup(&dir, &bytes, "backup_probe").await.as_deref(),
        Some("no_config"),
        "config.json 부재가 DB 백업을 망가뜨렸다"
    );

    pool.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// 백업은 감사 로그(`DbBackupDownloaded`)에 클라이언트 IP와 함께 남아야 한다
/// (2차 감사 소유자 라운드 #5). 임시 파일 정리 변경이 이 경로를 깨지 않았는지 확인.
#[tokio::test]
async fn backup_is_audit_logged_with_ip() {
    let dir = temp_dir();
    let db_path = dir.join("data.db");
    let pool = wal_pool(&db_path).await;
    let state = state_for(pool.clone(), &db_path);

    backup_bytes(&state).await;

    let (action, ip): (String, Option<String>) =
        sqlx::query_as("SELECT action, actor_ip FROM audit_log ORDER BY id DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(action, "DB_BACKUP_DOWNLOADED");
    assert_eq!(ip.as_deref(), Some("127.0.0.1"));

    pool.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}
