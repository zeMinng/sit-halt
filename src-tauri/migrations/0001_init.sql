-- 001_init.sql
-- SitHalt 初始表结构（对应 PRD 第八章本地数据模型）
-- 约定：时间使用带时区的 ISO 8601 文本（UTC），时长统一为整数秒，主键使用 UUID 文本。

CREATE TABLE IF NOT EXISTS app_settings (
    key         TEXT PRIMARY KEY NOT NULL,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS reminder_plans (
    id                      TEXT PRIMARY KEY NOT NULL,
    name                    TEXT NOT NULL,
    description             TEXT,
    mode                    TEXT NOT NULL DEFAULT 'interval',
    work_seconds            INTEGER NOT NULL,
    break_seconds           INTEGER NOT NULL,
    reminder_methods        TEXT NOT NULL DEFAULT '["notification"]',
    repeat_interval_seconds INTEGER NOT NULL DEFAULT 300,
    repeat_times            INTEGER NOT NULL DEFAULT 2,
    snooze_seconds          INTEGER NOT NULL DEFAULT 300,
    completion_mode         TEXT NOT NULL DEFAULT 'auto',
    idle_detection          INTEGER NOT NULL DEFAULT 1,
    idle_threshold_seconds  INTEGER NOT NULL DEFAULT 180,
    work_start              TEXT,
    work_end                TEXT,
    work_days               TEXT NOT NULL DEFAULT '[1,2,3,4,5]',
    smart_enabled           INTEGER NOT NULL DEFAULT 0,
    is_builtin              INTEGER NOT NULL DEFAULT 0,
    sort_order              INTEGER NOT NULL DEFAULT 0,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reminder_plans_sort ON reminder_plans (sort_order);

CREATE TABLE IF NOT EXISTS work_schedules (
    id           TEXT PRIMARY KEY NOT NULL,
    plan_id      TEXT NOT NULL,
    weekday      INTEGER NOT NULL,
    start_minute INTEGER NOT NULL,
    end_minute   INTEGER NOT NULL,
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL,
    FOREIGN KEY (plan_id) REFERENCES reminder_plans (id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_work_schedules_plan ON work_schedules (plan_id, weekday);

CREATE TABLE IF NOT EXISTS activities (
    id               TEXT PRIMARY KEY NOT NULL,
    name             TEXT NOT NULL,
    category         TEXT NOT NULL,
    description      TEXT,
    duration_seconds INTEGER NOT NULL,
    steps            TEXT NOT NULL DEFAULT '[]',
    image            TEXT,
    cautions         TEXT,
    enabled          INTEGER NOT NULL DEFAULT 1,
    is_builtin       INTEGER NOT NULL DEFAULT 0,
    sort_order       INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_activities_category ON activities (category, sort_order);

CREATE TABLE IF NOT EXISTS activity_sessions (
    id               TEXT PRIMARY KEY NOT NULL,
    activity_id      TEXT,
    activity_name    TEXT NOT NULL,
    category         TEXT,
    plan_id          TEXT,
    started_at       TEXT NOT NULL,
    ended_at         TEXT,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    planned_seconds  INTEGER NOT NULL DEFAULT 0,
    completion       TEXT NOT NULL DEFAULT 'completed',
    source           TEXT NOT NULL DEFAULT 'reminder',
    created_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_activity_sessions_started ON activity_sessions (started_at);

CREATE TABLE IF NOT EXISTS sitting_sessions (
    id               TEXT PRIMARY KEY NOT NULL,
    plan_id          TEXT,
    started_at       TEXT NOT NULL,
    ended_at         TEXT,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    interrupted_by   TEXT,
    created_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sitting_sessions_started ON sitting_sessions (started_at);

CREATE TABLE IF NOT EXISTS app_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    payload    TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_app_events_created ON app_events (created_at);
CREATE INDEX IF NOT EXISTS idx_app_events_type ON app_events (event_type, created_at);

CREATE TABLE IF NOT EXISTS daily_stats (
    date                   TEXT PRIMARY KEY NOT NULL,
    sitting_seconds        INTEGER NOT NULL DEFAULT 0,
    activity_count         INTEGER NOT NULL DEFAULT 0,
    activity_seconds       INTEGER NOT NULL DEFAULT 0,
    longest_sitting_seconds INTEGER NOT NULL DEFAULT 0,
    expected_activities    INTEGER NOT NULL DEFAULT 0,
    completion_rate        REAL,
    computed_at            TEXT NOT NULL
);
