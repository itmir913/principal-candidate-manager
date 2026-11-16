"""
실운영 PCM DB 를 독립 오라클(oracle.py)로 다시 계산해 저장된 결과와 대조한다.

    python -I tools/oracle/verify_db.py <data.db 경로> [--show-ids]

개인정보 원칙
  - 원본 DB 를 열지 않는다. 임시 폴더에 복사본(-wal/-shm 포함)을 만들어 읽기 전용으로 열고,
    끝나면 임시 폴더를 지운다. 원본 파일은 바뀌지 않는다.
  - students 에서는 id 와 is_enrolled 만 읽는다. 이름·학번·학년반번호는 SELECT 하지 않는다.
  - 출력은 건수뿐이다. 점수 값·기초데이터 값은 출력하지 않는다. 오라클 오류 문구는 값이 들어
    있으므로 첫 ':' 앞의 분류만 출력한다.
  - --show-ids 를 주면 불일치 행의 내부 번호(round_id, student_id, track_id)를 출력한다.
    이 번호는 이 DB 안에서만 학생을 가리킨다. 이 출력을 남에게 넘길 때는 주의하라.

대조하는 것
  - total_score, score_detail(전형요소별 점수), results.ranking(대학 전체 순위).
  - 순위는 라운드·대학 파티션으로 계산한다(scoring.rs 의 run_calculate_scores_on_conn 과 같은 범위).
  - 모집단위 순위(track_rank)는 DB 에 저장되지 않으므로 대조하지 않는다.

판별의 한계(읽고 해석하라)
  - 오라클은 명세를 보고 따로 쓴 구현이다. 명세 해석이 둘 다 같은 쪽으로 틀리면 일치한다.
  - 저장된 결과는 계산 시점의 값이다. 그 뒤 기초데이터·점수표를 바꿨으면 OPEN 라운드에서
    불일치가 나올 수 있다(구현 결함이 아니라 재계산 전 상태). 라운드 상태별로 나눠 보여 준다.
  - 정원·추천 확정(recommended)은 대조하지 않는다.
"""

import json
import os
import shutil
import sqlite3
import sys
import tempfile
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import oracle  # noqa: E402


def load(conn):
    q = lambda sql: [dict(r) for r in conn.execute(sql)]
    return {
        "areas": q("SELECT id, calc_type, max_score, match_mode, category_agg, lookup_scope "
                   "FROM areas ORDER BY id"),
        "universities": q("SELECT id, prioritize_enrolled FROM universities"),
        "tracks": q("SELECT id, univ_id, prioritize_enrolled FROM univ_tracks"),
        "students": q("SELECT id, is_enrolled FROM students"),
        "numeric_table": q("SELECT area_id, track_id, threshold, score FROM numeric_table"),
        "category_map": q("SELECT area_id, track_id, category, score FROM category_map"),
        "base_data": q("SELECT student_id, area_id, track_id, value FROM base_data"),
        "rounds": {r["id"]: r["status"] for r in q("SELECT id, status FROM rounds")},
        "applications": q("SELECT student_id, track_id, round_id, excluded, abandoned FROM applications"),
        "results": q("SELECT student_id, track_id, round_id, score_detail, total_score, ranking "
                     "FROM results"),
    }


def err_kind(e):
    return str(e).split(":", 1)[0].strip()


