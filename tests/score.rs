/// Score newtype 직렬화·역직렬화 검증.
/// 역직렬화는 요청 바디(전형요소 max_score 등)에 쓰이므로,
/// `as i64` 캐스트 포화(1e300→i64::MAX)를 이용한 우회 입력을 거부해야 한다.
use principal_candidate_manager::score::Score;

#[test]
fn score_roundtrip_normal_values() {
    for (json, raw) in [("30.5", 3_050_000i64), ("0", 0), ("-5", -500_000), ("0.00001", 1)] {
        let s: Score = serde_json::from_str(json).unwrap();
        assert_eq!(s.raw(), raw, "input {}", json);
    }
}

#[test]
fn score_serializes_as_decimal() {
    let json = serde_json::to_string(&Score::from_raw(3_050_000)).unwrap();
    assert_eq!(json, "30.5");
}

#[test]
fn score_rejects_huge_finite_value() {
    // 과거: (1e300 * 1e5) as i64 == i64::MAX 로 조용히 포화
    let err = serde_json::from_str::<Score>("1e300");
    assert!(err.is_err(), "1e300은 거부되어야 함");
    assert!(err.unwrap_err().to_string().contains("초과"));
}

#[test]
fn score_boundary_one_billion() {
    // 10억까지 허용 (parse_display_value와 동일 기준)
    let ok: Score = serde_json::from_str("1000000000").unwrap();
    assert_eq!(ok.raw(), 100_000_000_000_000);

    let err = serde_json::from_str::<Score>("1000000001");
    assert!(err.is_err(), "10억 초과는 거부되어야 함");
}

/// `Score` 역직렬화는 `(f * 100_000.0).round()` 로 최소 단위를 정한다.
/// 기존 테스트는 `0.00001`(= 정확히 1) 처럼 반올림이 개입하지 않는 값만 봐서,
/// `.round()` 가 절삭(`as i64`)으로 바뀌어도 전부 통과했다.
/// 최소 단위의 절반(0.000005)은 **올림**, 그 아래(0.000004)는 **버림**이어야 하며
/// 음수에서도 0에서 멀어지는 방향으로 대칭이어야 한다.
#[test]
fn score_deserialize_rounds_half_away_from_zero() {
    for (json, raw) in [
        ("0.000005", 1i64),   // 0.5 raw → 올림
        ("0.000004", 0),      // 0.4 raw → 버림
        ("-0.000005", -1),    // 음수도 0에서 멀어지는 방향
        ("-0.000004", 0),
    ] {
        let s: Score = serde_json::from_str(json).unwrap();
        assert_eq!(s.raw(), raw, "입력 {} 의 raw 값", json);
    }
}
