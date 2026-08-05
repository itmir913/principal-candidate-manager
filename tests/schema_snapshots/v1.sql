-- PCM 스키마 지문 v1 (생성 당시 앱 버전 0.2.12)
--
-- 자동 생성 파일. 손으로 고치지 말 것 — tests/schema_freeze.rs 가 실제 스키마와 대조한다.
-- 이 파일이 존재하는 순간 v1 스키마는 동결이다.
-- migrations/v1/*.sql 을 고치면 이 테스트가 깨진다. 그때는 파일을 맞추지 말고
-- 새 버전(migrations/v2/ + V2_FRAGMENTS)을 추가하고 SCHEMA_VERSION을 올려라.
--- SCHEMA FINGERPRINT ---
PRAGMA user_version = 1;

[index] idx_applications_round (on applications)
CREATE INDEX idx_applications_round ON applications(round_id);

[index] idx_audit_log_at (on audit_log)
CREATE INDEX idx_audit_log_at ON audit_log(at);

[index] idx_audit_log_round (on audit_log)
CREATE INDEX idx_audit_log_round ON audit_log(round_id);

[index] idx_base_data_multi (on base_data)
CREATE UNIQUE INDEX idx_base_data_multi ON base_data(student_id, area_id, COALESCE(track_id, 0), value) WHERE multi_value = 1;

[index] idx_base_data_single (on base_data)
CREATE UNIQUE INDEX idx_base_data_single ON base_data(student_id, area_id, COALESCE(track_id, 0)) WHERE multi_value = 0;

[index] idx_base_data_student (on base_data)
CREATE INDEX idx_base_data_student ON base_data(student_id);

[index] idx_category_map (on category_map)
CREATE UNIQUE INDEX idx_category_map ON category_map(area_id, COALESCE(track_id, 0), category);

[index] idx_numeric_table (on numeric_table)
CREATE UNIQUE INDEX idx_numeric_table ON numeric_table(area_id, COALESCE(track_id, 0), threshold);

[index] idx_one_active_round (on rounds)
CREATE UNIQUE INDEX idx_one_active_round ON rounds((1)) WHERE status != 'FINALIZED';

[index] idx_one_open_round (on rounds)
CREATE UNIQUE INDEX idx_one_open_round ON rounds(status) WHERE status = 'OPEN';

[index] idx_results_round_track (on results)
CREATE INDEX idx_results_round_track ON results(round_id, track_id);

[index] idx_students_position (on students)
CREATE UNIQUE INDEX idx_students_position ON students(grade, class_no, seq_no) WHERE is_enrolled = 1;

[index] sqlite_autoindex_app_configs_1 (on app_configs)
(암시적 — 제약이 생성한 인덱스)

[index] sqlite_autoindex_applications_1 (on applications)
(암시적 — 제약이 생성한 인덱스)

[index] sqlite_autoindex_areas_1 (on areas)
(암시적 — 제약이 생성한 인덱스)

[index] sqlite_autoindex_classes_1 (on classes)
(암시적 — 제약이 생성한 인덱스)

[index] sqlite_autoindex_results_1 (on results)
(암시적 — 제약이 생성한 인덱스)

[index] sqlite_autoindex_round_confirmations_1 (on round_confirmations)
(암시적 — 제약이 생성한 인덱스)

[index] sqlite_autoindex_students_1 (on students)
(암시적 — 제약이 생성한 인덱스)

[index] sqlite_autoindex_univ_tracks_1 (on univ_tracks)
(암시적 — 제약이 생성한 인덱스)

[index] sqlite_autoindex_universities_1 (on universities)
(암시적 — 제약이 생성한 인덱스)

[table] app_configs (on app_configs)
CREATE TABLE app_configs ( key TEXT PRIMARY KEY, value TEXT NOT NULL );

[table] applications (on applications)
CREATE TABLE applications ( student_id INTEGER NOT NULL REFERENCES students(id), track_id INTEGER NOT NULL REFERENCES univ_tracks(id), round_id INTEGER NOT NULL REFERENCES rounds(id), abandoned INTEGER NOT NULL DEFAULT 0 CHECK(abandoned IN (0, 1)), department_name TEXT NOT NULL DEFAULT '', excluded INTEGER NOT NULL DEFAULT 0 CHECK(excluded IN (0, 1)), excluded_reason TEXT, PRIMARY KEY (student_id, track_id, round_id), CHECK (excluded = 0 OR (excluded_reason IS NOT NULL AND TRIM(excluded_reason) <> '')) );

[table] areas (on areas)
CREATE TABLE areas ( id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, max_score INTEGER NOT NULL CHECK(max_score >= 0), calc_type TEXT NOT NULL CHECK(calc_type IN ('NUMERIC', 'CATEGORY', 'MANUAL')), teacher_editable INTEGER NOT NULL DEFAULT 1 CHECK(teacher_editable IN (0, 1)), lookup_scope TEXT NOT NULL DEFAULT 'SIMPLE' CHECK(lookup_scope IN ('SIMPLE', 'COMPOSITE')), match_mode TEXT CHECK(match_mode IN ('UPPER', 'LOWER', 'EXACT')), category_agg TEXT CHECK(category_agg IN ('SUM', 'MAX')), multi_value INTEGER NOT NULL DEFAULT 0 CHECK(multi_value IN (0, 1)), unit TEXT, CHECK(calc_type = 'CATEGORY' OR multi_value = 0), CHECK(calc_type = 'NUMERIC' OR match_mode IS NULL), CHECK(calc_type = 'CATEGORY' OR category_agg IS NULL), CHECK(calc_type != 'CATEGORY' OR unit IS NULL) );

[table] audit_log (on audit_log)
CREATE TABLE audit_log ( id INTEGER PRIMARY KEY AUTOINCREMENT, at TEXT NOT NULL, actor_type TEXT NOT NULL CHECK(actor_type IN ('ADMIN', 'TEACHER')), actor_grade INTEGER, actor_class_no INTEGER, actor_name TEXT, actor_ip TEXT, action TEXT NOT NULL, round_id INTEGER, student_id INTEGER, detail TEXT NOT NULL DEFAULT '{}', CHECK ( (actor_type = 'ADMIN' AND actor_grade IS NULL AND actor_class_no IS NULL) OR (actor_type = 'TEACHER' AND actor_grade IS NOT NULL AND actor_class_no IS NOT NULL) ) );

[table] base_data (on base_data)
CREATE TABLE base_data ( id INTEGER PRIMARY KEY AUTOINCREMENT, student_id INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE, area_id INTEGER NOT NULL REFERENCES areas(id) ON DELETE CASCADE, track_id INTEGER REFERENCES univ_tracks(id) ON DELETE CASCADE, value TEXT NOT NULL, multi_value INTEGER NOT NULL DEFAULT 0 CHECK(multi_value IN (0, 1)) );

[table] category_map (on category_map)
CREATE TABLE category_map ( area_id INTEGER NOT NULL REFERENCES areas(id) ON DELETE CASCADE, track_id INTEGER REFERENCES univ_tracks(id) ON DELETE CASCADE, category TEXT NOT NULL, score INTEGER NOT NULL );

[table] classes (on classes)
CREATE TABLE classes ( grade INTEGER NOT NULL, class_no INTEGER NOT NULL, teacher_name TEXT, password_hash TEXT NOT NULL, PRIMARY KEY (grade, class_no) );

[table] numeric_table (on numeric_table)
CREATE TABLE numeric_table ( area_id INTEGER NOT NULL REFERENCES areas(id) ON DELETE CASCADE, track_id INTEGER REFERENCES univ_tracks(id) ON DELETE CASCADE, threshold INTEGER NOT NULL, score INTEGER NOT NULL );

[table] results (on results)
CREATE TABLE results ( student_id INTEGER NOT NULL, track_id INTEGER NOT NULL, round_id INTEGER NOT NULL, score_detail TEXT NOT NULL DEFAULT '{}', total_score INTEGER NOT NULL DEFAULT 0, ranking INTEGER, recommended INTEGER NOT NULL DEFAULT 0 CHECK(recommended IN (0, 1)), calculated_at TEXT NOT NULL, PRIMARY KEY (student_id, track_id, round_id), FOREIGN KEY (student_id, track_id, round_id) REFERENCES applications(student_id, track_id, round_id) );

[table] round_confirmations (on round_confirmations)
CREATE TABLE round_confirmations ( round_id INTEGER NOT NULL, grade INTEGER NOT NULL, class_no INTEGER NOT NULL, confirmed_at TEXT NOT NULL, PRIMARY KEY (round_id, grade, class_no), FOREIGN KEY (round_id) REFERENCES rounds(id) ON DELETE CASCADE, FOREIGN KEY (grade, class_no) REFERENCES classes(grade, class_no) ON DELETE CASCADE );

[table] rounds (on rounds)
CREATE TABLE rounds ( id INTEGER PRIMARY KEY AUTOINCREMENT, status TEXT NOT NULL CHECK(status IN ('OPEN', 'CLOSED', 'FINALIZED')), opened_at TEXT NOT NULL, closed_at TEXT, finalized_at TEXT );

[table] sqlite_sequence (on sqlite_sequence)
CREATE TABLE sqlite_sequence(name,seq);

[table] students (on students)
CREATE TABLE students ( id INTEGER PRIMARY KEY AUTOINCREMENT, student_code TEXT NOT NULL UNIQUE, name TEXT NOT NULL, grade INTEGER, class_no INTEGER, seq_no INTEGER, is_enrolled INTEGER NOT NULL DEFAULT 1 CHECK(is_enrolled IN (0, 1)), grad_year INTEGER, FOREIGN KEY (grade, class_no) REFERENCES classes(grade, class_no), CHECK ( (is_enrolled = 1 AND grade IS NOT NULL AND class_no IS NOT NULL AND seq_no IS NOT NULL AND grad_year IS NULL) OR (is_enrolled = 0 AND grade IS NULL AND class_no IS NULL AND seq_no IS NULL AND grad_year IS NOT NULL) ) );

[table] univ_tracks (on univ_tracks)
CREATE TABLE univ_tracks ( id INTEGER PRIMARY KEY AUTOINCREMENT, univ_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE, track_name TEXT NOT NULL, unit_quota INTEGER, prioritize_enrolled INTEGER NOT NULL DEFAULT 0 CHECK(prioritize_enrolled IN (0, 1)), UNIQUE (univ_id, track_name) );

[table] universities (on universities)
CREATE TABLE universities ( id INTEGER PRIMARY KEY AUTOINCREMENT, univ_name TEXT NOT NULL UNIQUE, total_quota INTEGER, prioritize_enrolled INTEGER NOT NULL DEFAULT 0 CHECK(prioritize_enrolled IN (0, 1)) );

[trigger] trg_prevent_base_data_delete_for_applied (on base_data)
CREATE TRIGGER trg_prevent_base_data_delete_for_applied BEFORE DELETE ON base_data BEGIN SELECT RAISE(ABORT, 'Cannot delete base_data: student has application in CLOSED round') WHERE EXISTS ( SELECT 1 FROM applications ap JOIN rounds r ON r.id = ap.round_id WHERE ap.student_id = OLD.student_id AND r.status = 'CLOSED' ); END;

[trigger] trg_prevent_delete_audit_log (on audit_log)
CREATE TRIGGER trg_prevent_delete_audit_log BEFORE DELETE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is immutable'); END;

[trigger] trg_prevent_delete_closed_application (on applications)
CREATE TRIGGER trg_prevent_delete_closed_application BEFORE DELETE ON applications BEGIN SELECT RAISE(ABORT, 'Cannot delete application: round is CLOSED or FINALIZED') WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) IN ('CLOSED', 'FINALIZED'); END;

[trigger] trg_prevent_delete_closed_result (on results)
CREATE TRIGGER trg_prevent_delete_closed_result BEFORE DELETE ON results BEGIN SELECT RAISE(ABORT, 'Cannot delete result: round is CLOSED or FINALIZED') WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) IN ('CLOSED', 'FINALIZED'); END;

[trigger] trg_prevent_exclude_recommended (on applications)
CREATE TRIGGER trg_prevent_exclude_recommended BEFORE UPDATE ON applications WHEN OLD.excluded = 0 AND NEW.excluded = 1 BEGIN SELECT RAISE(ABORT, 'Cannot exclude application: already recommended') WHERE EXISTS ( SELECT 1 FROM results r WHERE r.student_id = NEW.student_id AND r.track_id = NEW.track_id AND r.round_id = NEW.round_id AND r.recommended = 1 ); END;

[trigger] trg_prevent_update_audit_log (on audit_log)
CREATE TRIGGER trg_prevent_update_audit_log BEFORE UPDATE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is immutable'); END;

[trigger] trg_prevent_update_closed_application (on applications)
CREATE TRIGGER trg_prevent_update_closed_application BEFORE UPDATE ON applications BEGIN SELECT RAISE(ABORT, 'Cannot update application: round is CLOSED. Only excluded/excluded_reason may change.') WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'CLOSED' AND ( OLD.student_id != NEW.student_id OR OLD.track_id != NEW.track_id OR OLD.round_id != NEW.round_id OR OLD.department_name != NEW.department_name OR OLD.abandoned != NEW.abandoned ); SELECT RAISE(ABORT, 'Cannot update application: round is FINALIZED. Only abandoned 0->1 is permitted.') WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'FINALIZED' AND ( OLD.student_id != NEW.student_id OR OLD.track_id != NEW.track_id OR OLD.round_id != NEW.round_id OR OLD.department_name != NEW.department_name OR (OLD.abandoned = 1 AND NEW.abandoned = 0) OR OLD.excluded != NEW.excluded OR OLD.excluded_reason IS NOT NEW.excluded_reason ); END;

[trigger] trg_prevent_update_finalized_result (on results)
CREATE TRIGGER trg_prevent_update_finalized_result BEFORE UPDATE ON results BEGIN SELECT RAISE(ABORT, 'Cannot update result: round is FINALIZED') WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'FINALIZED'; END;

[trigger] trg_require_all_decided_before_finalize (on rounds)
CREATE TRIGGER trg_require_all_decided_before_finalize BEFORE UPDATE ON rounds WHEN OLD.status = 'CLOSED' AND NEW.status = 'FINALIZED' BEGIN SELECT RAISE(ABORT, 'Cannot finalize round: undecided applications remain') WHERE EXISTS ( SELECT 1 FROM applications a LEFT JOIN results r ON r.student_id = a.student_id AND r.track_id = a.track_id AND r.round_id = a.round_id WHERE a.round_id = OLD.id AND a.excluded = 0 AND COALESCE(r.recommended, 0) = 0 ); END;

[trigger] trg_track_prioritize_insert_guard (on univ_tracks)
CREATE TRIGGER trg_track_prioritize_insert_guard BEFORE INSERT ON univ_tracks WHEN NEW.prioritize_enrolled = 0 AND (SELECT prioritize_enrolled FROM universities WHERE id = NEW.univ_id) = 1 BEGIN SELECT RAISE(ABORT, 'univ prioritize=1 requires track prioritize=1'); END;

[trigger] trg_track_prioritize_update_guard (on univ_tracks)
CREATE TRIGGER trg_track_prioritize_update_guard BEFORE UPDATE OF prioritize_enrolled ON univ_tracks WHEN NEW.prioritize_enrolled = 0 AND (SELECT prioritize_enrolled FROM universities WHERE id = NEW.univ_id) = 1 BEGIN SELECT RAISE(ABORT, 'univ prioritize=1 requires track prioritize=1'); END;

[trigger] trg_univ_prioritize_cascade (on universities)
CREATE TRIGGER trg_univ_prioritize_cascade AFTER UPDATE OF prioritize_enrolled ON universities WHEN NEW.prioritize_enrolled <> OLD.prioritize_enrolled BEGIN UPDATE univ_tracks SET prioritize_enrolled = NEW.prioritize_enrolled WHERE univ_id = NEW.id; END;
