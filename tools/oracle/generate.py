"""
오라클 대조용 무작위 시나리오 생성기.

- 결정적 시드(SEED)로 재현 가능.
- 명세가 '오류'로 규정한 입력은 만들지 않는다 (성공 경로 대조가 목적).
  * UPPER 표에는 항상 threshold=0 행을 넣고 NUMERIC 값은 >= 0 으로 뽑는다.
  * EXACT 값은 표의 threshold 중에서 뽑는다.
  * 모든 (지원자 x 전형요소) 조합에 base_data 를 반드시 넣는다.
- base_data 의 track 스코프 모호성을 피하려고
  COMPOSITE area 는 (student, track) 별 행만, SIMPLE area 는 track NULL 행만 넣는다.
- 위 회피 규칙들이 비워 둔 영역(오류 경로 / COMPOSITE 점수표 폴백 / excluded·abandoned)은
  라운드 3 에서 round3.py 가 결정적 시나리오로 따로 채운다.
- 점수/기준값은 전부 x100000 raw 정수.

출력: scenarios.json  (Rust 덤프 테스트 tests/audit_oracle_dump.rs 가 같은 파일을 읽는다)
"""

import json
import random
import os

SEED = 20260817
N_SCENARIOS = 170

CALC_TYPES = ["NUMERIC", "CATEGORY", "MANUAL"]
MATCH_MODES = ["UPPER", "LOWER", "EXACT"]
CATEGORIES = ["1등급", "2등급", "3등급", "해당없음", "교내대회", "봉사우수", "결석", "지각"]


