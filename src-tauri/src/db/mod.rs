use std::path::PathBuf;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use serde_json::json;
use sqlx::{Column, Row};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    Activity, ActivityInput, ActivityRow, ActivitySession, ActivitySessionRow, BackupInfo,
    DailyPoint, DbHealth, ReminderPlan, ReminderPlanInput, ReminderPlanRow, SittingSession,
    TodayStats, WorkSchedule, WorkScheduleInput,
};

pub struct Database {
    pub pool: SqlitePool,
    pub path: PathBuf,
}

/// 待恢复的库文件（放在数据库同级，启动时检测到即完成替换）。
const PENDING_RESTORE_SUFFIX: &str = "db.restore-pending";

fn open_pool(db_path: &PathBuf) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(StdDuration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(StdDuration::from_secs(10))
        .connect_lazy_with(options);

    Ok(pool)
}

/// 数据库初始化：文件落在系统推荐的应用数据目录，避免升级 / 卸载丢数据（PRD 8.2）。
pub async fn init(app: &AppHandle) -> AppResult<Database> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::State(format!("无法获取应用数据目录：{e}")))?;

    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("sithalt.db");

    // 启动时先把上次待恢复的备份落位，再正常建池迁移。
    apply_pending_restore(&db_path).await?;

    let pool = open_pool(&db_path)?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    seed_if_empty(&pool).await?;

    Ok(Database {
        pool,
        path: db_path,
    })
}

pub async fn health(pool: &SqlitePool, db_path: &PathBuf) -> AppResult<DbHealth> {
    let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await?;
    let table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_one(pool)
    .await?;

    Ok(DbHealth {
        ok: true,
        db_path: db_path.display().to_string(),
        migration_count,
        table_count,
        server_time: Utc::now().to_rfc3339(),
    })
}

// ---------------------------------------------------------------- 时间工具

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// 本地某一天的 [start, end) 对应的 UTC ISO 字符串区间。
fn local_day_range(date: NaiveDate) -> AppResult<(String, String)> {
    let start_local = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Invalid(format!("无法解析日期 {date}")))?;
    let start = Local
        .from_local_datetime(&start_local)
        .single()
        .ok_or_else(|| AppError::Invalid("本地时间转换失败".into()))?;
    let start_utc: DateTime<Utc> = start.with_timezone(&Utc);
    let end_utc = start_utc + Duration::days(1);
    Ok((start_utc.to_rfc3339(), end_utc.to_rfc3339()))
}

// ---------------------------------------------------------------- 初始化数据

