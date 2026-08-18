"""
라운드 3 확장 시나리오 — 결정적(무작위 없음).

2단계 오라클(generate.py, 170개)이 **구조적으로 비워 둔** 세 영역을 채운다
(02-findings-review.md §3 이 지목한 것):

  A. excluded / abandoned 혼재            → 과제 2 (A-2)
  B. 명세가 Err 로 규정한 입력(오류 경로)  → 과제 3
  C. COMPOSITE 점수표 폴백                → 과제 3

generate.py 의 무작위 시나리오 뒤에 그대로 이어 붙는다. 무작위 스트림을 건드리지
않으므로 기존 170개(s001~s170)는 바이트 단위로 동일하게 유지된다.

시나리오 스키마 확장 (기존 키는 그대로):
  applications[] 에 선택 키 excluded / excluded_reason / abandoned / recommended
  최상위에 선택 키 round_status ("CLOSED" 기본, "FINALIZED" 가능)
  최상위에 선택 키 note (사람이 읽을 의도 설명)
"""


def _area(aid, calc_type, scope, max_score, match_mode=None, category_agg=None):
    return {
        "id": aid,
        "name": "요소%d" % aid,
        "calc_type": calc_type,
        "max_score": max_score,
        "match_mode": match_mode,
        "category_agg": category_agg,
        "lookup_scope": scope,
        "multi_value": 1 if category_agg == "SUM" else 0,
        "teacher_editable": 1,
    }


def _student(sid, enrolled=1, seq=None):
    return {
        "id": sid,
        "student_code": "%04d%03d" % (2026 if enrolled else 2025, sid),
        "name": "학생%02d" % sid,
        "is_enrolled": enrolled,
        "grade": 3 if enrolled else None,
        "class_no": 1 if enrolled else None,
        "seq_no": (seq if seq is not None else sid) if enrolled else None,
        "grad_year": None if enrolled else 2025,
    }


def _app(sid, tid, **flags):
    a = {"student_id": sid, "track_id": tid, "round_id": 1}
    a.update(flags)
    return a


def _base(sid, aid, track_id, value, multi_value=0):
    return {
        "student_id": sid,
        "area_id": aid,
        "track_id": track_id,
        "value": str(value),
        "multi_value": multi_value,
    }


def _skeleton(name, note, areas, tracks_spec, students, apps,
              numeric_table, category_map, base_data, round_status="CLOSED"):
    """tracks_spec: [(track_id, univ_id, unit_quota, prioritize)] / 대학은 univ_id 별로 자동 생성."""
    univ_ids = sorted({u for _, u, _, _ in tracks_spec})
    return {
        "name": name,
        "note": note,
        "round_id": 1,
        "round_status": round_status,
        "areas": areas,
        "universities": [
            {"id": u, "univ_name": "대학%d" % u, "total_quota": None, "prioritize_enrolled": 0}
            for u in univ_ids
        ],
        "tracks": [
            {"id": t, "univ_id": u, "track_name": "모집단위%d" % t,
             "unit_quota": q, "prioritize_enrolled": p}
            for t, u, q, p in tracks_spec
        ],
        "students": students,
        "applications": apps,
        "numeric_table": numeric_table,
        "category_map": category_map,
        "base_data": base_data,
    }


# ─────────────────────────────────────────── A. excluded / abandoned

