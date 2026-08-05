/// 릴리즈된 스키마 버전 동결 테스트
///
/// 출시 이후에는 이미 배포된 버전의 스키마를 손대면 안 된다. 현장 DB는 그 스키마로
/// 만들어져 있고, `PRAGMA user_version`으로 자기 버전을 알린다. 조각 파일을 조용히
/// 고치면 새로 만든 DB와 기존 DB의 구조가 갈라지는데 두 DB 모두 user_version은
/// 같으므로, 앱은 그 차이를 영영 감지하지 못한다.
///
/// 그래서 버전별 스키마 지문을 `tests/schema_snapshots/v{N}.sql`에 고정해 둔다.
///   - `migrations/v{N}/` 을 고치면 지문이 어긋나 이 테스트가 깨진다.
///   - 고치는 유일한 정공법은 새 버전(`migrations/v2/`, `V2_FRAGMENTS`)을 추가하고
///     `SCHEMA_VERSION`을 올리는 것이다. 그러면 v{N} 지문은 그대로 통과하고,
///     새 버전 스냅샷만 새로 생성된다.
///
/// [테스트]
///   1. 버전 수와 SCHEMA_VERSION 일치 (스냅샷 대상 범위 자체를 고정)
///   2. 릴리즈된 각 버전의 스키마 지문 동결 — 어긋나면 실패
///   3. SCHEMA_VERSION보다 높은 스냅샷 존재 금지 (버전 되돌리기·마이그레이션 삭제 감지)
///   4. 최종 버전 지문 == init_pool이 실제로 만드는 새 DB의 지문
use std::path::PathBuf;

use principal_candidate_manager::db::{self, run_migrations_with, SCHEMA_VERSION};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

/// 스냅샷이 **없는** 버전에 한해 파일을 생성해 주는 환경변수.
/// 이미 있는 스냅샷은 이 변수로도 덮어쓰지 않는다 — 동결의 의미가 사라지기 때문이다.
const WRITE_ENV: &str = "PCM_WRITE_SCHEMA_SNAPSHOT";

/// 헤더(생성 당시 앱 버전 등)와 본문(지문)의 경계. 본문만 비교한다.
const BODY_MARKER: &str = "--- SCHEMA FINGERPRINT ---";

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/schema_snapshots")
}

fn snapshot_path(version: u32) -> PathBuf {
    snapshot_dir().join(format!("v{version}.sql"))
}

// ────────────────────────────────────────────────────────────────────────────
// 지문 생성
// ────────────────────────────────────────────────────────────────────────────

/// SQL 텍스트에서 의미 없는 차이(주석·들여쓰기·줄바꿈)를 제거한다.
/// 열·제약·트리거 본문 같은 **구조**가 바뀌면 결과가 달라진다.
fn normalize_sql(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            // '' 이스케이프는 닫고 다시 여는 것으로 처리돼도 균형이 맞는다
            if c == '\'' {
                in_string = false;
            }
            continue;
        }
        match c {
            '\'' => {
                in_string = true;
                out.push(c);
            }
            // 줄 주석은 문자열 밖에서만 주석이다
            '-' if chars.peek() == Some(&'-') => {
                for c2 in chars.by_ref() {
                    if c2 == '\n' {
                        break;
                    }
                }
                out.push('\n');
            }
            _ => out.push(c),
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 연결된 DB의 스키마 지문. sqlite_master 전체(테이블·인덱스·트리거·뷰)와
/// user_version을 결정적인 순서로 직렬화한다.
async fn fingerprint(pool: &SqlitePool) -> String {
    let user_version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await
        .expect("user_version 조회 실패");

    let rows = sqlx::query(
        "SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name, tbl_name",
    )
    .fetch_all(pool)
    .await
    .expect("sqlite_master 조회 실패");

    let mut out = format!("PRAGMA user_version = {user_version};\n");
    for row in rows {
        let typ: String = row.get(0);
        let name: String = row.get(1);
        let tbl: String = row.get(2);
        let sql: Option<String> = row.get(3);
        out.push('\n');
        out.push_str(&format!("[{typ}] {name} (on {tbl})\n"));
        match sql {
            // UNIQUE/PK 제약이 자동 생성한 인덱스는 sql이 NULL이다.
            // 이름 자체가 제약의 존재를 증언하므로 항목으로는 남긴다.
            None => out.push_str("(암시적 — 제약이 생성한 인덱스)\n"),
            Some(s) => {
                out.push_str(&normalize_sql(&s));
                out.push_str(";\n");
            }
        }
    }
    out
}

/// in-memory DB에 v1..=version 마이그레이션만 적용한 뒤 지문을 뜬다.
async fn fingerprint_at_version(version: u32) -> String {
    let sqls = db::migration_sqls();
    assert!(
        version as usize <= sqls.len(),
        "v{version} 마이그레이션이 없다 (등록된 버전 수: {})",
        sqls.len()
    );
    let refs: Vec<&str> = sqls[..version as usize]
        .iter()
        .map(String::as_str)
        .collect();

    let pool = memory_pool().await;
    run_migrations_with(&pool, &refs)
        .await
        .unwrap_or_else(|e| panic!("v{version}까지 마이그레이션 실패: {e}"));

    fingerprint(&pool).await
}

async fn memory_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("in-memory 풀 생성 실패")
}