async fn seed_if_empty(pool: &SqlitePool) -> AppResult<()> {
    let plan_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reminder_plans")
        .fetch_one(pool)
        .await?;
    if plan_count == 0 {
        let presets = [
            (
                "轻松模式",
                "25 分钟工作 + 5 分钟活动，适合阅读与轻量任务",
                25 * 60,
                5 * 60,
                0,
            ),
            (
                "标准模式",
                "30 分钟工作 + 5 分钟活动，日常办公默认",
                30 * 60,
                5 * 60,
                1,
            ),
            (
                "办公模式",
                "45 分钟工作 + 5 分钟活动，适合连续处理事务",
                45 * 60,
                5 * 60,
                2,
            ),
            (
                "健康模式",
                "50 分钟工作 + 10 分钟活动，活动更充分",
                50 * 60,
                10 * 60,
                3,
            ),
            (
                "专注模式",
                "60 分钟工作 + 10 分钟活动，适合深度编码",
                60 * 60,
                10 * 60,
                4,
            ),
        ];
        for (name, desc, work, brk, order) in presets {
            let id = Uuid::new_v4().to_string();
            let now = now_iso();
            sqlx::query(
                r#"INSERT INTO reminder_plans
                   (id, name, description, mode, work_seconds, break_seconds, reminder_methods,
                    repeat_interval_seconds, repeat_times, snooze_seconds, completion_mode,
                    idle_detection, idle_threshold_seconds, work_start, work_end, work_days,
                    smart_enabled, is_builtin, sort_order, created_at, updated_at)
                   VALUES (?, ?, ?, 'pomodoro', ?, ?, ?, 300, 2, 300, 'auto', 1, 180, NULL, NULL, ?, 0, 1, ?, ?, ?)"#,
            )
            .bind(&id)
            .bind(name)
            .bind(desc)
            .bind(work as i64)
            .bind(brk as i64)
            .bind(json!(["notification", "popup", "sound"]).to_string())
            .bind(json!([1, 2, 3, 4, 5]).to_string())
            .bind(order)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        }
    }

    let activity_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM activities")
        .fetch_one(pool)
        .await?;
    if activity_count == 0 {
        let builtin: [(&str, &str, &str, i64, Vec<&str>, &str, i64); 10] = [
            (
                "起身行走",
                "身体活动",
                "离开座位走动一会儿，让下肢血液循环恢复正常。",
                180,
                vec![
                    "从座位站起，双脚踩实地面",
                    "在室内或走廊匀速走动",
                    "保持自然呼吸，不要低头看手机",
                ],
                "如有膝踝不适，改为原地站立或小幅踏步。",
                0,
            ),
            (
                "原地深蹲",
                "身体活动",
                "用自重深蹲激活臀腿肌群，抵消久坐带来的髋部紧张。",
                120,
                vec![
                    "双脚与肩同宽站立",
                    "臀部向后下方坐，膝盖不内扣",
                    "下蹲到舒适深度后缓慢起身",
                ],
                "膝关节有伤时不要做深蹲，可改为坐立起身。",
                1,
            ),
            (
                "肩部环绕",
                "肩颈放松",
                "缓解长时间保持固定姿势造成的肩部僵硬。",
                120,
                vec!["双肩放松下沉", "缓慢向前画大圈 10 次", "再向后画大圈 10 次"],
                "动作放慢，出现疼痛立即停止。",
                2,
            ),
            (
                "颈部拉伸",
                "肩颈放松",
                "放松颈后与斜方肌，减轻低头带来的牵拉。",
                120,
                vec![
                    "头部缓慢向左侧倾斜，左耳靠近左肩，保持 20 秒",
                    "回到中立位，换右侧重复",
                    "最后收下巴做 10 次颈部回缩",
                ],
                "不要做绕颈旋转，力度以轻微牵拉感为宜。",
                3,
            ),
            (
                "腰部伸展",
                "腰背放松",
                "站立位伸展腰背，缓解久坐导致的腰部压缩感。",
                120,
                vec![
                    "站直，双手举过头顶",
                    "向左侧轻微侧弯，保持 15 秒",
                    "回到中立位后换右侧重复",
                ],
                "腰部急性疼痛期不做侧弯。",
                4,
            ),
            (
                "背部拉伸",
                "腰背放松",
                "打开胸椎与肩胛，改善含胸驼背。",
                120,
                vec![
                    "双手在身前交叉，掌心向外",
                    "手臂向前推，背部微微拱起",
                    "保持 20 秒后放松",
                ],
                "肩部有伤时降低幅度。",
                5,
            ),
            (
                "远眺放松",
                "眼部放松",
                "把视线从近距离屏幕切换到远处，缓解调节紧张。",
                120,
                vec!["走到窗边", "视线投向 6 米以外的物体", "保持 20 秒不看屏幕"],
                "保持室内光线充足。",
                6,
            ),
            (
                "闭眼休息",
                "眼部放松",
                "完全闭眼，减少泪液蒸发与睫状肌负担。",
                120,
                vec!["摘掉眼镜或放松镜架", "轻闭双眼", "自然呼吸，不揉眼"],
                "不要趴着压迫眼球。",
                7,
            ),
            (
                "喝水",
                "健康习惯",
                "补充水分，同时完成一次短暂离座。",
                60,
                vec![
                    "起身到饮水处接水",
                    "小口喝完一杯（约 200ml）",
                    "顺手活动一下手腕",
                ],
                "睡前 1 小时避免大量饮水。",
                8,
            ),
            (
                "深呼吸",
                "健康习惯",
                "用缓慢呼吸降低久坐时的焦虑与肌肉紧张。",
                60,
                vec!["吸气 4 秒", "屏息 2 秒", "呼气 6 秒，重复 6 轮"],
                "出现头晕时恢复自然呼吸。",
                9,
            ),
        ];

        for (name, category, desc, duration, steps, cautions, order) in builtin {
            let id = format!("builtin-{}", order);
            let now = now_iso();
            sqlx::query(
                r#"INSERT INTO activities
                   (id, name, category, description, duration_seconds, steps, image, cautions,
                    enabled, is_builtin, sort_order, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, NULL, ?, 1, 1, ?, ?, ?)"#,
            )
            .bind(&id)
            .bind(name)
            .bind(category)
            .bind(desc)
            .bind(duration)
            .bind(json!(steps).to_string())
            .bind(cautions)
            .bind(order)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        }
    }

    // 默认设置项：只补齐缺失的键，不覆盖用户已有配置。
    let defaults = [
        ("theme", json!("system")),
        ("language", json!("zh-CN")),
        ("closeToTray", json!(true)),
        ("autostart", json!(false)),
        ("showOnStartup", json!(false)),
        ("notificationEnabled", json!(true)),
        ("soundEnabled", json!(true)),
        ("soundVolume", json!(0.7)),
        ("floatWidget", json!(false)),
        ("backupIntervalDays", json!(7)),
        ("backupKeep", json!(5)),
        ("activePlanId", json!(null)),
        (
            "shortcuts",
            json!({
                "toggleMain": "Ctrl+Alt+S",
                "pauseResume": "Ctrl+Alt+P",
                "startBreak": "Ctrl+Alt+B",
                "dnd": "Ctrl+Alt+D"
            })
        ),
    ];
    for (key, value) in defaults {
        let exists: Option<String> =
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?")
                .bind(key)
                .fetch_optional(pool)
                .await?;
        if exists.is_none() {
            sqlx::query("INSERT INTO app_settings (key, value, updated_at) VALUES (?, ?, ?)")
                .bind(key)
                .bind(value.to_string())
                .bind(now_iso())
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------- 记录事件

pub async fn record_event(pool: &SqlitePool, event_type: &str, payload: serde_json::Value) {
    let _ =
        sqlx::query("INSERT INTO app_events (event_type, payload, created_at) VALUES (?, ?, ?)")
            .bind(event_type)
            .bind(payload.to_string())
            .bind(now_iso())
            .execute(pool)
            .await;
}

// ---------------------------------------------------------------- 提醒计划

pub async fn list_plans(pool: &SqlitePool) -> AppResult<Vec<ReminderPlan>> {
    let rows = sqlx::query_as::<_, ReminderPlanRow>(
        "SELECT * FROM reminder_plans ORDER BY sort_order ASC, created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(ReminderPlanRow::into_dto).collect())
}

pub async fn get_plan(pool: &SqlitePool, id: &str) -> AppResult<ReminderPlan> {
    let row = sqlx::query_as::<_, ReminderPlanRow>("SELECT * FROM reminder_plans WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Invalid(format!("提醒计划不存在：{id}")))?;
    Ok(row.into_dto())
}

pub async fn default_plan(pool: &SqlitePool) -> AppResult<Option<ReminderPlan>> {
    let active: Option<String> =
        sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'activePlanId'")
            .fetch_optional(pool)
            .await?;
    if let Some(raw) = active {
        if let Ok(Some(id)) = serde_json::from_str::<Option<String>>(&raw) {
            if let Ok(plan) = get_plan(pool, &id).await {
                return Ok(Some(plan));
            }
        }
    }
    let rows = sqlx::query_as::<_, ReminderPlanRow>(
        "SELECT * FROM reminder_plans ORDER BY sort_order ASC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(rows.map(ReminderPlanRow::into_dto))
}

pub async fn save_plan(pool: &SqlitePool, input: ReminderPlanInput) -> AppResult<ReminderPlan> {
    if input.name.trim().is_empty() {
        return Err(AppError::Invalid("计划名称不能为空".into()));
    }
    if input.work_seconds < 60 {
        return Err(AppError::Invalid("工作时长至少 1 分钟".into()));
    }
    if input.break_seconds < 30 {
        return Err(AppError::Invalid("活动时长至少 30 秒".into()));
    }

    let now = now_iso();
    let id = match input.id.clone() {
        Some(id) if !id.trim().is_empty() => {
            // 仅允许修改自定义计划的时长等配置，内置计划同样可改（PRD 4.3.1）。
            let exists: Option<String> =
                sqlx::query_scalar("SELECT id FROM reminder_plans WHERE id = ?")
                    .bind(&id)
                    .fetch_optional(pool)
                    .await?;
            if exists.is_none() {
                return Err(AppError::Invalid(format!("提醒计划不存在：{id}")));
            }
            id
        }
        _ => Uuid::new_v4().to_string(),
    };

    sqlx::query(
        r#"INSERT INTO reminder_plans
           (id, name, description, mode, work_seconds, break_seconds, reminder_methods,
            repeat_interval_seconds, repeat_times, snooze_seconds, completion_mode,
            idle_detection, idle_threshold_seconds, work_start, work_end, work_days,
            smart_enabled, is_builtin, sort_order, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET
             name = excluded.name,
             description = excluded.description,
             mode = excluded.mode,
             work_seconds = excluded.work_seconds,
             break_seconds = excluded.break_seconds,
             reminder_methods = excluded.reminder_methods,
             repeat_interval_seconds = excluded.repeat_interval_seconds,
             repeat_times = excluded.repeat_times,
             snooze_seconds = excluded.snooze_seconds,
             completion_mode = excluded.completion_mode,
             idle_detection = excluded.idle_detection,
             idle_threshold_seconds = excluded.idle_threshold_seconds,
             work_start = excluded.work_start,
             work_end = excluded.work_end,
             work_days = excluded.work_days,
             smart_enabled = excluded.smart_enabled,
             sort_order = excluded.sort_order,
             updated_at = excluded.updated_at"#,
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.description)
    .bind(&input.mode)
    .bind(input.work_seconds)
    .bind(input.break_seconds)
    .bind(json!(input.reminder_methods).to_string())
    .bind(input.repeat_interval_seconds)
    .bind(input.repeat_times)
    .bind(input.snooze_seconds)
    .bind(&input.completion_mode)
    .bind(input.idle_detection)
    .bind(input.idle_threshold_seconds)
    .bind(&input.work_start)
    .bind(&input.work_end)
    .bind(json!(input.work_days).to_string())
    .bind(input.smart_enabled)
    .bind(input.sort_order)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    record_event(pool, "plan_saved", json!({ "planId": id })).await;
    get_plan(pool, &id).await
}

pub async fn delete_plan(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let is_builtin: Option<bool> =
        sqlx::query_scalar("SELECT is_builtin FROM reminder_plans WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    match is_builtin {
        None => return Err(AppError::Invalid(format!("提醒计划不存在：{id}"))),
        Some(true) => return Err(AppError::Invalid("内置计划不可删除，可以修改其参数".into())),
        Some(false) => {}
    }

    sqlx::query("DELETE FROM reminder_plans WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    // 若被删的正是当前启用计划，清空启用标记，避免下次启动回退到不存在的计划
    let active: Option<String> =
        sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'activePlanId'")
            .fetch_optional(pool)
            .await?;
    if let Some(raw) = active {
        if let Ok(Some(aid)) = serde_json::from_str::<Option<String>>(&raw) {
            if aid == id {
                set_setting(pool, "activePlanId", json!(null)).await?;
            }
        }
    }

    record_event(pool, "plan_deleted", json!({ "planId": id })).await;
    Ok(())
}

pub async fn set_active_plan(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM reminder_plans WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::Invalid(format!("提醒计划不存在：{id}")));
    }
    set_setting(pool, "activePlanId", json!(id)).await
}

// ---------------------------------------------------------------- 工作时间表

/// 读取工作时间表。传 `Some(plan_id)` 只取该计划的时段，传 `None` 取全部。
pub async fn list_work_schedules(
    pool: &SqlitePool,
    plan_id: Option<&str>,
) -> AppResult<Vec<WorkSchedule>> {
    match plan_id {
        Some(id) => Ok(sqlx::query_as::<_, WorkSchedule>(
            "SELECT * FROM work_schedules WHERE plan_id = ? ORDER BY weekday ASC, start_minute ASC",
        )
        .bind(id)
        .fetch_all(pool)
        .await?),
        None => Ok(sqlx::query_as::<_, WorkSchedule>(
            "SELECT * FROM work_schedules ORDER BY plan_id ASC, weekday ASC, start_minute ASC",
        )
        .fetch_all(pool)
        .await?),
    }
}

/// 把 [start, end) 时段展开到 1440 分钟；end < start 按跨零点处理。
fn fills_minutes(start: i64, end: i64) -> [bool; 1440] {
    let mut minute = [false; 1440];
    if start <= end {
        for t in start..end {
            minute[t as usize] = true;
        }
    } else {
        for t in start..1440 {
            minute[t as usize] = true;
        }
        for t in 0..end {
            minute[t as usize] = true;
        }
    }
    minute
}

/// 两个时段是否在任意一分钟上重叠（含跨零点）。
fn intervals_overlap(a: (i64, i64), b: (i64, i64)) -> bool {
    let am = fills_minutes(a.0, a.1);
    let bm = fills_minutes(b.0, b.1);
    (0..1440).any(|t| am[t] && bm[t])
}

fn fmt_minute(m: i64) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

pub async fn save_work_schedule(
    pool: &SqlitePool,
    input: WorkScheduleInput,
) -> AppResult<WorkSchedule> {
    if !(1..=7).contains(&input.weekday) {
        return Err(AppError::Invalid(
            "工作日必须是 1（周一）到 7（周日）".into(),
        ));
    }
    if !(0..1440).contains(&input.start_minute) || !(0..1440).contains(&input.end_minute) {
        return Err(AppError::Invalid("时间必须落在 00:00 到 23:59 之间".into()));
    }
    if input.start_minute == input.end_minute {
        return Err(AppError::Invalid("开始时间与结束时间不能相同".into()));
    }

    let plan_exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM reminder_plans WHERE id = ?")
            .bind(&input.plan_id)
            .fetch_optional(pool)
            .await?;
    if plan_exists.is_none() {
        return Err(AppError::Invalid(format!(
            "提醒计划不存在：{}",
            input.plan_id
        )));
    }

    let id = match input.id.clone() {
        Some(id) if !id.trim().is_empty() => id,
        _ => Uuid::new_v4().to_string(),
    };

    // 同计划、同工作日的其他已启用时段不得与本次时段重叠（跨零点一并纳入比较）。
    if input.enabled {
        let others: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT id, start_minute, end_minute FROM work_schedules
             WHERE plan_id = ? AND weekday = ? AND enabled = 1 AND id <> ?",
        )
        .bind(&input.plan_id)
        .bind(input.weekday)
        .bind(&id)
        .fetch_all(pool)
        .await?;
        for (_oid, os, oe) in others {
            if intervals_overlap(
                (input.start_minute, input.end_minute),
                (os, oe),
            ) {
                return Err(AppError::Invalid(format!(
                    "该工作日已存在重叠时段 {}–{}，请先调整或禁用",
                    fmt_minute(os),
                    fmt_minute(oe)
                )));
            }
        }
    }

    let now = now_iso();
    sqlx::query(
        r#"INSERT INTO work_schedules (id, plan_id, weekday, start_minute, end_minute, enabled, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET
             plan_id = excluded.plan_id,
             weekday = excluded.weekday,
             start_minute = excluded.start_minute,
             end_minute = excluded.end_minute,
             enabled = excluded.enabled"#,
    )
    .bind(&id)
    .bind(&input.plan_id)
    .bind(input.weekday)
    .bind(input.start_minute)
    .bind(input.end_minute)
    .bind(input.enabled)
    .bind(&now)
    .execute(pool)
    .await?;

    record_event(pool, "work_schedule_saved", json!({ "scheduleId": id })).await;
    get_work_schedule(pool, &id).await
}

pub async fn get_work_schedule(pool: &SqlitePool, id: &str) -> AppResult<WorkSchedule> {
    sqlx::query_as::<_, WorkSchedule>("SELECT * FROM work_schedules WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Invalid(format!("工作时间段不存在：{id}")))
}

pub async fn delete_work_schedule(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM work_schedules WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::Invalid(format!("工作时间段不存在：{id}")));
    }
    sqlx::query("DELETE FROM work_schedules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    record_event(pool, "work_schedule_deleted", json!({ "scheduleId": id })).await;
    Ok(())
}

// ---------------------------------------------------------------- 备份

fn backup_dir(db_path: &std::path::Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("backups")
}

/// 用 `VACUUM INTO` 生成一份一致性快照（比直接复制 -wal 状态的库文件更安全）。
pub async fn backup_now(
    pool: &SqlitePool,
    db_path: &PathBuf,
    keep: usize,
) -> AppResult<BackupInfo> {
    let dir = backup_dir(db_path);
    std::fs::create_dir_all(&dir)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let target = dir.join(format!("sithalt-{stamp}.db"));

    // VACUUM INTO 的路径用单引号包裹，路径里的单引号需要转义。
    let escaped = target.display().to_string().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{escaped}'"))
        .execute(pool)
        .await?;

    prune_backups(&dir, keep);
    let meta = std::fs::metadata(&target)?;
    Ok(BackupInfo {
        file: target.display().to_string(),
        size_bytes: meta.len() as i64,
        created_at: now_iso(),
    })
}

fn prune_backups(dir: &std::path::Path, keep: usize) {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(iter) => iter
            .flatten()
            .map(|entry| entry.path())
            .filter(|p| p.extension().map(|e| e == "db").unwrap_or(false))
            .collect(),
        Err(_) => return,
    };
    if files.len() <= keep {
        return;
    }
    files.sort();
    let remove_count = files.len() - keep;
    for path in files.into_iter().take(remove_count) {
        let _ = std::fs::remove_file(path);
    }
}

pub fn list_backups(db_path: &PathBuf) -> AppResult<Vec<BackupInfo>> {
    let dir = backup_dir(db_path);
    let mut items = Vec::new();
    if let Ok(iter) = std::fs::read_dir(&dir) {
        for entry in iter.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e != "db").unwrap_or(true) {
                continue;
            }
            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            let created_at = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<Local>::from(t).to_rfc3339())
                .unwrap_or_else(now_iso);
            items.push(BackupInfo {
                file: path.display().to_string(),
                size_bytes: meta.len() as i64,
                created_at,
            });
        }
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

fn pending_path(db_path: &PathBuf) -> PathBuf {
    let stem = db_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "sithalt.db".into());
    db_path.with_file_name(format!("{stem}.{PENDING_RESTORE_SUFFIX}"))
}

/// 校验备份文件是可识别的 SitHalt 数据库：可只读打开、含迁移记录与核心数据表。
pub async fn validate_backup_file(path: &PathBuf) -> AppResult<()> {
    if !path.exists() {
        return Err(AppError::Invalid(format!("备份文件不存在：{}", path.display())));
    }
    if path.extension().map(|e| e != "db").unwrap_or(true) {
        return Err(AppError::Invalid("备份文件必须是 .db 文件".into()));
    }

    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_lazy_with(options);

    let migration_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await?;
    if migration_count == 0 {
        pool.close().await;
        return Err(AppError::Invalid(
            "备份文件缺少迁移记录，不是有效的 SitHalt 数据库".into(),
        ));
    }

    for table in [
        "reminder_plans",
        "activities",
        "activity_sessions",
        "sitting_sessions",
    ] {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_optional(&pool)
        .await?;
        if exists.is_none() {
            pool.close().await;
            return Err(AppError::Invalid(format!("备份文件缺少核心数据表：{table}")));
        }
    }
    pool.close().await;
    Ok(())
}

/// 恢复前准备：校验备份 → 兜底备份当前库 → 放入待恢复文件。
/// 运行中的连接池仍占用当前库，真正的文件替换留到下次启动（PRD 4.9）。
pub async fn prepare_restore(
    pool: &SqlitePool,
    db_path: &PathBuf,
    backup_file: &str,
) -> AppResult<BackupInfo> {
    let candidate = PathBuf::from(backup_file);
    let backup_dir = std::fs::canonicalize(backup_dir(db_path))
        .unwrap_or_else(|_| backup_dir(db_path));
    let canonical_file = std::fs::canonicalize(&candidate)
        .map_err(|_| AppError::Invalid(format!("备份文件不存在：{backup_file}")))?;
    if !canonical_file.starts_with(&backup_dir) {
        return Err(AppError::Invalid("只能恢复应用生成的本地备份".into()));
    }

    validate_backup_file(&canonical_file).await?;

    // 恢复前先兜底备份当前数据，便于回退；兜底是最新快照，不会被清理策略删掉。
    let keep = read_setting_i64(pool, "backupKeep", 5).await.max(1) as usize;
    let fallback = backup_now(pool, db_path, keep).await?;

    let pending = pending_path(db_path);
    std::fs::copy(&canonical_file, &pending)?;

    record_event(pool, "db_restore_prepared", json!({ "backup": backup_file })).await;
    Ok(fallback)
}

/// 启动时检测待恢复文件：校验后用它替换当前库，并清理 WAL / SHM 临时文件。
pub async fn apply_pending_restore(db_path: &PathBuf) -> AppResult<()> {
    let pending = pending_path(db_path);
    if !pending.exists() {
        return Ok(());
    }
    validate_backup_file(&pending).await?;

    let file_name = db_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "sithalt.db".into());
    for suffix in ["", "-wal", "-shm"] {
        let stale = db_path.with_file_name(format!("{file_name}{suffix}"));
        let _ = std::fs::remove_file(stale);
    }

    std::fs::copy(&pending, db_path)?;
    let _ = std::fs::remove_file(&pending);
    Ok(())
}

/// 启动时按设置决定是否需要备份。间隔与保留份数都来自设置项，缺省 7 天 / 5 份。
pub async fn maybe_auto_backup(pool: &SqlitePool, db_path: &PathBuf) {
    let interval_days = read_setting_i64(pool, "backupIntervalDays", 7).await;
    if interval_days <= 0 {
        return;
    }
    let keep = read_setting_i64(pool, "backupKeep", 5).await.max(1) as usize;

    let last = std::fs::read_dir(backup_dir(db_path))
        .ok()
        .and_then(|iter| {
            iter.flatten()
                .filter_map(|entry| entry.metadata().ok()?.modified().ok())
                .max()
        });

    let due = match last {
        None => true,
        Some(t) => {
            let ts = chrono::DateTime::<Local>::from(t).timestamp();
            Utc::now().timestamp() - ts >= interval_days * 86_400
        }
    };
    if !due {
        return;
    }
    match backup_now(pool, db_path, keep).await {
        Ok(info) => {
            let _ = set_setting(pool, "lastBackupAt", json!(info.created_at)).await;
            record_event(pool, "db_backup", json!({ "file": info.file })).await;
        }
        Err(err) => eprintln!("[db] 自动备份失败：{err}"),
    }
}

async fn read_setting_i64(pool: &SqlitePool, key: &str, fallback: i64) -> i64 {
    get_setting_raw(pool, key)
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<i64>(&raw).ok())
        .unwrap_or(fallback)
}

