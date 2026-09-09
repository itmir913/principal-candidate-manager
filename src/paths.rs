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

/// 백업 zip에 동봉하는 복원 안내문 이름. zip 최상위에 둔다 —
/// 데이터 폴더 안에 넣으면 복원한 폴더에까지 따라 들어가 남는다.
pub const README_FILENAME: &str = "복원방법.txt";

/// 자동 실행 레지스트리 값의 이름을 exe 경로에서 만든다 (이슈 #30).
///
/// 값 이름이 `"PCM"` 고정이던 동안, 인원제한 O/X 인스턴스를 두 개 돌리면 둘 다
/// `HKCU\...\Run\PCM` 한 자리에 써서 **나중에 켠 쪽이 앞의 등록을 덮어썼다** —
/// 재부팅하면 한 개만 살아난다. exe 경로로 이름을 갈라 인스턴스마다 제 자리를 준다.
///
/// 제목(`app_title`)이 아니라 exe 경로에서 뽑는 이유: 제목은 언제든 바뀌는데, 이름이
/// 따라 바뀌면 옛 이름의 등록이 유령으로 남고 그걸 지우는 책임이 새로 생긴다.
/// exe 위치는 데이터 폴더의 기준이기도 해서(`data_dir()`) 인스턴스 정체성에 더 가깝다.
///
/// 대소문자와 구분자를 정규화한다 — Windows 경로는 대소문자를 가리지 않고,
/// 같은 exe가 `C:\pcm.exe`와 `C:/pcm/a.exe`로 들어와도 같은 인스턴스다.
pub fn autostart_value_name(exe_path: &str) -> String {
    let normalized = exe_path.replace('/', "\\").to_lowercase();

    // FNV-1a 64비트. 레지스트리 값 이름을 가르는 용도라 암호학적 성질이 필요 없고,
    // 의존성 없이 결정적이면 된다.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in normalized.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    format!("PCM-{:016x}", hash)
}

/// 0.2.14 이전이 쓰던 고정 레지스트리 값 이름. 이관·정리 목적으로만 참조한다.
pub const LEGACY_AUTOSTART_VALUE_NAME: &str = "PCM";
