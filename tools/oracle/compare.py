"""
오라클(expected) <-> 구현 덤프(actual) 대조.

입력:
  scenarios.json  — generate.py 산출
  actual.json     — tests/audit_oracle_dump.rs 산출 (PCM_ORACLE_DIR 지정 후 cargo test)

대조 항목 (지원 행 단위):
  1. total_score  : 오라클 raw 정수  vs  DB results.total_score (정수 그대로)
  2. total_score  : 오라클 raw/100000 vs  GET /rounds/:id/results 의 JSON f64 (x100000 이탈 경로)
  3. ranking      : 오라클 대학 순위  vs  results.ranking
  4. track_rank   : 오라클 모집단위 순위 vs 핸들러가 SQL RANK() 로 파생한 값
  5. score_detail : 전형요소별 raw 점수 맵
  6. excluded / abandoned : 화면이 상태 플래그를 그대로 싣는가 (라운드 3)

라운드 3 확장 — **오류 경로 대조**:
  명세가 Err 로 규정한 입력에서는 오라클도 OracleError 를 던지고 구현도 계산에 실패해야
  '일치'다. 한쪽만 실패하면 불일치다. 판정 표:

      오라클 Err | 구현 Err | 판정
      -----------+----------+--------------------------------
         O       |    O     | error_agree      (일치)
         O       |    X     | error_oracle_only(불일치)
         X       |    O     | error_impl_only  (불일치)
         X       |    X     | 행 단위 값 대조로 진행

  단, **명세가 규칙을 정하지 않은 지점**에서 갈린 경우는 별도 구획에 적는다.
  값이 틀린 것이 아니라 규칙이 없는 것이므로 같은 칸에 세면 오히려 사실을 흐린다.

출력: 불일치 유형별 건수 + 최소 재현 케이스.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import oracle  # noqa: E402


# 명세 공백에서 오라클과 구현이 갈리는 것이 **이미 알려진** 지점.
# 여기에 적으려면 02-findings.md / 03-round2-findings.md 에 S- 번호가 있어야 한다.
KNOWN_SPEC_GAP = {
    "r3b06_composite_base_data_track_missing": (
        "S-02 — COMPOSITE base_data 의 track 스코핑에 폴백이 있는지 명세가 침묵한다. "
        "오라클은 폴백 있음(공통 행 사용), 구현은 폴백 없음(Err). "
        "어느 쪽이 옳은지 명세로 판정할 수 없다."
    ),
}


def main():
    scns = {s["name"]: s for s in json.load(open(os.path.join(HERE, "scenarios.json"), encoding="utf-8"))}
    actual = json.load(open(os.path.join(HERE, "actual.json"), encoding="utf-8"))

    n_rows = 0
    n_scn = 0
    n_error_scn = 0
    mismatches = {
        "total_score_raw": [],
        "total_score_json": [],
        "ranking": [],
        "track_rank": [],
        "score_detail": [],
        "excluded_flag": [],
        "abandoned_flag": [],
        "missing_row": [],
        "extra_row": [],
        "error_oracle_only": [],
        "error_impl_only": [],
        "lifecycle_error": [],
    }
    error_agree = []
    spec_gap = []

    # 공허 통과 방지: actual 이 비었거나 시나리오 일부만 덤프됐는데 "불일치 0" 으로
    # 통과하면 대조가 없었던 것과 같다. 덤프 단계가 조용히 빠지는 경우를 여기서 잡는다.
    if not actual:
        print("actual.json 이 비어 있다 — 덤프 단계(audit_oracle_dump)가 돌지 않았다")
        return 1
    missing = set(scns) - {a["name"] for a in actual}
    if missing:
        print("덤프에 빠진 시나리오 %d개: %s" % (len(missing), ", ".join(sorted(missing)[:5])))
        return 1

    for act in actual:
        name = act["name"]
        scn = scns[name]
        n_scn += 1

        if act.get("lifecycle_error"):
            mismatches["lifecycle_error"].append((name, act["lifecycle_error"]))
            continue

        impl_err = act.get("calc_error")
        try:
            exp_rows = oracle.evaluate(scn)
            oracle_err = None
        except oracle.OracleError as e:
            exp_rows = None
            oracle_err = str(e)

        # ── 오류 경로 판정 ────────────────────────────────────────
        if oracle_err or impl_err:
            n_error_scn += 1
            if name in KNOWN_SPEC_GAP:
                spec_gap.append((name, KNOWN_SPEC_GAP[name], oracle_err, impl_err))
            elif oracle_err and impl_err:
                error_agree.append((name, oracle_err, impl_err))
            elif oracle_err:
                mismatches["error_oracle_only"].append((name, oracle_err))
            else:
                mismatches["error_impl_only"].append((name, impl_err))

            # 오류 시나리오는 결과 행이 남지 않아야 한다 (부분 저장 금지)
            if impl_err and act.get("rows"):
                mismatches["extra_row"].append(
                    (name, "계산 실패인데 results 행이 %d건 남았다" % len(act["rows"])))
            continue

        exp = {(r["student_id"], r["track_id"]): r for r in exp_rows}
        got = {(r["student_id"], r["track_id"]): r for r in act["rows"]}

        for k in exp:
            if k not in got:
                mismatches["missing_row"].append((name, k))
        for k in got:
            if k not in exp:
                mismatches["extra_row"].append((name, k))

        for k in sorted(set(exp) & set(got)):
            e, g = exp[k], got[k]
            n_rows += 1

            if e["total_score"] != g["total_score_raw"]:
                mismatches["total_score_raw"].append(
                    (name, k, e["total_score"], g["total_score_raw"]))

            # x100000 이탈 경로: 명세 §2.1 는 raw/100000.0 을 규정한다
            want_json = oracle.to_json_score(e["total_score"])
            if g["total_score_json"] is None or want_json != g["total_score_json"]:
                mismatches["total_score_json"].append(
                    (name, k, want_json, g["total_score_json"]))

            if e["ranking"] != g["ranking"]:
                mismatches["ranking"].append((name, k, e["ranking"], g["ranking"]))

            if e["track_rank"] != g["track_rank"]:
                mismatches["track_rank"].append((name, k, e["track_rank"], g["track_rank"]))

            if e["score_detail"] != {kk: vv for kk, vv in g["score_detail_raw"].items()}:
                mismatches["score_detail"].append(
                    (name, k, e["score_detail"], g["score_detail_raw"]))

            # 라운드 3 — 화면이 상태 플래그를 싣는가. 순위는 위에서 이미 대조됐으므로
            # 여기서 확인하는 것은 "순위가 그대로인데 표식도 함께 온다"는 결합이다.
            if g.get("excluded") is not None:
                if bool(e.get("excluded", 0)) != bool(g["excluded"]):
                    mismatches["excluded_flag"].append(
                        (name, k, e.get("excluded", 0), g["excluded"]))
            if g.get("abandoned") is not None:
                if bool(e.get("abandoned", 0)) != bool(g["abandoned"]):
                    mismatches["abandoned_flag"].append(
                        (name, k, e.get("abandoned", 0), g["abandoned"]))

    print("=" * 72)
    print("시나리오 %d개 (성공 경로 %d / 오류 경로 %d) / 대조 결과 행 %d건"
          % (n_scn, n_scn - n_error_scn, n_error_scn, n_rows))
    print("=" * 72)
    total_bad = 0
    for kind, items in mismatches.items():
        print("%-20s %5d 건" % (kind, len(items)))
        total_bad += len(items)
        for it in items[:3]:
            print("    최소 재현: %r" % (it,))
    print("-" * 72)
    print("불일치 합계: %d" % total_bad)

    print()
    print("오류 경로 일치 (오라클 Err == 구현 Err): %d 건" % len(error_agree))
    for name, oe, ie in error_agree:
        print("  %s" % name)
        print("      오라클: %s" % oe)
        print("      구현  : %s" % ie.replace("\n", " ")[:120])

    print()
    print("명세 공백에서 갈린 항목 (불일치 아님 — 명세로 판정 불가): %d 건" % len(spec_gap))
    for name, why, oe, ie in spec_gap:
        print("  %s" % name)
        print("      %s" % why)
        print("      오라클: %s" % (oe or "성공"))
        print("      구현  : %s" % ((ie or "성공").replace("\n", " ")[:120]))

    return 0 if total_bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