// ---------------------------------------------------------------- 设置

pub async fn get_settings(pool: &SqlitePool) -> AppResult<serde_json::Value> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM app_settings")
        .fetch_all(pool)
        .await?;
    let mut map = serde_json::Map::new();
    for (key, value) in rows {
        map.insert(
            key,
            serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value)),
        );
    }
    Ok(serde_json::Value::Object(map))
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: serde_json::Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value.to_string())
    .bind(now_iso())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_setting_raw(pool: &SqlitePool, key: &str) -> AppResult<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?,
    )
}

// ---------------------------------------------------------------- 活动库

pub async fn list_activities(pool: &SqlitePool) -> AppResult<Vec<Activity>> {
    let rows = sqlx::query_as::<_, ActivityRow>(
        "SELECT * FROM activities ORDER BY sort_order ASC, created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(ActivityRow::into_dto).collect())
}

pub async fn get_activity(pool: &SqlitePool, id: &str) -> AppResult<Activity> {
    let row = sqlx::query_as::<_, ActivityRow>("SELECT * FROM activities WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Invalid(format!("活动不存在：{id}")))?;
    Ok(row.into_dto())
}

pub async fn save_activity(pool: &SqlitePool, input: ActivityInput) -> AppResult<Activity> {
    if input.name.trim().is_empty() {
        return Err(AppError::Invalid("活动名称不能为空".into()));
    }
    if input.duration_seconds < 10 {
        return Err(AppError::Invalid("活动时长至少 10 秒".into()));
    }
    let now = now_iso();
    let id = match input.id.clone() {
        Some(id) if !id.trim().is_empty() => id,
        _ => Uuid::new_v4().to_string(),
    };
    sqlx::query(
        r#"INSERT INTO activities
           (id, name, category, description, duration_seconds, steps, image, cautions,
            enabled, is_builtin, sort_order, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET
             name = excluded.name,
             category = excluded.category,
             description = excluded.description,
             duration_seconds = excluded.duration_seconds,
             steps = excluded.steps,
             image = excluded.image,
             cautions = excluded.cautions,
             enabled = excluded.enabled,
             sort_order = excluded.sort_order,
             updated_at = excluded.updated_at"#,
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.category)
    .bind(&input.description)
    .bind(input.duration_seconds)
    .bind(json!(input.steps).to_string())
    .bind(&input.image)
    .bind(&input.cautions)
    .bind(input.enabled)
    .bind(input.sort_order)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_activity(pool, &id).await
}

pub async fn delete_activity(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let is_builtin: Option<bool> =
        sqlx::query_scalar("SELECT is_builtin FROM activities WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    match is_builtin {
        None => return Err(AppError::Invalid(format!("活动不存在：{id}"))),
        Some(true) => return Err(AppError::Invalid("内置活动不可删除，可以将其禁用".into())),
        Some(false) => {}
    }
    sqlx::query("DELETE FROM activities WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------- 记录写入

/// 开启一段久坐会话，返回会话 id（用于后续更新与收尾）。
pub async fn open_sitting_session(pool: &SqlitePool, plan_id: &str) -> AppResult<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    sqlx::query(
        "INSERT INTO sitting_sessions (id, plan_id, started_at, ended_at, duration_seconds, interrupted_by, created_at)
         VALUES (?, ?, ?, NULL, 0, NULL, ?)",
    )
    .bind(&id)
    .bind(plan_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn update_sitting_session(pool: &SqlitePool, id: &str, seconds: i64) {
    let _ = sqlx::query(
        "UPDATE sitting_sessions SET duration_seconds = ? WHERE id = ? AND ended_at IS NULL",
    )
    .bind(seconds.max(0))
    .bind(id)
    .execute(pool)
    .await;
}

pub async fn close_sitting_session(pool: &SqlitePool, id: &str, seconds: i64, reason: &str) {
    let _ = sqlx::query(
        "UPDATE sitting_sessions SET ended_at = ?, duration_seconds = ?, interrupted_by = ?
         WHERE id = ? AND ended_at IS NULL",
    )
    .bind(now_iso())
    .bind(seconds.max(0))
    .bind(reason)
    .bind(id)
    .execute(pool)
    .await;
}

/// 应用启动时收尾上次未关闭的会话，避免把离线时间算进久坐（PRD 13.2）。
pub async fn close_dangling_sessions(pool: &SqlitePool) {
    // 异常退出时，会话的 duration_seconds 可能还停留在 0（周期写库尚未发生）。
    // 用 breakpoint 里的 workSeconds 兜底，保证久坐时长不被记成 0 秒。
    let fallback = get_setting_raw(pool, "engineCheckpoint")
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("workSeconds").and_then(|w| w.as_i64()))
        .unwrap_or(0);

    let _ = sqlx::query(
        "UPDATE sitting_sessions
         SET ended_at = ?,
             interrupted_by = 'app_restart',
             duration_seconds = CASE
                 WHEN duration_seconds > 0 THEN duration_seconds
                 ELSE ?
             END
         WHERE ended_at IS NULL",
    )
    .bind(now_iso())
    .bind(fallback)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "UPDATE activity_sessions SET ended_at = ?, completion = 'interrupted'
         WHERE ended_at IS NULL",
    )
    .bind(now_iso())
    .execute(pool)
    .await;
}

pub struct NewActivitySession<'a> {
    pub activity_id: Option<&'a str>,
    pub activity_name: &'a str,
    pub category: Option<&'a str>,
    pub plan_id: Option<&'a str>,
    pub started_at: &'a str,
    pub duration_seconds: i64,
    pub planned_seconds: i64,
    pub completion: &'a str,
    pub source: &'a str,
}

pub async fn insert_activity_session(
    pool: &SqlitePool,
    s: NewActivitySession<'_>,
) -> AppResult<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    sqlx::query(
        "INSERT INTO activity_sessions
         (id, activity_id, activity_name, category, plan_id, started_at, ended_at,
          duration_seconds, planned_seconds, completion, source, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(s.activity_id)
    .bind(s.activity_name)
    .bind(s.category)
    .bind(s.plan_id)
    .bind(s.started_at)
    .bind(&now)
    .bind(s.duration_seconds)
    .bind(s.planned_seconds)
    .bind(s.completion)
    .bind(s.source)
    .bind(&now)
    .execute(pool)
    .await?;
    record_event(
        pool,
        "activity_recorded",
        json!({ "sessionId": id, "activityName": s.activity_name, "duration": s.duration_seconds }),
    )
    .await;
    Ok(id)
}

// ---------------------------------------------------------------- 统计

pub async fn recent_activity_sessions(
    pool: &SqlitePool,
    limit: i64,
) -> AppResult<Vec<ActivitySession>> {
    let rows = sqlx::query_as::<_, ActivitySessionRow>(
        "SELECT * FROM activity_sessions ORDER BY started_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(ActivitySession::from).collect())
}

pub async fn today_stats(pool: &SqlitePool) -> AppResult<TodayStats> {
    let today = Local::now().date_naive();
    let (start, end) = local_day_range(today)?;

    let sitting_seconds: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM sitting_sessions
         WHERE started_at >= ? AND started_at < ?",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await?;

    let longest_sitting_seconds: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(duration_seconds), 0) FROM sitting_sessions
         WHERE started_at >= ? AND started_at < ?",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await?;

    let activity_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_sessions
         WHERE started_at >= ? AND started_at < ? AND completion = 'completed'",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await?;

    let activity_seconds: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM activity_sessions
         WHERE started_at >= ? AND started_at < ? AND completion = 'completed'",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await?;

    // 应完成活动次数 = 当日触发的提醒次数，完成率口径见 PRD 7.3。
    let expected_activities: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_events
         WHERE event_type = 'reminder_triggered' AND created_at >= ? AND created_at < ?",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await?;

    let completion_rate = if expected_activities > 0 {
        Some(
            ((activity_count as f64 / expected_activities as f64) * 100.0)
                .min(100.0)
                .round(),
        )
    } else {
        None
    };

    let recent_activities = recent_activity_sessions(pool, 6).await?;
    let trend = trend_points(pool, 7).await?;

    Ok(TodayStats {
        date: today.to_string(),
        sitting_seconds,
        activity_count,
        activity_seconds,
        longest_sitting_seconds,
        expected_activities,
        completion_rate,
        trend,
        recent_activities,
    })
}

/// 最近 N 天（含今天）的每日汇总，用于首页与统计页趋势图。
pub async fn trend_points(pool: &SqlitePool, days: i64) -> AppResult<Vec<DailyPoint>> {
    let today = Local::now().date_naive();
    let mut points = Vec::new();
    for offset in (0..days).rev() {
        let date = today - Duration::days(offset);
        let (start, end) = local_day_range(date)?;

        let sitting_seconds: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(duration_seconds), 0) FROM sitting_sessions
             WHERE started_at >= ? AND started_at < ?",
        )
        .bind(&start)
        .bind(&end)
        .fetch_one(pool)
        .await?;

        let activity_seconds: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(duration_seconds), 0) FROM activity_sessions
             WHERE started_at >= ? AND started_at < ? AND completion = 'completed'",
        )
        .bind(&start)
        .bind(&end)
        .fetch_one(pool)
        .await?;

        let activity_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM activity_sessions
             WHERE started_at >= ? AND started_at < ? AND completion = 'completed'",
        )
        .bind(&start)
        .bind(&end)
        .fetch_one(pool)
        .await?;

        points.push(DailyPoint {
            date: date.format("%m-%d").to_string(),
            sitting_seconds,
            activity_seconds,
            activity_count,
        });
    }
    Ok(points)
}

