"""
PCM 채점 독립 오라클
====================

근거 문서 (이 파일 작성 시점에 읽은 것 전부):
  - src/docs/00_spec_round_and_scoring.md  §2 (성적 계산), §3 (순위 산출)
  - src/docs/08_excel_import.md            §5 (base_data 저장 형태), 파싱 Fail-Fast

작성 순서 고지 (PROMPT-02-audit.md "작성 순서 강제"):
  이 파일은 src/handlers/scoring.rs 를 **한 번도 열지 않은 상태**에서 작성했다.
  읽은 것은 위 두 명세와 1단계 인벤토리(좌표 목록)뿐이다.
  인벤토리는 함수 이름·줄번호·"어디서 나눗셈이 일어나는가" 같은 좌표만 담고 있고
  점수·순위 규칙 자체는 담고 있지 않으므로, 규칙의 출처는 명세 §2·§3 하나다.

구현하는 규칙 (명세 인용):
  §2.1  모든 점수는 x100000 정수. JSON 이탈 시 raw/100000.0
  §2.3  NUMERIC : base_data 정수 -> numeric_table 구간 조회 -> min(max_score)
        CATEGORY: base_data 범주들 -> category_map 조회 -> SUM|MAX -> min(max_score)
        MANUAL  : base_data 정수 그대로 -> min(max_score) (하한 없음)
  §2.4  전 전형요소 합산 = total_score
  §3.1  results.ranking = 대학 파티션, universities.prioritize_enrolled 만 참조
  §3.3  track_rank     = 모집단위 파티션, univ_tracks.prioritize_enrolled 만 참조
  §3.4  Standard Competition Ranking (1, 1, 3, 4)
  §3.5  각 범위는 자기 플래그만 쓴다 (OR 금지)

라운드 3 추가 (03-round2-findings.md 과제 2):
  §5.3:450 "순위 계산(RANK())은 excluded 포함 전원으로 계산"
        -> ranking / track_rank 파티션에서 excluded 행을 빼지 않는다.
  §6.1:528 정원 집계 영향 — excluded 없음 / abandoned 있음
        -> 정원은 이 오라클의 범위 밖(순위만 다룬다). abandoned 를 순위에서
           빼야 한다는 조항은 **어느 문서에도 없으므로** 여기서도 빼지 않는다.
           그 침묵 자체가 결과다 (S-08).
"""


class OracleError(Exception):
    """명세가 '오류'라고 규정한 상황. Fail-Fast."""


# ---------------------------------------------------------------- 2.3 NUMERIC

def lookup_range_score(match_mode, value, rows):
    """§2.3 NUMERIC 3.  rows = [(threshold, score)] (정렬 무관, 여기서 정렬한다)."""
    if not rows:
        raise OracleError("numeric_table 비어 있음")
    rows = sorted(rows, key=lambda r: r[0])

    if match_mode == "UPPER":
        # value >= threshold 인 행 중 최대 threshold 행. 모두보다 작으면 오류.
        cand = [r for r in rows if value >= r[0]]
        if not cand:
            raise OracleError(
                "UPPER: 값 %d 이 모든 기준값보다 작음 (최소 기준값 %d)" % (value, rows[0][0])
            )
        return max(cand, key=lambda r: r[0])[1]

    if match_mode == "LOWER":
        # value <= threshold 인 행 중 최소 threshold 행.
        # value > 최대 threshold 이면 최대 threshold 행 사용 (오류 아님).
        cand = [r for r in rows if value <= r[0]]
        if not cand:
            return rows[-1][1]
        return min(cand, key=lambda r: r[0])[1]

    if match_mode == "EXACT":
        for th, sc in rows:
            if th == value:
                return sc
        raise OracleError("EXACT: 값 %d 과 일치하는 기준값 없음" % value)

    raise OracleError("알 수 없는 match_mode: %r" % (match_mode,))


# ------------------------------------------------------- 점수표 조회 + COMPOSITE 폴백

def _numeric_rows(scn, area, track_id):
    """§2.3 NUMERIC 2. COMPOSITE 이고 모집단위별 표가 없으면 공통(track_id IS NULL) 폴백."""
    all_rows = [r for r in scn["numeric_table"] if r["area_id"] == area["id"]]
    if area["lookup_scope"] == "COMPOSITE":
        per_track = [(r["threshold"], r["score"]) for r in all_rows if r["track_id"] == track_id]
        if per_track:
            return per_track
    return [(r["threshold"], r["score"]) for r in all_rows if r["track_id"] is None]