def _group_a():
    """순위가 excluded/abandoned 에 흔들리지 않는지 — MANUAL 로 점수를 직접 고정한다."""
    out = []

    # A-1. 한 모집단위 상위 2명 미선발. 3위가 여전히 3위로 나오는가.
    students = [_student(i) for i in range(1, 5)]
    out.append(_skeleton(
        "r3a01_excluded_top2",
        "상위 2명 excluded=1. track_rank/ranking 은 1,2,3,4 그대로여야 한다(명세 §5.3:450).",
        areas=[_area(1, "MANUAL", "SIMPLE", 10_000_000)],
        tracks_spec=[(1, 1, None, 0)],
        students=students,
        apps=[
            _app(1, 1, excluded=1, excluded_reason="감사: 미선발"),
            _app(2, 1, excluded=1, excluded_reason="감사: 미선발"),
            _app(3, 1),
            _app(4, 1),
        ],
        numeric_table=[], category_map=[],
        base_data=[_base(i, 1, None, v) for i, v in
                   [(1, 10_000_000), (2, 9_000_000), (3, 8_000_000), (4, 7_000_000)]],
    ))

    # A-2. 추천 확정 후 전원 포기. 순위는 남고 정원만 반환된다(§6.1:528).
    out.append(_skeleton(
        "r3a02_all_abandoned",
        "전원 recommended=1 + abandoned=1 (FINALIZED). 순위는 유지, 정원 집계만 0.",
        areas=[_area(1, "MANUAL", "SIMPLE", 10_000_000)],
        tracks_spec=[(1, 1, 3, 0)],
        students=[_student(i) for i in range(1, 4)],
        apps=[_app(i, 1, abandoned=1, recommended=1) for i in range(1, 4)],
        numeric_table=[], category_map=[],
        base_data=[_base(i, 1, None, v) for i, v in
                   [(1, 10_000_000), (2, 9_000_000), (3, 8_000_000)]],
        round_status="FINALIZED",
    ))

    # A-3. 미선발 + 포기 + 정상이 한 트랙에 섞이고 동점까지 있는 경우.
    #      FINALIZED 로 가려면 **모든 지원이 excluded=1 이거나 recommended=1** 이어야 한다
    #      (trg_require_all_decided_before_finalize, 003-rounds.sql:26). 그래서 abandoned 를
    #      섞은 시나리오는 필연적으로 "전원 결정됨" 상태가 된다 — 03-round2-findings.md §2.3 참조.
    out.append(_skeleton(
        "r3a03_mixed_with_tie",
        "excluded/abandoned/추천 혼재 + 동점 2쌍. Standard Competition Ranking 이 흔들리지 않아야 한다.",
        areas=[_area(1, "MANUAL", "SIMPLE", 10_000_000)],
        tracks_spec=[(1, 1, 2, 0), (2, 1, 2, 0)],
        students=[_student(i) for i in range(1, 7)],
        apps=[
            _app(1, 1, excluded=1, excluded_reason="감사: 미선발"),
            _app(2, 1, recommended=1),
            _app(3, 1, recommended=1, abandoned=1),
            _app(4, 2, recommended=1),
            _app(5, 2, excluded=1, excluded_reason="감사: 미선발"),
            _app(6, 2, recommended=1),
        ],
        numeric_table=[], category_map=[],
        base_data=[_base(i, 1, None, v) for i, v in [
            (1, 9_000_000), (2, 9_000_000),   # 트랙1 동점
            (3, 5_000_000),
            (4, 7_000_000), (5, 7_000_000),   # 트랙2 동점
            (6, 3_000_000),
        ]],
        round_status="FINALIZED",
    ))

    # A-4. 재학생 우선 + 미선발 혼재 — 두 정렬 키가 함께 걸릴 때도 같은가.
    out.append(_skeleton(
        "r3a04_excluded_with_prioritize",
        "prioritize_enrolled=1 인 트랙에서 재학생 1위를 excluded 처리. 졸업생이 2위로 남는가.",
        areas=[_area(1, "MANUAL", "SIMPLE", 10_000_000)],
        tracks_spec=[(1, 1, None, 1)],
        students=[_student(1, enrolled=1, seq=1), _student(2, enrolled=0), _student(3, enrolled=1, seq=2)],
        apps=[
            _app(1, 1, excluded=1, excluded_reason="감사: 미선발"),
            _app(2, 1),
            _app(3, 1),
        ],
        numeric_table=[], category_map=[],
        base_data=[_base(i, 1, None, v) for i, v in
                   [(1, 5_000_000), (2, 10_000_000), (3, 4_000_000)]],
    ))
    return out


# ─────────────────────────────────────────────────── B. 오류 경로