/// 月度汇总（统计页用）。
pub async fn month_summary(pool: &SqlitePool) -> AppResult<serde_json::Value> {
    let today = Local::now().date_naive();
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .ok_or_else(|| AppError::Invalid("无法计算月份起始日".into()))?;
    let (start, _) = local_day_range(month_start)?;

    let sitting: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM sitting_sessions WHERE started_at >= ?",
    )
    .bind(&start)
    .fetch_one(pool)
    .await?;
    let activity: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM activity_sessions
         WHERE started_at >= ? AND completion = 'completed'",
    )
    .bind(&start)
    .fetch_one(pool)
    .await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_sessions WHERE started_at >= ? AND completion = 'completed'",
    )
    .bind(&start)
    .fetch_one(pool)
    .await?;

    // 月度完成率口径与今日一致：已完成次数 / 当月触发的提醒次数。
    let expected: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_events
         WHERE event_type = 'reminder_triggered' AND created_at >= ?",
    )
    .bind(&start)
    .fetch_one(pool)
    .await?;
    let completion_rate = if expected > 0 {
        Some(
            ((count as f64 / expected as f64) * 100.0)
                .min(100.0)
                .round(),
        )
    } else {
        None
    };

    // 每周活动汇总（按 ISO 周号分组，UTC 日期）。
    let weekly_rows = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT strftime('%Y-%W', started_at) AS week,
                COUNT(*) FILTER (WHERE completion = 'completed'),
                COALESCE(SUM(CASE WHEN completion = 'completed' THEN duration_seconds ELSE 0 END), 0)
         FROM activity_sessions
         WHERE started_at >= ?
         GROUP BY week
         ORDER BY week",
    )
    .bind(&start)
    .fetch_all(pool)
    .await?;
    let weekly: Vec<serde_json::Value> = weekly_rows
        .into_iter()
        .map(|(week, activity_count, activity_seconds)| {
            json!({
                "week": week,
                "activityCount": activity_count,
                "activitySeconds": activity_seconds,
            })
        })
        .collect();

    Ok(json!({
        "month": format!("{}-{:02}", today.year(), today.month()),
        "sittingSeconds": sitting,
        "activitySeconds": activity,
        "activityCount": count,
        "completionRate": completion_rate,
        "weekly": weekly,
        "daily": trend_points(pool, today.day() as i64).await?,
    }))
}

