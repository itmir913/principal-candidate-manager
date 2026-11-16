"""verify_db.py 자기검증 — run.mjs 5단계.

판별력의 소재: scenarios.json 과 Rust 실측(actual.json, 2단계가 현재 코드로 만든다)으로 시나리오마다
migrations 스키마의 합성 DB 를 만든다. 변조하지 않은 DB 는 verify() 가 0(일치)이어야 하고, 첫 결과 행의
총점·세부 점수·대학 순위를 하나씩 바꾼 DB 는 1(불일치)이어야 한다. 스키마나 오라클이 바뀌어
verify_db.py 가 낡으면 여기서 실패한다. calc_error 이거나 결과 행이 없는 시나리오는 건너뛴다.
"""
import contextlib, glob, io, json, os, shutil, sqlite3, sys, tempfile

REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
sys.path.insert(0, os.path.join(REPO, "tools", "oracle"))
import verify_db  # noqa

scns = json.load(open(os.path.join(REPO, "tools/oracle/scenarios.json"), encoding="utf8"))
acts = {a["name"]: a for a in json.load(open(os.path.join(REPO, "tools/oracle/actual.json"), encoding="utf8"))}
ddl = "".join(open(f, encoding="utf8").read() for f in
              sorted(glob.glob(os.path.join(REPO, "migrations/v1/*.sql"))) +
              sorted(glob.glob(os.path.join(REPO, "migrations/v2/*.sql"))))


def build(path, scn, rows, mutate=None):
    c = sqlite3.connect(path)
    c.executescript(ddl)
    ins = lambda t, d: c.execute("INSERT INTO %s (%s) VALUES (%s)" % (t, ",".join(d), ",".join("?" * len(d))), list(d.values()))
    st = "CLOSED" if scn.get("round_status") != "REOPENED" else "OPEN"
    c.execute("INSERT INTO rounds (id,status,opened_at) VALUES (?,?,'x')", (scn["round_id"], st))
    for a in scn["areas"]: ins("areas", a)
    for u in scn["universities"]: ins("universities", u)
    for t in scn["tracks"]: ins("univ_tracks", t)
    for s in scn["students"]: ins("students", s)
    for t in ("numeric_table", "category_map", "base_data"):
        for r in scn[t]: ins(t, r)
    for a in scn["applications"]:
        ins("applications", {k: a.get(k, 0) for k in ("student_id", "track_id", "round_id", "excluded", "abandoned")} |
            ({"excluded_reason": "x"} if a.get("excluded") else {}))
    for i, r in enumerate(rows):
        det, tot, rk = dict(r["score_detail_raw"]), r["total_score_raw"], r["ranking"]
        if mutate and i == 0:
            if mutate == "total": tot += 1
            if mutate == "detail": det[next(iter(det))] += 1
            if mutate == "rank": rk = (rk or 0) + 1
        c.execute("INSERT INTO results (student_id,track_id,round_id,score_detail,total_score,ranking,calculated_at) VALUES (?,?,?,?,?,?,'x')",
                  (r["student_id"], r["track_id"], r["round_id"], json.dumps(det), tot, rk))
    c.commit(); c.close()


ok = fail = skipped = 0
tmp = tempfile.mkdtemp()
for n, scn in enumerate(scns):
    act = acts.get(scn["name"])
    if not act or act.get("calc_error") or not act["rows"]:
        skipped += 1; continue
    for mutate in (None, "total", "detail", "rank"):
        if mutate == "rank" and act["rows"][0]["ranking"] is None:
            continue
        p = os.path.join(tmp, "%d-%s.db" % (n, mutate))
        build(p, scn, act["rows"], mutate)
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            rc = verify_db.verify(p)
        want = 0 if mutate is None else 1
        if rc == want: ok += 1
        else:
            fail += 1
            if fail <= 3: print("FAIL", scn["name"], mutate, rc, out.getvalue()[-400:])
shutil.rmtree(tmp, ignore_errors=True)
print("verify_db 자기검증: 통과 %d, 실패 %d, 건너뜀 %d" % (ok, fail, skipped))
if fail or not ok:
    sys.exit(1)