def _group_b():
    """명세가 Err 로 규정한 입력. 오라클도 실패하고 구현도 실패해야 '일치'다."""
    out = []

    # B-1. UPPER 하한 미만 (명세 §2.3 NUMERIC 3 — 모든 하한치보다 작으면 오류)
    out.append(_skeleton(
        "r3b01_upper_below_min",
        "UPPER 표의 최소 threshold=1000000 인데 값 500000 → Err.",
        areas=[_area(1, "NUMERIC", "SIMPLE", 10_000_000, match_mode="UPPER")],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(1)],
        apps=[_app(1, 1)],
        numeric_table=[
            {"area_id": 1, "track_id": None, "threshold": 1_000_000, "score": 5_000_000},
            {"area_id": 1, "track_id": None, "threshold": 2_000_000, "score": 8_000_000},
        ],
        category_map=[],
        base_data=[_base(1, 1, None, 500_000)],
    ))

    # B-2. EXACT 미매칭
    out.append(_skeleton(
        "r3b02_exact_no_match",
        "EXACT 표에 없는 값 → Err.",
        areas=[_area(1, "NUMERIC", "SIMPLE", 10_000_000, match_mode="EXACT")],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(1)],
        apps=[_app(1, 1)],
        numeric_table=[
            {"area_id": 1, "track_id": None, "threshold": 100_000, "score": 1_000_000},
            {"area_id": 1, "track_id": None, "threshold": 200_000, "score": 2_000_000},
        ],
        category_map=[],
        base_data=[_base(1, 1, None, 150_000)],
    ))

    # B-3. 범주 미등록
    out.append(_skeleton(
        "r3b03_category_unmapped",
        "base_data 범주 '미등록범주' 가 category_map 에 없음 → Err.",
        areas=[_area(1, "CATEGORY", "SIMPLE", 10_000_000, category_agg="MAX")],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(1)],
        apps=[_app(1, 1)],
        numeric_table=[],
        category_map=[
            {"area_id": 1, "track_id": None, "category": "1등급", "score": 0},
            {"area_id": 1, "track_id": None, "category": "2등급", "score": 3_000_000},
        ],
        base_data=[_base(1, 1, None, "미등록범주")],
    ))

    # B-4. base_data 없음
    out.append(_skeleton(
        "r3b04_base_data_missing",
        "지원자에 대한 base_data 행이 아예 없음 → Err.",
        areas=[_area(1, "NUMERIC", "SIMPLE", 10_000_000, match_mode="LOWER")],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(1)],
        apps=[_app(1, 1)],
        numeric_table=[
            {"area_id": 1, "track_id": None, "threshold": 100_000, "score": 1_000_000},
        ],
        category_map=[],
        base_data=[],
    ))

    # B-5. 전형요소가 2개인데 한 명만 오류 — All-or-Nothing 이면 라운드 전체가 Err
    out.append(_skeleton(
        "r3b05_one_of_many_fails",
        "정상 지원자 2명 + 오류 지원자 1명. 계산은 전체가 Err 여야 한다(부분 저장 없음).",
        areas=[_area(1, "NUMERIC", "SIMPLE", 10_000_000, match_mode="UPPER")],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(i) for i in range(1, 4)],
        apps=[_app(1, 1), _app(2, 1), _app(3, 1)],
        numeric_table=[
            {"area_id": 1, "track_id": None, "threshold": 1_000_000, "score": 5_000_000},
        ],
        category_map=[],
        base_data=[
            _base(1, 1, None, 2_000_000),
            _base(2, 1, None, 3_000_000),
            _base(3, 1, None, 0),          # 하한 미만 → Err
        ],
    ))

    # B-6. COMPOSITE 인데 해당 트랙 base_data 없음 (S-02 가 지적한 비대칭 지점)
    out.append(_skeleton(
        "r3b06_composite_base_data_track_missing",
        "COMPOSITE area 에 공통(track NULL) base_data 만 있고 트랙 행이 없다. "
        "구현은 폴백이 없어 Err. 오라클은 폴백이 있어 성공 — 명세 공백 S-02 가 실제로 갈리는 지점.",
        areas=[_area(1, "MANUAL", "COMPOSITE", 10_000_000)],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(1)],
        apps=[_app(1, 1)],
        numeric_table=[], category_map=[],
        base_data=[_base(1, 1, None, 5_000_000)],   # track_id=None 뿐
    ))
    return out


# ────────────────────────────────────── C. COMPOSITE 점수표 폴백