pub async fn history_sessions(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
    category: Option<String>,
    keyword: Option<String>,
    completion: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> AppResult<Vec<ActivitySession>> {
    let mut sql = String::from("SELECT * FROM activity_sessions WHERE 1 = 1");
    if category.is_some() {
        sql.push_str(" AND category = ?");
    }
    if keyword.is_some() {
        sql.push_str(" AND activity_name LIKE ?");
    }
    if completion.is_some() {
        sql.push_str(" AND completion = ?");
    }
    // started_at 为 UTC ISO8601，取前 10 位日期做前缀比较（YYYY-MM-DD）
    if from.is_some() {
        sql.push_str(" AND substr(started_at, 1, 10) >= ?");
    }
    if to.is_some() {
        sql.push_str(" AND substr(started_at, 1, 10) <= ?");
    }
    sql.push_str(" ORDER BY started_at DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query_as::<_, ActivitySessionRow>(&sql);
    if let Some(c) = category {
        query = query.bind(c);
    }
    if let Some(k) = keyword {
        query = query.bind(format!("%{k}%"));
    }
    if let Some(c) = completion {
        query = query.bind(c);
    }
    if let Some(f) = from {
        query = query.bind(f);
    }
    if let Some(t) = to {
        query = query.bind(t);
    }
    query = query.bind(limit).bind(offset);

    let rows = query.fetch_all(pool).await?;
    Ok(rows.into_iter().map(ActivitySession::from).collect())
}

pub async fn sitting_history(pool: &SqlitePool, limit: i64) -> AppResult<Vec<SittingSession>> {
    Ok(sqlx::query_as::<_, SittingSession>(
        "SELECT * FROM sitting_sessions ORDER BY started_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

/// 供调试与单元测试使用：读取某表行数。
pub async fn count_rows(pool: &SqlitePool, table: &str) -> AppResult<i64> {
    let allowed = [
        "app_settings",
        "reminder_plans",
        "work_schedules",
        "activities",
        "activity_sessions",
        "sitting_sessions",
        "app_events",
        "daily_stats",
    ];
    if !allowed.contains(&table) {
        return Err(AppError::Invalid(format!("未知数据表：{table}")));
    }
    let sql = format!("SELECT COUNT(*) FROM {table}");
    Ok(sqlx::query_scalar(&sql).fetch_one(pool).await?)
}

// ---------------------------------------------------------------- 导出 / 导入

const EXPORT_SCHEMA_VERSION: i64 = 1;

const PLAN_COLS: &[&str] = &[
    "id",
    "name",
    "description",
    "mode",
    "work_seconds",
    "break_seconds",
    "reminder_methods",
    "repeat_interval_seconds",
    "repeat_times",
    "snooze_seconds",
    "completion_mode",
    "idle_detection",
    "idle_threshold_seconds",
    "work_start",
    "work_end",
    "work_days",
    "smart_enabled",
    "is_builtin",
    "sort_order",
    "created_at",
    "updated_at",
];

const SCHEDULE_COLS: &[&str] = &[
    "id",
    "plan_id",
    "weekday",
    "start_minute",
    "end_minute",
    "enabled",
    "created_at",
];

const ACTIVITY_COLS: &[&str] = &[
    "id",
    "name",
    "category",
    "description",
    "duration_seconds",
    "steps",
    "image",
    "cautions",
    "enabled",
    "is_builtin",
    "sort_order",
    "created_at",
    "updated_at",
];

const SESSION_COLS: &[&str] = &[
    "id",
    "activity_id",
    "activity_name",
    "category",
    "plan_id",
    "started_at",
    "ended_at",
    "duration_seconds",
    "planned_seconds",
    "completion",
    "source",
    "created_at",
];

const SITTING_COLS: &[&str] = &[
    "id",
    "plan_id",
    "started_at",
    "ended_at",
    "duration_seconds",
    "interrupted_by",
    "created_at",
];

const STATS_COLS: &[&str] = &[
    "date",
    "sitting_seconds",
    "activity_count",
    "activity_seconds",
    "longest_sitting_seconds",
    "expected_activities",
    "completion_rate",
    "computed_at",
];

const SETTING_COLS: &[&str] = &["key", "value", "updated_at"];

fn cell_to_json(row: &sqlx::sqlite::SqliteRow, name: &str) -> serde_json::Value {
    if let Ok(v) = row.try_get::<String, _>(name) {
        return json!(v);
    }
    if let Ok(v) = row.try_get::<i64, _>(name) {
        return json!(v);
    }
    if let Ok(v) = row.try_get::<f64, _>(name) {
        return json!(v);
    }
    serde_json::Value::Null
}

async fn table_to_json(pool: &SqlitePool, table: &str) -> AppResult<Vec<serde_json::Value>> {
    let rows = sqlx::query(&format!("SELECT * FROM {table}"))
        .fetch_all(pool)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut map = serde_json::Map::new();
        for col in row.columns() {
            map.insert(col.name().to_string(), cell_to_json(&row, col.name()));
        }
        out.push(serde_json::Value::Object(map));
    }
    Ok(out)
}

/// 导出全部业务数据为可序列化的 JSON 对象（写文件由命令层负责）。
pub async fn export_all_data(pool: &SqlitePool) -> AppResult<serde_json::Value> {
    Ok(json!({
        "schemaVersion": EXPORT_SCHEMA_VERSION,
        "app": "SitHalt",
        "exportedAt": now_iso(),
        "settings": table_to_json(pool, "app_settings").await?,
        "plans": table_to_json(pool, "reminder_plans").await?,
        "schedules": table_to_json(pool, "work_schedules").await?,
        "activities": table_to_json(pool, "activities").await?,
        "activitySessions": table_to_json(pool, "activity_sessions").await?,
        "sittingSessions": table_to_json(pool, "sitting_sessions").await?,
        "dailyStats": table_to_json(pool, "daily_stats").await?,
    }))
}

fn json_to_option_string(v: serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s),
        other => Some(other.to_string()),
    }
}

async fn insert_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    cols: &[&str],
    rows: Option<&serde_json::Value>,
) -> AppResult<()> {
    let Some(rows) = rows.and_then(|v| v.as_array()) else {
        return Ok(());
    };
    if rows.is_empty() {
        return Ok(());
    }
    let placeholders = cols.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({placeholders})",
        cols.join(",")
    );
    for row in rows {
        let Some(obj) = row.as_object() else { continue };
        let mut query = sqlx::query(&sql);
        for col in cols {
            let value = obj.get(*col).cloned().unwrap_or(serde_json::Value::Null);
            query = query.bind(json_to_option_string(value));
        }
        query.execute(&mut **tx).await?;
    }
    Ok(())
}