def _category_score(scn, area, track_id, category):
    """§2.3 CATEGORY 2. per-category 폴백: 모집단위별 없으면 공통 테이블."""
    rows = [r for r in scn["category_map"] if r["area_id"] == area["id"]]
    if area["lookup_scope"] == "COMPOSITE":
        for r in rows:
            if r["track_id"] == track_id and r["category"] == category:
                return r["score"]
    for r in rows:
        if r["track_id"] is None and r["category"] == category:
            return r["score"]
    raise OracleError("category_map 에 범주 %r 없음 (area %d)" % (category, area["id"]))


# ------------------------------------------------------------------ base_data 조회

def _base_values(scn, area, student_id, track_id):
    """base_data 행 조회.

    명세 공백: §2.3 은 'base_data 에서 조회'라고만 하고 track_id 스코핑 규칙을
    규정하지 않는다. 오라클은 다음 규칙을 쓴다.
      COMPOSITE : (student, area, track) 행이 있으면 그것, 없으면 (student, area, NULL)
      SIMPLE    : (student, area, NULL)
    비교용 시나리오는 이 모호성이 결과를 바꾸지 않도록 생성한다
    (generate.py 는 area.lookup_scope 에 맞는 스코프 한 곳에만 행을 넣는다).
    """
    rows = [r for r in scn["base_data"]
            if r["student_id"] == student_id and r["area_id"] == area["id"]]
    if area["lookup_scope"] == "COMPOSITE":
        per_track = [r["value"] for r in rows if r["track_id"] == track_id]
        if per_track:
            return per_track
    return [r["value"] for r in rows if r["track_id"] is None]


# ------------------------------------------------------------- 2.3 전형요소 점수

I64_MIN, I64_MAX = -(2 ** 63), 2 ** 63 - 1


def _checked_add(a, b):
    """§2.3 CATEGORY SUM / §2.4 — checked_add. overflow 는 Fail-Fast."""
    s = a + b
    if s < I64_MIN or s > I64_MAX:
        raise OracleError("overflow: %d + %d" % (a, b))
    return s


def area_score(scn, area, student_id, track_id):
    ct = area["calc_type"]
    vals = _base_values(scn, area, student_id, track_id)

    if ct == "NUMERIC":
        if len(vals) != 1:
            raise OracleError("NUMERIC base_data 행이 1개가 아님 (%d개)" % len(vals))
        try:
            v = int(vals[0])
        except ValueError:
            raise OracleError("NUMERIC base_data 값 파싱 실패: %r" % (vals[0],))
        raw = lookup_range_score(area["match_mode"], v, _numeric_rows(scn, area, track_id))

    elif ct == "CATEGORY":
        if not vals:
            raise OracleError("CATEGORY base_data 0건")
        scores = [_category_score(scn, area, track_id, c) for c in vals]
        if area["category_agg"] == "SUM":
            raw = 0
            for s in scores:
                raw = _checked_add(raw, s)
        elif area["category_agg"] == "MAX":
            raw = max(scores)
        else:
            raise OracleError("알 수 없는 category_agg: %r" % (area["category_agg"],))

    elif ct == "MANUAL":
        if len(vals) != 1:
            raise OracleError("MANUAL base_data 행이 1개가 아님 (%d개)" % len(vals))
        try:
            raw = int(vals[0])
        except ValueError:
            raise OracleError("MANUAL base_data 값 파싱 실패: %r" % (vals[0],))

    else:
        raise OracleError("알 수 없는 calc_type: %r" % (ct,))

    # §2.3 4 — 상한만 적용. 하한 없음(음수 통과, 감점 설계).
    return min(raw, area["max_score"])


# ---------------------------------------------------------------- 2.4 총점 합산

def total_score(scn, student_id, track_id):
    total = 0
    detail = {}
    for area in scn["areas"]:
        s = area_score(scn, area, student_id, track_id)
        detail[str(area["id"])] = s
        total = _checked_add(total, s)
    return total, detail


# ------------------------------------------------------------------ 3.4 순위 계산