def gen_scenario(rnd, idx):
    scn = {"name": "s%03d" % idx, "round_id": 1}

    # ---- 전형요소
    n_areas = rnd.randint(1, 4)
    areas = []
    for a in range(1, n_areas + 1):
        ct = rnd.choice(CALC_TYPES)
        scope = rnd.choice(["SIMPLE", "COMPOSITE"])
        max_score = rnd.choice([100000, 500000, 1000000, 3000000, 10000000, 12345600])
        area = {
            "id": a,
            "name": "요소%d" % a,
            "calc_type": ct,
            "max_score": max_score,
            "match_mode": rnd.choice(MATCH_MODES) if ct == "NUMERIC" else None,
            "category_agg": rnd.choice(["SUM", "MAX"]) if ct == "CATEGORY" else None,
            "lookup_scope": scope,
            "multi_value": 0,
            "teacher_editable": 1,
        }
        if ct == "CATEGORY" and area["category_agg"] == "SUM":
            area["multi_value"] = 1
        areas.append(area)
    scn["areas"] = areas

    # ---- 대학 / 모집단위
    n_univ = rnd.randint(1, 3)
    univs, tracks = [], []
    tid = 0
    for u in range(1, n_univ + 1):
        prio_u = rnd.choice([0, 1])
        univs.append({
            "id": u,
            "univ_name": "대학%d" % u,
            "total_quota": rnd.choice([None, 1, 2, 3, 5]),
            "prioritize_enrolled": prio_u,
        })
        for _ in range(rnd.randint(1, 3)):
            tid += 1
            # 트리거 불변식: 대학=1 이면 모든 트랙=1
            prio_t = 1 if prio_u == 1 else rnd.choice([0, 1])
            tracks.append({
                "id": tid,
                "univ_id": u,
                "track_name": "모집단위%d" % tid,
                "unit_quota": rnd.choice([None, 1, 2, 3]),
                "prioritize_enrolled": prio_t,
            })
    scn["universities"] = univs
    scn["tracks"] = tracks

    # ---- 학생
    n_students = rnd.randint(4, 14)
    students = []
    for s in range(1, n_students + 1):
        enrolled = rnd.choice([0, 1])
        students.append({
            "id": s,
            "student_code": "%04d%03d" % (2026 - (0 if enrolled else 1), s),
            "name": "학생%02d" % s,
            "is_enrolled": enrolled,
            "grade": 3 if enrolled else None,
            "class_no": rnd.randint(1, 3) if enrolled else None,
            "seq_no": None,          # 아래에서 충돌 없이 배정
            "grad_year": None if enrolled else 2025,
        })
    # 재학생 (grade, class_no, seq_no) 유일성 확보
    seq_by_class = {}
    for st in students:
        if st["is_enrolled"]:
            k = (st["grade"], st["class_no"])
            seq_by_class[k] = seq_by_class.get(k, 0) + 1
            st["seq_no"] = seq_by_class[k]
    scn["students"] = students

    # ---- 지원 (student x track)
    apps = []
    for st in students:
        for t in rnd.sample(tracks, rnd.randint(1, min(2, len(tracks)))):
            apps.append({"student_id": st["id"], "track_id": t["id"], "round_id": 1})
    scn["applications"] = apps

    # ---- 점수표
    numeric_table, category_map = [], []
    for area in areas:
        # COMPOSITE 이면 실제 지원이 있는 트랙마다 표를 만든다 (폴백 경로를 피해 결정적으로)
        if area["lookup_scope"] == "COMPOSITE":
            scopes = sorted({a["track_id"] for a in apps})
        else:
            scopes = [None]

        if area["calc_type"] == "NUMERIC":
            for sc in scopes:
                n_rows = rnd.randint(2, 6)
                ths = sorted(rnd.sample(range(0, 1000000, 25000), n_rows))
                if area["match_mode"] == "UPPER" and ths[0] != 0:
                    ths[0] = 0
                for th in ths:
                    numeric_table.append({
                        "area_id": area["id"],
                        "track_id": sc,
                        "threshold": th,
                        "score": rnd.randint(-area["max_score"], area["max_score"]),
                    })
        elif area["calc_type"] == "CATEGORY":
            for sc in scopes:
                cats = rnd.sample(CATEGORIES, rnd.randint(2, 5))
                for i, c in enumerate(cats):
                    category_map.append({
                        "area_id": area["id"],
                        "track_id": sc,
                        "category": c,
                        "score": 0 if i == 0 else rnd.randint(-area["max_score"], area["max_score"]),
                    })
    scn["numeric_table"] = numeric_table
    scn["category_map"] = category_map

    # ---- 기초 데이터 (모든 지원자 x 모든 전형요소)
    base = []
    seen = set()
    for app in apps:
        for area in areas:
            scope = app["track_id"] if area["lookup_scope"] == "COMPOSITE" else None
            key = (app["student_id"], area["id"], scope)
            if key in seen:
                continue
            seen.add(key)

            if area["calc_type"] == "NUMERIC":
                ths = sorted(r["threshold"] for r in numeric_table
                             if r["area_id"] == area["id"] and r["track_id"] == scope)
                if area["match_mode"] == "EXACT":
                    v = rnd.choice(ths)
                elif area["match_mode"] == "UPPER":
                    v = rnd.randint(0, 1200000)
                else:  # LOWER — 상한 초과도 명세상 허용(최대 구간 사용)
                    v = rnd.randint(0, 1300000)
                base.append({"student_id": app["student_id"], "area_id": area["id"],
                             "track_id": scope, "value": str(v), "multi_value": 0})

            elif area["calc_type"] == "CATEGORY":
                cats = [r["category"] for r in category_map
                        if r["area_id"] == area["id"] and r["track_id"] == scope]
                if area["multi_value"]:
                    picked = rnd.sample(cats, rnd.randint(1, len(cats)))
                else:
                    picked = [rnd.choice(cats)]
                for c in picked:
                    base.append({"student_id": app["student_id"], "area_id": area["id"],
                                 "track_id": scope, "value": c,
                                 "multi_value": area["multi_value"]})

            else:  # MANUAL
                v = rnd.randint(-area["max_score"], area["max_score"] + 500000)
                base.append({"student_id": app["student_id"], "area_id": area["id"],
                             "track_id": scope, "value": str(v), "multi_value": 0})
    scn["base_data"] = base
    return scn


def main():
    rnd = random.Random(SEED)
    scns = [gen_scenario(rnd, i) for i in range(1, N_SCENARIOS + 1)]

    # 라운드 3 확장 — 무작위 스트림을 건드리지 않으려고 **뒤에** 붙인다.
    # 덕분에 s001~s170 은 2단계와 바이트 단위로 동일하다.
    import round3
    r3 = round3.scenarios()
    scns += r3

    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "scenarios.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(scns, f, ensure_ascii=False, indent=1)
    n_rows = sum(len(s["applications"]) for s in scns)
    print("시나리오 %d개 (무작위 %d + 라운드3 확장 %d), 결과 행(지원) %d건 -> %s"
          % (len(scns), N_SCENARIOS, len(r3), n_rows, out))


if __name__ == "__main__":
    main()