/// 导入 JSON 数据：校验版本与必备数据 → 事务内清空业务表并按依赖顺序写回。
/// 调用方需在导入前先自动备份当前库，失败时事务回滚、原始数据不受影响。
pub async fn import_all_data(pool: &SqlitePool, payload: &serde_json::Value) -> AppResult<()> {
    let version = payload
        .get("schemaVersion")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if version != EXPORT_SCHEMA_VERSION {
        return Err(AppError::Invalid(format!("不支持的数据版本：{version}")));
    }
    for key in ["plans", "activities"] {
        if payload.get(key).and_then(|v| v.as_array()).is_none() {
            return Err(AppError::Invalid(format!("导出文件缺少 {key} 数据")));
        }
    }

    let mut tx = pool.begin().await?;

    // 逆外键顺序清空
    for table in [
        "work_schedules",
        "activity_sessions",
        "sitting_sessions",
        "daily_stats",
        "activities",
        "reminder_plans",
        "app_settings",
    ] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *tx)
            .await?;
    }

    insert_rows(&mut tx, "reminder_plans", PLAN_COLS, payload.get("plans")).await?;
    insert_rows(&mut tx, "work_schedules", SCHEDULE_COLS, payload.get("schedules")).await?;
    insert_rows(&mut tx, "activities", ACTIVITY_COLS, payload.get("activities")).await?;
    insert_rows(&mut tx, "activity_sessions", SESSION_COLS, payload.get("activitySessions")).await?;
    insert_rows(&mut tx, "sitting_sessions", SITTING_COLS, payload.get("sittingSessions")).await?;
    insert_rows(&mut tx, "daily_stats", STATS_COLS, payload.get("dailyStats")).await?;
    insert_rows(&mut tx, "app_settings", SETTING_COLS, payload.get("settings")).await?;

    tx.commit().await?;
    Ok(())
}