def standard_competition_rank(rows, order_key, tie_key):
    """§3.4 — 같은 순위 값이면 같은 순위, 다음 순위는 건너뛴다 (1, 1, 3, 4).

    order_key : 정렬용 (오름차순 정렬되므로 DESC 는 음수화해서 넘긴다)
    tie_key   : 동점 판정용. order_key 와 일관되어야 그룹이 연속된다.
    반환: {(student_id, track_id): rank}
    """
    ordered = sorted(rows, key=order_key)
    out = {}
    i, n = 0, len(ordered)
    while i < n:
        j = i
        while j + 1 < n and tie_key(ordered[j + 1]) == tie_key(ordered[i]):
            j += 1
        for k in range(i, j + 1):
            out[(ordered[k]["student_id"], ordered[k]["track_id"])] = i + 1
        i = j + 1
    return out


# ------------------------------------------------------------------ 시나리오 평가

def evaluate(scn):
    """시나리오 -> [{student_id, track_id, round_id, total_score, ranking, track_rank, score_detail}]"""
    students = {s["id"]: s for s in scn["students"]}
    tracks = {t["id"]: t for t in scn["tracks"]}
    univs = {u["id"]: u for u in scn["universities"]}

    rows = []
    for app in scn["applications"]:
        tot, detail = total_score(scn, app["student_id"], app["track_id"])
        st = students[app["student_id"]]
        tr = tracks[app["track_id"]]
        rows.append({
            "student_id": app["student_id"],
            "track_id": app["track_id"],
            "round_id": app["round_id"],
            "total_score": tot,
            "score_detail": detail,
            "is_enrolled": st["is_enrolled"],
            "univ_id": tr["univ_id"],
            # §5.3:450 — 순위 계산에서 빼지 않는다. 아래 파티션 어디에도 필터가 없다.
            "excluded": app.get("excluded", 0),
            "abandoned": app.get("abandoned", 0),
        })

    # §3.1 대학 전체 순위 — universities.prioritize_enrolled 만 참조 (§3.5)
    # 파티션은 '지원 전원'이다. excluded/abandoned 필터가 없는 것이 의도된 상태다(§5.3:450).
    for uid, u in univs.items():
        part = [r for r in rows if r["univ_id"] == uid]
        if not part:
            continue
        if u["prioritize_enrolled"]:
            ranks = standard_competition_rank(
                part,
                order_key=lambda r: (-r["is_enrolled"], -r["total_score"]),
                tie_key=lambda r: (r["total_score"], r["is_enrolled"]),
            )
        else:
            ranks = standard_competition_rank(
                part,
                order_key=lambda r: (-r["total_score"],),
                tie_key=lambda r: (r["total_score"],),
            )
        for r in part:
            r["ranking"] = ranks[(r["student_id"], r["track_id"])]

    # §3.2/§3.3 모집단위 순위 — univ_tracks.prioritize_enrolled 만 참조 (§3.5)
    # SQL: ORDER BY CASE WHEN ut.prioritize_enrolled=1 THEN s.is_enrolled ELSE NULL END
    #                DESC NULLS LAST, r.total_score DESC
    # prioritize=0 이면 첫 키가 전 행 NULL 이라 total_score 만 남는다.
    for tid, t in tracks.items():
        part = [r for r in rows if r["track_id"] == tid]
        if not part:
            continue
        if t["prioritize_enrolled"]:
            ranks = standard_competition_rank(
                part,
                order_key=lambda r: (-r["is_enrolled"], -r["total_score"]),
                tie_key=lambda r: (r["is_enrolled"], r["total_score"]),
            )
        else:
            ranks = standard_competition_rank(
                part,
                order_key=lambda r: (-r["total_score"],),
                tie_key=lambda r: (r["total_score"],),
            )
        for r in part:
            r["track_rank"] = ranks[(r["student_id"], r["track_id"])]

    for r in rows:
        r.pop("is_enrolled")
        r.pop("univ_id")
    return sorted(rows, key=lambda r: (r["student_id"], r["track_id"], r["round_id"]))


# --------------------------------------------------------- 2.1 JSON 이탈 (참고용)

def to_json_score(raw):
    """§2.1 — JSON 직렬화는 raw/100000.0"""
    return raw / 100000.0