// ────────────────────────────────────────────────────────────────────────────
// 스냅샷 파일 입출력
// ────────────────────────────────────────────────────────────────────────────

/// 스냅샷 파일에서 본문(지문)만 떼어 낸다.
fn read_snapshot_body(version: u32) -> Option<String> {
    let text = std::fs::read_to_string(snapshot_path(version)).ok()?;
    let (_, body) = text.split_once(BODY_MARKER).unwrap_or_else(|| {
        panic!(
            "v{version} 스냅샷에 '{BODY_MARKER}' 경계선이 없다 — 파일이 손상됐다.\n\
             파일: {}",
            snapshot_path(version).display()
        )
    });
    // 경계선 다음 줄부터가 본문
    Some(body.trim_start_matches(['\r', '\n']).replace("\r\n", "\n"))
}

fn write_snapshot(version: u32, body: &str) {
    let path = snapshot_path(version);
    assert!(
        !path.exists(),
        "v{version} 스냅샷이 이미 있다 — 덮어쓰지 않는다. 동결된 스키마다."
    );
    std::fs::create_dir_all(snapshot_dir()).expect("스냅샷 디렉터리 생성 실패");

    let header = format!(
        "-- PCM 스키마 지문 v{version} (생성 당시 앱 버전 {app})\n\
         --\n\
         -- 자동 생성 파일. 손으로 고치지 말 것 — tests/schema_freeze.rs 가 실제 스키마와 대조한다.\n\
         -- 이 파일이 존재하는 순간 v{version} 스키마는 동결이다.\n\
         -- migrations/v{version}/*.sql 을 고치면 이 테스트가 깨진다. 그때는 파일을 맞추지 말고\n\
         -- 새 버전(migrations/v{next}/ + V{next}_FRAGMENTS)을 추가하고 SCHEMA_VERSION을 올려라.\n\
         {BODY_MARKER}\n",
        app = env!("CARGO_PKG_VERSION"),
        next = version + 1,
    );
    std::fs::write(&path, format!("{header}{body}")).expect("스냅샷 기록 실패");
    eprintln!("v{version} 스냅샷 생성: {}", path.display());
}

/// 지문 두 개의 첫 불일치 지점을 사람이 읽을 형태로 만든다.
fn diff_report(expected: &str, actual: &str) -> String {
    let (exp_lines, act_lines): (Vec<_>, Vec<_>) =
        (expected.lines().collect(), actual.lines().collect());
    let mut report = String::new();
    for i in 0..exp_lines.len().max(act_lines.len()) {
        let e = exp_lines.get(i).copied().unwrap_or("(없음)");
        let a = act_lines.get(i).copied().unwrap_or("(없음)");
        if e != a {
            report.push_str(&format!(
                "첫 불일치 {}번째 줄\n  스냅샷: {}\n  현재  : {}\n",
                i + 1,
                e,
                a
            ));
            break;
        }
    }
    report.push_str(&format!(
        "(스냅샷 {}줄 / 현재 {}줄)",
        exp_lines.len(),
        act_lines.len()
    ));
    report
}