/// 恢复出厂：事务内清空全部业务表，再重新播种内置计划 / 活动与默认设置。
/// 调用方需在前端完成二次确认；本函数不做备份。
pub async fn reset_all_data(pool: &SqlitePool) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for table in [
        "work_schedules",
        "activity_sessions",
        "sitting_sessions",
        "daily_stats",
        "app_events",
        "activities",
        "reminder_plans",
        "app_settings",
    ] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    // 库已清空，重新生成内置计划、内置活动与默认设置。
    seed_if_empty(pool).await?;
    Ok(())
}

fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// 把活动记录导出为 CSV 文本（带 UTF-8 BOM，便于 Excel 直接打开）。
pub async fn export_activity_csv(pool: &SqlitePool) -> AppResult<String> {
    let rows = sqlx::query_as::<_, ActivitySessionRow>(
        "SELECT * FROM activity_sessions ORDER BY started_at DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut csv = String::from("\u{FEFF}");
    csv.push_str("开始时间,活动名称,分类,计划ID,时长(秒),计划时长(秒),完成状态,来源,结束时间\n");
    for r in rows {
        let line = [
            r.started_at,
            r.activity_name,
            r.category.unwrap_or_default(),
            r.plan_id.unwrap_or_default(),
            r.duration_seconds.to_string(),
            r.planned_seconds.to_string(),
            r.completion,
            r.source,
            r.ended_at.unwrap_or_default(),
        ]
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",");
        csv.push_str(&line);
        csv.push('\n');
    }
    Ok(csv)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn memory_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("连接内存数据库");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("执行迁移");
        pool
    }

    const TABLES: [&str; 8] = [
        "app_settings",
        "reminder_plans",
        "work_schedules",
        "activities",
        "activity_sessions",
        "sitting_sessions",
        "app_events",
        "daily_stats",
    ];

    #[tokio::test]
    async fn migrations_create_all_tables() {
        let pool = memory_pool().await;
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table'
             AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        for expected in TABLES {
            assert!(
                tables.iter().any(|name| name == expected),
                "迁移后缺少数据表：{expected}"
            );
        }
    }

    #[tokio::test]
    async fn settings_roundtrip_and_events() {
        let pool = memory_pool().await;
        set_setting(&pool, "theme", json!("dark")).await.unwrap();
        let raw = get_setting_raw(&pool, "theme").await.unwrap();
        assert_eq!(raw.as_deref(), Some("\"dark\""));

        record_event(&pool, "reminder_triggered", json!({ "planId": "p" })).await;
        let events: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM app_events WHERE event_type = 'reminder_triggered'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(events, 1);
    }

    #[tokio::test]
    async fn work_schedule_crud_roundtrip() {
        let pool = memory_pool().await;
        seed_if_empty(&pool).await.unwrap();
        let plan_id = list_plans(&pool).await.unwrap()[0].id.clone();

        let saved = save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: None,
                plan_id: plan_id.clone(),
                weekday: 3,
                start_minute: 9 * 60,
                end_minute: 12 * 60,
                enabled: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(saved.weekday, 3);
        assert_eq!(saved.end_minute, 720);

        let listed = list_work_schedules(&pool, Some(&plan_id)).await.unwrap();
        assert_eq!(listed.len(), 1);

        // 更新同一条记录（沿用 id），不应产生新行
        let updated = save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: Some(saved.id.clone()),
                plan_id: plan_id.clone(),
                weekday: 3,
                start_minute: 13 * 60,
                end_minute: 18 * 60,
                enabled: false,
            },
        )
        .await
        .unwrap();
        assert!(!updated.enabled);
        assert_eq!(
            list_work_schedules(&pool, Some(&plan_id))
                .await
                .unwrap()
                .len(),
            1
        );

        // 非法输入被拒绝
        let bad = save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: None,
                plan_id: plan_id.clone(),
                weekday: 8,
                start_minute: 0,
                end_minute: 60,
                enabled: true,
            },
        )
        .await;
        assert!(bad.is_err(), "weekday 超出 1-7 应报错");

        delete_work_schedule(&pool, &saved.id).await.unwrap();
        assert!(
            list_work_schedules(&pool, Some(&plan_id))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn export_import_roundtrip_preserves_data() {
        let pool = memory_pool().await;
        seed_if_empty(&pool).await.unwrap();

        set_setting(&pool, "theme", json!("dark")).await.unwrap();
        let plan_id = list_plans(&pool).await.unwrap()[0].id.clone();
        sqlx::query(
            "INSERT INTO activity_sessions
               (id, activity_id, activity_name, category, plan_id, started_at, ended_at,
                duration_seconds, planned_seconds, completion, source, created_at)
             VALUES ('s1', NULL, '起身行走', '身体活动', ?, ?, NULL, 120, 300, 'completed', 'reminder', ?)",
        )
        .bind(&plan_id)
        .bind(now_iso())
        .bind(now_iso())
        .execute(&pool)
        .await
        .unwrap();

        let exported = export_all_data(&pool).await.unwrap();

        // 清空计划与记录，模拟库被改写
        sqlx::query("DELETE FROM activity_sessions")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM reminder_plans")
            .execute(&pool)
            .await
            .unwrap();
        let wiped: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM reminder_plans")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(wiped, 0);

        import_all_data(&pool, &exported).await.unwrap();

        let plans = list_plans(&pool).await.unwrap();
        assert!(!plans.is_empty(), "导入后计划应还原");
        let sessions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM activity_sessions WHERE id = 's1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(sessions, 1);
        let theme = get_setting_raw(&pool, "theme").await.unwrap();
        assert_eq!(theme.as_deref(), Some("\"dark\""));

        // 不支持的数据版本应被拒绝，且不破坏当前数据
        let mut bad = exported.clone();
        bad["schemaVersion"] = json!(999);
        assert!(import_all_data(&pool, &bad).await.is_err());
    }

    #[tokio::test]
    async fn schedule_overlap_is_rejected() {
        let pool = memory_pool().await;
        seed_if_empty(&pool).await.unwrap();
        let plan_id = list_plans(&pool).await.unwrap()[0].id.clone();

        save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: None,
                plan_id: plan_id.clone(),
                weekday: 3,
                start_minute: 9 * 60,
                end_minute: 12 * 60,
                enabled: true,
            },
        )
        .await
        .unwrap();

        // 完全包含在已有时段内 → 拒绝
        let inside = save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: None,
                plan_id: plan_id.clone(),
                weekday: 3,
                start_minute: 10 * 60,
                end_minute: 11 * 60,
                enabled: true,
            },
        )
        .await;
        assert!(inside.is_err(), "重叠时段应被拒绝");

        // 相邻不重叠（12:00 紧接着开始）→ 允许
        let adjacent = save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: None,
                plan_id: plan_id.clone(),
                weekday: 3,
                start_minute: 12 * 60,
                end_minute: 13 * 60,
                enabled: true,
            },
        )
        .await;
        assert!(adjacent.is_ok());

        // 跨零点时段与凌晨时段重叠 → 拒绝
        save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: None,
                plan_id: plan_id.clone(),
                weekday: 4,
                start_minute: 22 * 60,
                end_minute: 2 * 60,
                enabled: true,
            },
        )
        .await
        .unwrap();
        let early = save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: None,
                plan_id: plan_id.clone(),
                weekday: 4,
                start_minute: 1 * 60,
                end_minute: 3 * 60,
                enabled: true,
            },
        )
        .await;
        assert!(early.is_err(), "跨零点重叠应被拒绝");

        // 禁用时段不参与重叠校验
        let disabled = save_work_schedule(
            &pool,
            WorkScheduleInput {
                id: None,
                plan_id: plan_id.clone(),
                weekday: 3,
                start_minute: 10 * 60,
                end_minute: 11 * 60,
                enabled: false,
            },
        )
        .await;
        assert!(disabled.is_ok());
    }

    #[tokio::test]
    async fn reset_all_data_clears_and_reseeds() {
        let pool = memory_pool().await;
        seed_if_empty(&pool).await.unwrap();

        set_setting(&pool, "theme", json!("dark")).await.unwrap();
        let plan_id = list_plans(&pool).await.unwrap()[0].id.clone();
        sqlx::query(
            "INSERT INTO activity_sessions
               (id, activity_id, activity_name, category, plan_id, started_at, ended_at,
                duration_seconds, planned_seconds, completion, source, created_at)
             VALUES ('s1', NULL, '起身行走', '身体活动', ?, ?, NULL, 120, 300, 'completed', 'reminder', ?)",
        )
        .bind(&plan_id)
        .bind(now_iso())
        .bind(now_iso())
        .execute(&pool)
        .await
        .unwrap();

        reset_all_data(&pool).await.unwrap();

        let session_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM activity_sessions")
                .fetch_one(&pool)
                .await
                .unwrap();
        let sitting_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sitting_sessions")
                .fetch_one(&pool)
                .await
                .unwrap();
        let plan_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM reminder_plans")
                .fetch_one(&pool)
                .await
                .unwrap();
        let activity_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM activities")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(session_count, 0);
        assert_eq!(sitting_count, 0);
        assert!(plan_count > 0, "应重新播种内置计划");
        assert!(activity_count > 0, "应重新播种内置活动");

        // 自定义设置被清空，回到默认主题
        let theme = get_setting_raw(&pool, "theme").await.unwrap();
        assert_eq!(theme, Some(json!("system").to_string()));
    }

    #[tokio::test]
    async fn activity_image_roundtrip() {
        let pool = memory_pool().await;
        seed_if_empty(&pool).await.unwrap();

        let saved = save_activity(
            &pool,
            ActivityInput {
                id: None,
                name: "测试动作".into(),
                category: "身体活动".into(),
                description: None,
                duration_seconds: 60,
                steps: vec!["步骤一".into()],
                cautions: None,
                image: Some("https://example.com/a.png".into()),
                enabled: true,
                sort_order: 99,
            },
        )
        .await
        .unwrap();
        assert_eq!(saved.image, Some("https://example.com/a.png".into()));

        // 更新时清空图片，应写回 NULL
        let updated = save_activity(
            &pool,
            ActivityInput {
                id: Some(saved.id.clone()),
                name: saved.name,
                category: saved.category,
                description: saved.description,
                duration_seconds: saved.duration_seconds,
                steps: saved.steps,
                cautions: saved.cautions,
                image: None,
                enabled: saved.enabled,
                sort_order: saved.sort_order,
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.image, None);
    }

    #[tokio::test]
    async fn export_csv_has_bom_and_header() {
        let pool = memory_pool().await;
        seed_if_empty(&pool).await.unwrap();
        let csv = export_activity_csv(&pool).await.unwrap();
        assert!(csv.starts_with("\u{FEFF}"), "CSV 应带 UTF-8 BOM");
        assert!(csv.contains("开始时间,活动名称,分类"));
    }
}