def verify(db_path, show_ids=False):
    tmp = tempfile.mkdtemp(prefix="pcm-verify-")
    try:
        for suffix in ("", "-wal", "-shm"):
            if os.path.exists(db_path + suffix):
                shutil.copy2(db_path + suffix, os.path.join(tmp, "copy.db" + suffix))
        uri = "file:" + os.path.join(tmp, "copy.db").replace("\\", "/") + "?mode=ro"
        conn = sqlite3.connect(uri, uri=True)
        conn.row_factory = sqlite3.Row
        try:
            d = load(conn)
        finally:
            conn.close()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    stored = {(r["round_id"], r["student_id"], r["track_id"]): r for r in d["results"]}
    apps_by_round = defaultdict(list)
    for a in d["applications"]:
        apps_by_round[a["round_id"]].append(a)

    stats = defaultdict(Counter)       # status -> Counter
    oracle_errors = Counter()
    bad_ids = defaultdict(list)

    for round_id, apps in sorted(apps_by_round.items()):
        status = d["rounds"].get(round_id, "UNKNOWN")
        st = stats[status]
        scn = {k: d[k] for k in ("areas", "universities", "tracks", "students",
                                 "numeric_table", "category_map", "base_data")}
        # 오라클 오류가 행 단위로 나도록 행마다 따로 계산한 뒤 순위를 모아서 매긴다.
        rows = []
        for a in apps:
            st["지원"] += 1
            key = (round_id, a["student_id"], a["track_id"])
            if key not in stored:
                st["결과 행 없음"] += 1
                continue
            try:
                tot, detail = oracle.total_score(scn, a["student_id"], a["track_id"])
            except oracle.OracleError as e:
                st["오라클 계산 불가"] += 1
                oracle_errors[err_kind(e)] += 1
                bad_ids["오라클 계산 불가"].append(key)
                continue
            rows.append((a, tot, detail))

        for a, tot, detail in rows:
            key = (round_id, a["student_id"], a["track_id"])
            s = stored[key]
            st["대조"] += 1
            if tot == s["total_score"]:
                st["총점 일치"] += 1
            else:
                st["총점 불일치"] += 1
                bad_ids["총점 불일치"].append(key)
            if detail == {k: int(v) for k, v in json.loads(s["score_detail"]).items()}:
                st["세부 점수 일치"] += 1
            else:
                st["세부 점수 불일치"] += 1
                bad_ids["세부 점수 불일치"].append(key)

        # 대학 순위: 라운드 안 대학 파티션. 오라클이 계산 못 한 행이 있으면 그 대학은 순위 대조를 건너뛴다.
        failed_univs = set()
        track_univ = {t["id"]: t["univ_id"] for t in d["tracks"]}
        for k in bad_ids["오라클 계산 불가"]:
            if k[0] == round_id:
                failed_univs.add(track_univ[k[2]])
        sub = dict(scn)
        sub["applications"] = [
            {"student_id": a["student_id"], "track_id": a["track_id"], "round_id": round_id}
            for a, _, _ in rows if track_univ[a["track_id"]] not in failed_univs
        ]
        if not sub["applications"]:
            continue
        for e in oracle.evaluate(sub):
            key = (round_id, e["student_id"], e["track_id"])
            got = stored[key]["ranking"]
            if got is None:
                st["순위 없음(미계산)"] += 1
            elif got == e["ranking"]:
                st["순위 일치"] += 1
            else:
                st["순위 불일치"] += 1
                bad_ids["순위 불일치"].append(key)

    print("=== PCM 실데이터 오라클 대조 ===")
    print("라운드 수:", len(apps_by_round), "| 전형요소 수:", len(d["areas"]))
    for status in sorted(stats):
        print("\n[라운드 상태 %s]" % status)
        for k in ("지원", "결과 행 없음", "오라클 계산 불가", "대조",
                  "총점 일치", "총점 불일치", "세부 점수 일치", "세부 점수 불일치",
                  "순위 일치", "순위 불일치", "순위 없음(미계산)"):
            if stats[status][k]:
                print("  %-14s %d" % (k, stats[status][k]))
    if oracle_errors:
        print("\n오라클 계산 불가 사유(분류만):")
        for k, n in oracle_errors.most_common():
            print("  %d  %s" % (n, k))
    if show_ids:
        for k, ids in bad_ids.items():
            if ids:
                print("\n%s (round_id, student_id, track_id):" % k)
                for i in ids:
                    print("  ", i)

    mismatch = sum(len(bad_ids[k]) for k in ("총점 불일치", "세부 점수 불일치", "순위 불일치"))
    print("\n판정:", "불일치 없음" if mismatch == 0 else "불일치 %d건" % mismatch)
    return 0 if mismatch == 0 else 1


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if len(args) != 1 or not os.path.isfile(args[0]):
        print("사용법: python -I tools/oracle/verify_db.py <data.db 경로> [--show-ids]")
        sys.exit(2)
    sys.exit(verify(args[0], show_ids="--show-ids" in sys.argv))