// ────────────────────────────────────────────────────────────────────────────
// 1. 스냅샷 대상 범위 고정 — 버전 수와 SCHEMA_VERSION은 같아야 한다
// ────────────────────────────────────────────────────────────────────────────
#[test]
fn migration_count_equals_schema_version() {
    assert_eq!(
        db::migration_sqls().len(),
        SCHEMA_VERSION as usize,
        "SCHEMA_VERSION과 등록된 마이그레이션 수가 어긋났다 — \
         새 버전을 추가했다면 둘 다 올려야 한다"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// 2. 릴리즈된 각 버전의 스키마는 동결이다
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn released_schema_versions_are_frozen() {
    for version in 1..=SCHEMA_VERSION {
        let actual = fingerprint_at_version(version).await;

        let Some(expected) = read_snapshot_body(version) else {
            if std::env::var(WRITE_ENV).is_ok() {
                write_snapshot(version, &actual);
                continue;
            }
            panic!(
                "v{version} 스키마 스냅샷이 없다: {}\n\n\
                 새 스키마 버전을 방금 추가했다면 아래 명령으로 지문을 만들고 커밋해라.\n  \
                 $env:{WRITE_ENV}=1; cargo test --test schema_freeze\n\n\
                 스냅샷을 지운 것이라면 되돌려라 — 릴리즈된 스키마의 지문은 사라지면 안 된다.",
                snapshot_path(version).display()
            );
        };

        // assert_eq!가 아니라 panic! — 지문 전문(수백 줄)이 두 번 찍히면
        // 정작 읽어야 할 안내가 스크롤 밖으로 밀려난다.
        assert!(
            expected.trim_end() == actual.trim_end(),
            "\n\n=== v{version} 스키마가 동결 지문과 다르다 ===\n{}\n\n\
             이 버전은 이미 배포됐다. 현장 DB는 옛 구조 그대로이고 user_version은 똑같이 \
             {version}이라, 조각 파일만 고치면 앱은 두 DB의 차이를 영영 감지하지 못한다.\n\n\
             할 일:\n  \
             1. migrations/v{version}/ 의 변경을 되돌린다\n  \
             2. 변경분을 migrations/v{next}/ 새 조각으로 옮기고 V{next}_FRAGMENTS를 만든다\n  \
             3. src/db.rs 의 SCHEMA_VERSION을 {next}(으)로, MIGRATION_FRAGMENTS에 V{next}_FRAGMENTS를 추가\n  \
             4. $env:{WRITE_ENV}=1; cargo test --test schema_freeze 로 v{next} 지문 생성 후 커밋\n\n\
             스냅샷 파일을 현재 스키마에 맞춰 고치는 것은 해결이 아니다.\n",
            diff_report(&expected, &actual),
            next = version + 1,
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 3. SCHEMA_VERSION보다 높은 스냅샷이 남아 있으면 안 된다
//    (버전을 되돌렸거나 마이그레이션을 삭제한 흔적)
// ────────────────────────────────────────────────────────────────────────────
#[test]
fn no_snapshot_beyond_schema_version() {
    let dir = snapshot_dir();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| {
        panic!(
            "스냅샷 디렉터리가 없거나 읽히지 않는다 ({}): {e}\n\
             릴리즈된 스키마의 지문은 저장소에 있어야 한다 — 지웠다면 되돌려라.",
            dir.display()
        )
    });

    let mut found: Vec<u32> = Vec::new();
    for entry in entries {
        let name = entry.expect("디렉터리 항목 읽기 실패").file_name();
        let name = name.to_string_lossy().to_string();
        let Some(num) = name
            .strip_prefix('v')
            .and_then(|s| s.strip_suffix(".sql"))
            .and_then(|s| s.parse::<u32>().ok())
        else {
            panic!("스냅샷 디렉터리에 규칙 밖 파일이 있다: {name} (v{{N}}.sql 만 허용)");
        };
        assert!(
            num <= SCHEMA_VERSION,
            "v{num} 스냅샷이 있는데 SCHEMA_VERSION은 {SCHEMA_VERSION}이다 — \
             버전을 되돌렸거나 마이그레이션을 지웠다. 배포된 버전은 되돌릴 수 없다"
        );
        found.push(num);
    }

    // 생성 모드에서는 완전성을 따지지 않는다 — 같은 실행에서 다른 테스트가 아직
    // 파일을 만들기 전일 수 있다. 생성 직후 한 번 더 돌리면 이 단언이 검증한다.
    if std::env::var(WRITE_ENV).is_ok() {
        eprintln!("{WRITE_ENV} 설정됨 — 스냅샷 완전성 검사는 건너뛴다. 다시 한 번 실행해 확인해라.");
        return;
    }

    found.sort_unstable();
    assert_eq!(
        found,
        (1..=SCHEMA_VERSION).collect::<Vec<_>>(),
        "스냅샷이 1..={SCHEMA_VERSION} 전 버전에 대해 있어야 한다"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// 4. init_pool이 실제로 만드는 새 DB의 지문 == 최종 버전 스냅샷
//    (마이그레이션 배열은 그대로인데 init_pool 경로만 달라지는 경우를 잡는다)
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn fresh_db_matches_latest_snapshot() {
    let path = std::env::temp_dir().join(format!("pcm_schema_freeze_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let pool = db::init_pool(path.to_str().unwrap())
        .await
        .expect("새 DB 초기화 실패");
    let actual = fingerprint(&pool).await;
    pool.close().await;
    let _ = std::fs::remove_file(&path);

    let expected = read_snapshot_body(SCHEMA_VERSION).unwrap_or_else(|| {
        panic!(
            "v{SCHEMA_VERSION} 스냅샷이 없다 — released_schema_versions_are_frozen 안내를 따라 생성해라"
        )
    });

    assert!(
        expected.trim_end() == actual.trim_end(),
        "\n\n=== init_pool이 만든 새 DB가 v{SCHEMA_VERSION} 동결 지문과 다르다 ===\n{}\n",
        diff_report(&expected, &actual),
    );
}