def _group_c():
    """generate.py:114-115 가 '폴백 경로를 피해' 트랙마다 표를 만든 그 경로를 정면으로 친다.

    base_data 는 트랙 스코프에 둔다 (구현이 COMPOSITE base_data 에 폴백을 두지 않으므로 — S-02).
    비워 두는 것은 **점수표**뿐이다. 그래야 §2.3 이 규정한 점수표 폴백만 분리해서 볼 수 있다.
    """
    out = []

    # C-1. NUMERIC COMPOSITE — 트랙별 표가 전혀 없음 → 공통 표 폴백
    out.append(_skeleton(
        "r3c01_numeric_composite_full_fallback",
        "COMPOSITE NUMERIC 인데 numeric_table 에 트랙 행이 0건. 공통(track NULL) 표로 폴백.",
        areas=[_area(1, "NUMERIC", "COMPOSITE", 10_000_000, match_mode="UPPER")],
        tracks_spec=[(1, 1, None, 0), (2, 1, None, 0)],
        students=[_student(i) for i in range(1, 4)],
        apps=[_app(1, 1), _app(2, 1), _app(3, 2)],
        numeric_table=[
            {"area_id": 1, "track_id": None, "threshold": 0, "score": 1_000_000},
            {"area_id": 1, "track_id": None, "threshold": 500_000, "score": 4_000_000},
            {"area_id": 1, "track_id": None, "threshold": 900_000, "score": 9_000_000},
        ],
        category_map=[],
        base_data=[
            _base(1, 1, 1, 300_000),
            _base(2, 1, 1, 950_000),
            _base(3, 1, 2, 600_000),
        ],
    ))

    # C-2. NUMERIC COMPOSITE — 트랙1 에만 표가 있고 트랙2 는 없음 (부분 폴백)
    out.append(_skeleton(
        "r3c02_numeric_composite_partial_fallback",
        "트랙1 은 전용 표, 트랙2 는 표 없음 → 트랙2 만 공통 표로 폴백. 같은 값이 트랙마다 다른 점수를 받아야 한다.",
        areas=[_area(1, "NUMERIC", "COMPOSITE", 10_000_000, match_mode="UPPER")],
        tracks_spec=[(1, 1, None, 0), (2, 1, None, 0)],
        students=[_student(1), _student(2)],
        apps=[_app(1, 1), _app(2, 2)],
        numeric_table=[
            {"area_id": 1, "track_id": 1, "threshold": 0, "score": 2_000_000},
            {"area_id": 1, "track_id": 1, "threshold": 500_000, "score": 7_000_000},
            {"area_id": 1, "track_id": None, "threshold": 0, "score": 1_000_000},
            {"area_id": 1, "track_id": None, "threshold": 500_000, "score": 3_000_000},
        ],
        category_map=[],
        base_data=[_base(1, 1, 1, 600_000), _base(2, 1, 2, 600_000)],
    ))

    # C-3. CATEGORY COMPOSITE — per-category 폴백 (whole-map 폴백이 아님을 가른다)
    out.append(_skeleton(
        "r3c03_category_per_category_fallback",
        "트랙 표에 '1등급' 만 있고 '2등급' 은 공통 표에만 있다. "
        "명세 §2.3 CATEGORY 2 는 범주 하나씩 폴백하므로 두 범주 모두 점수를 받아야 한다. "
        "whole-map 폴백이면 '1등급' 이 트랙 표에 있다는 이유로 '2등급' 이 오류가 난다.",
        areas=[_area(1, "CATEGORY", "COMPOSITE", 10_000_000, category_agg="SUM")],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(1), _student(2)],
        apps=[_app(1, 1), _app(2, 1)],
        numeric_table=[],
        category_map=[
            {"area_id": 1, "track_id": 1, "category": "1등급", "score": 0},
            {"area_id": 1, "track_id": 1, "category": "봉사우수", "score": 2_000_000},
            {"area_id": 1, "track_id": None, "category": "1등급", "score": 0},
            {"area_id": 1, "track_id": None, "category": "2등급", "score": 500_000},
        ],
        base_data=[
            _base(1, 1, 1, "1등급", multi_value=1),
            _base(1, 1, 1, "2등급", multi_value=1),      # 트랙 표에 없음 → 공통 폴백
            _base(2, 1, 1, "봉사우수", multi_value=1),
        ],
    ))

    # C-4. CATEGORY COMPOSITE MAX — 폴백 + 집계 방식 조합
    out.append(_skeleton(
        "r3c04_category_fallback_max",
        "MAX 집계에서 폴백으로 얻은 점수가 최대값일 때 그 값이 선택되는가.",
        areas=[_area(1, "CATEGORY", "COMPOSITE", 10_000_000, category_agg="MAX")],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(1)],
        apps=[_app(1, 1)],
        numeric_table=[],
        category_map=[
            {"area_id": 1, "track_id": 1, "category": "1등급", "score": 1_000_000},
            {"area_id": 1, "track_id": None, "category": "특별", "score": 6_000_000},
        ],
        base_data=[_base(1, 1, 1, "특별")],
    ))

    # C-5. COMPOSITE 폴백 + max_score 캡핑이 함께 걸리는 경우
    out.append(_skeleton(
        "r3c05_fallback_then_cap",
        "공통 표로 폴백한 점수가 max_score 를 넘는다 → 캡핑까지 통과해야 값이 맞는다.",
        areas=[_area(1, "NUMERIC", "COMPOSITE", 2_000_000, match_mode="LOWER")],
        tracks_spec=[(1, 1, None, 0)],
        students=[_student(1)],
        apps=[_app(1, 1)],
        numeric_table=[
            {"area_id": 1, "track_id": None, "threshold": 100_000, "score": 9_000_000},
        ],
        category_map=[],
        base_data=[_base(1, 1, 1, 50_000)],
    ))
    return out


def scenarios():
    return _group_a() + _group_b() + _group_c()
