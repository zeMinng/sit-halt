pub mod machine;

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, Timelike, Utc};
use serde_json::json;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Mutex;

use crate::db::{self, NewActivitySession};
use crate::error::{AppError, AppResult};
use crate::models::{Activity, EngineStatus, ReminderPlan, WorkSchedule};

use machine::{Machine, Phase, PlanSpec, Trigger};

pub const EVT_STATUS: &str = "engine:status";
pub const EVT_REMINDER: &str = "engine:reminder";
pub const EVT_RECORDED: &str = "engine:recorded";

const CHECKPOINT_KEY: &str = "engineCheckpoint";

/// 一段久坐会话（工作周期）的落库信息。
struct SessionState {
    id: String,
    started_at: String,
}

/// 一段活动（休息）的落库信息。
struct BreakState {
    started_at: String,
    activity_id: Option<String>,
    activity_name: String,
    category: Option<String>,
    planned_seconds: i64,
    source: String,
}

struct Inner {
    machine: Machine,
    plan: Option<ReminderPlan>,
    last_tick: Instant,
    session: Option<SessionState>,
    break_state: Option<BreakState>,
    /// 活动倒计时结束但完成方式为「询问」时，等待用户确认
    awaiting_confirm: bool,
    last_persist: Instant,
    /// 最近一次读取到的系统空闲秒数
    idle_seconds: Option<u64>,
    /// 挂起原因：idle / outside_work_hours
    suspend_reason: Option<String>,
    /// 当前计划配置的工作时段（为空时退回计划的起止时间）
    schedules: Vec<WorkSchedule>,
}

pub struct Engine {
    inner: Mutex<Inner>,
    pool: SqlitePool,
    app: AppHandle,
    /// 托盘提示文字的节流时间
    tray_tick: std::sync::Mutex<Instant>,
}

impl Engine {
    pub async fn new(app: AppHandle, pool: SqlitePool) -> Arc<Self> {
        let plan = db::default_plan(&pool).await.ok().flatten();
        let schedules = match plan.as_ref() {
            Some(p) => db::list_work_schedules(&pool, Some(&p.id))
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let mut machine = Machine::new();
        if let Some(p) = plan.as_ref() {
            machine.set_plan(PlanSpec {
                work_seconds: p.work_seconds as u64,
                break_seconds: p.break_seconds as u64,
                completion_mode: p.completion_mode.clone(),
                repeat_times: p.repeat_times as u32,
                repeat_interval_seconds: p.repeat_interval_seconds as u64,
            });
        }

        // 重启恢复：读取断点，若上次处于计时中，则恢复为「暂停中」并保留进度，
        // 避免把应用关闭期间的时间算进久坐（PRD 13.2）。
        let mut restored = false;
        if let Ok(Some(raw)) = db::get_setting_raw(&pool, CHECKPOINT_KEY).await {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                let saved_plan = value
                    .get("planId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let work_seconds = value
                    .get("workSeconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let phase = value
                    .get("phase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("idle")
                    .to_string();
                let same_plan = plan.as_ref().map(|p| p.id == saved_plan).unwrap_or(false);
                if same_plan && !matches!(phase.as_str(), "idle" | "break") && work_seconds > 0 {
                    machine.work_seconds = work_seconds;
                    machine.phase = Phase::Paused;
                    restored = true;
                }
            }
        }

        if restored {
            eprintln!("[engine] 已从断点恢复上次计时进度，当前为暂停状态，等待用户继续");
        }

        Arc::new(Self {
            inner: Mutex::new(Inner {
                machine,
                plan,
                last_tick: Instant::now(),
                session: None,
                break_state: None,
                awaiting_confirm: false,
                last_persist: Instant::now(),
                idle_seconds: None,
                suspend_reason: None,
                schedules,
            }),
            pool,
            app,
            tray_tick: std::sync::Mutex::new(Instant::now()),
        })
    }

    pub fn spawn_ticker(self: &Arc<Self>) {
        let engine = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                if let Err(err) = engine.tick().await {
                    eprintln!("[engine] tick 失败：{err}");
                }
            }
        });
    }

    // ------------------------------------------------------------ 内部工具

    fn status_of(inner: &Inner) -> EngineStatus {
        let m = &inner.machine;
        EngineStatus {
            phase: m.phase.as_str().to_string(),
            plan_id: inner.plan.as_ref().map(|p| p.id.clone()),
            plan_name: inner.plan.as_ref().map(|p| p.name.clone()),
            reminder_methods: inner
                .plan
                .as_ref()
                .map(|p| p.reminder_methods.clone())
                .unwrap_or_default(),
            work_seconds: m.work_seconds as i64,
            work_target_seconds: m.work_target() as i64,
            work_remaining_seconds: m.remaining_work_seconds() as i64,
            break_seconds: m.break_seconds as i64,
            break_target_seconds: m.break_target_seconds() as i64,
            break_remaining_seconds: m.remaining_break_seconds() as i64,
            activity_id: m.activity_id.clone(),
            activity_name: m.activity_name.clone(),
            completion_mode: m.plan.as_ref().map(|p| p.completion_mode.clone()),
            reminder_elapsed_seconds: m.reminder_elapsed as i64,
            snooze_remaining_seconds: m.snooze_remaining.map(|s| s as i64),
            dnd_remaining_seconds: m.dnd_remaining.map(|s| s as i64),
            cycle_index: m.cycle_index as i64,
            ignored_count: m.ignored_count as i64,
            can_resume: matches!(m.phase, Phase::Paused | Phase::Suspended),
            idle_seconds: inner.idle_seconds.map(|s| s as i64),
            suspend_reason: inner.suspend_reason.clone(),
            in_work_window: in_work_window_at(inner.plan.as_ref(), &inner.schedules, Local::now()),
            schedule_count: inner.schedules.iter().filter(|s| s.enabled).count() as i64,
            server_time: Utc::now().to_rfc3339(),
        }
    }

    fn emit_status(&self, status: &EngineStatus) {
        let _ = self.app.emit(EVT_STATUS, status);
        self.update_tray(status);
    }

    /// 托盘提示：显示「下次提醒」倒计时。每 10 秒最多刷新一次，避免每秒调用系统接口。
    fn update_tray(&self, status: &EngineStatus) {
        {
            let mut last = match self.tray_tick.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let urgent = matches!(status.phase.as_str(), "idle" | "reminding" | "break");
            if !urgent && last.elapsed() < Duration::from_secs(10) {
                return;
            }
            *last = Instant::now();
        }
        if let Some(tray) = self.app.tray_by_id("sithalt-tray") {
            let _ = tray.set_tooltip(Some(tray_text(status)));
        }
    }

    pub async fn status(&self) -> EngineStatus {
        let inner = self.inner.lock().await;
        Self::status_of(&inner)
    }

    fn notify(&self, title: &str, body: &str) {
        let _ = self
            .app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show();
    }

    /// 用户在设置里关闭系统通知后，Rust 侧不再发系统通知（前端仍会收到提醒事件）。
    async fn notification_enabled(&self) -> bool {
        db::get_setting_raw(&self.pool, "notificationEnabled")
            .await
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str::<bool>(&raw).ok())
            .unwrap_or(true)
    }

    async fn persist_checkpoint(
        &self,
        plan_id: Option<String>,
        work_seconds: u64,
        phase: &str,
        cycle_index: u64,
    ) {
        let value = json!({
            "planId": plan_id,
            "workSeconds": work_seconds,
            "phase": phase,
            "cycleIndex": cycle_index,
            "savedAt": Utc::now().to_rfc3339(),
        });
        let _ = db::set_setting(&self.pool, CHECKPOINT_KEY, value).await;
    }

    async fn open_session(&self, plan_id: &str) -> AppResult<SessionState> {
        let id = db::open_sitting_session(&self.pool, plan_id).await?;
        Ok(SessionState {
            id,
            started_at: db::now_iso(),
        })
    }

    async fn flush_session(&self, inner: &mut Inner, reason: &str) {
        if let Some(session) = inner.session.take() {
            let seconds = inner.machine.work_seconds as i64;
            db::close_sitting_session(&self.pool, &session.id, seconds, reason).await;
            db::record_event(
                &self.pool,
                "sitting_session_closed",
                json!({
                    "sessionId": session.id,
                    "startedAt": session.started_at,
                    "seconds": seconds,
                    "reason": reason,
                }),
            )
            .await;
        }
    }

    async fn record_break_session(
        &self,
        break_state: &BreakState,
        actual_seconds: i64,
        completion: &str,
    ) {
        let _ = db::insert_activity_session(
            &self.pool,
            NewActivitySession {
                activity_id: break_state.activity_id.as_deref(),
                activity_name: &break_state.activity_name,
                category: break_state.category.as_deref(),
                plan_id: None,
                started_at: &break_state.started_at,
                duration_seconds: actual_seconds,
                planned_seconds: break_state.planned_seconds,
                completion,
                source: &break_state.source,
            },
        )
        .await;
        let _ = self.app.emit(
            EVT_RECORDED,
            json!({
                "activityName": break_state.activity_name,
                "duration": actual_seconds,
                "completion": completion,
            }),
        );
    }

    // ------------------------------------------------------------ 主循环

    async fn tick(&self) -> AppResult<()> {
        let now = Instant::now();
        let mut trigger: Option<Trigger> = None;
        let status: EngineStatus;
        let mut need_session = false;
        let mut session_update: Option<i64> = None;
        let mut pause_reason: Option<String> = None;
        let mut suspended: Option<String> = None;
        let mut auto_resumed = false;

        {
            let mut inner = self.inner.lock().await;
            let mut dt = now.duration_since(inner.last_tick);
            inner.last_tick = now;

            // 单次跨度超过 5 秒视为系统休眠 / 卡顿，不计入久坐时长。
            if dt > Duration::from_secs(5) {
                dt = Duration::ZERO;
            }
            let secs = dt.as_secs();

            // 系统空闲 / 工作时间判断（Phase 3）：离开座位或在非工作时间不计久坐。
            let idle = crate::platform::IdleProbe::idle_seconds();
            inner.idle_seconds = idle;
            let idle_threshold = inner
                .plan
                .as_ref()
                .map(|p| p.idle_threshold_seconds.max(30) as u64)
                .unwrap_or(180);
            // 智能模式强制按「实际活跃时长」计时，其它模式跟随计划里的空闲检测开关
            let idle_enabled = inner
                .plan
                .as_ref()
                .map(|p| p.idle_detection || p.mode == "smart")
                .unwrap_or(false);
            let away = idle_enabled && idle.map(|s| s >= idle_threshold).unwrap_or(false);
            let in_window = in_work_window_at(inner.plan.as_ref(), &inner.schedules, Local::now());

            if secs > 0 {
                inner.machine.tick_dnd(secs);
                let suppress = inner.machine.in_dnd() && inner.machine.phase == Phase::Working;
                if !suppress && !inner.awaiting_confirm {
                    trigger = inner.machine.tick_work(secs);
                    // 用户长时间无响应时，按计划的「重复提醒间隔 / 次数」再次提醒
                    if trigger.is_none() && inner.machine.should_repeat_reminder() {
                        trigger = Some(Trigger::ReminderRepeated);
                    }
                }
            }

            // 离开座位或不在工作时间：挂起计时，不把这段时间算进久坐（PRD 6.2 / AC-06）
            match presence_action(inner.machine.phase, away, in_window) {
                PresenceAction::SuspendWork => {
                    if inner.machine.suspend().is_ok() {
                        let reason = if away { "idle" } else { "outside_work_hours" };
                        inner.suspend_reason = Some(reason.to_string());
                        pause_reason = Some(reason.to_string());
                        suspended = Some(reason.to_string());
                        trigger = None;
                    }
                }
                PresenceAction::Restore => {
                    if inner.machine.restore_from_suspend().is_ok() {
                        inner.suspend_reason = None;
                        auto_resumed = true;
                    }
                }
                PresenceAction::None => {}
            }

            // 工作期间每 15 秒把进度写回数据库，保证异常退出后仍可恢复。
            if inner.machine.phase == Phase::Working {
                if inner.session.is_none() {
                    need_session = true;
                } else if now.duration_since(inner.last_persist) >= Duration::from_secs(15) {
                    inner.last_persist = now;
                    session_update = Some(inner.machine.work_seconds as i64);
                }
            }

            match trigger {
                Some(Trigger::Reminder) | Some(Trigger::ReminderRepeated) => {
                    pause_reason = Some("reminder".into());
                }
                Some(Trigger::BreakElapsed) => {
                    // 完成方式：手动 / 询问 时等待用户确认，不自动结束
                    let mode = inner
                        .machine
                        .plan
                        .as_ref()
                        .map(|p| p.completion_mode.clone())
                        .unwrap_or_else(|| "auto".into());
                    if mode != "auto" {
                        inner.awaiting_confirm = true;
                        trigger = None;
                    }
                }
                _ => {}
            }

            status = Self::status_of(&inner);
        }

        if need_session {
            if let Some(plan_id) = status.plan_id.clone() {
                let session = self.open_session(&plan_id).await?;
                let mut inner = self.inner.lock().await;
                inner.session = Some(session);
            }
        }

        if let Some(seconds) = session_update {
            let session_id = {
                let inner = self.inner.lock().await;
                inner.session.as_ref().map(|s| s.id.clone())
            };
            if let Some(id) = session_id {
                db::update_sitting_session(&self.pool, &id, seconds).await;
            }
        }

        if let Some(reason) = pause_reason {
            let mut inner = self.inner.lock().await;
            self.flush_session(&mut inner, &reason).await;
        }

        if let Some(reason) = suspended {
            db::record_event(&self.pool, "engine_suspended", json!({ "reason": reason })).await;
        }
        if auto_resumed {
            db::record_event(&self.pool, "engine_auto_resumed", json!({})).await;
        }

        if let Some(t) = trigger {
            self.handle_trigger(t, &status).await?;
        }

        self.emit_status(&status);

        // 断点持久化（每 15 秒）
        let should_persist = {
            let inner = self.inner.lock().await;
            now.duration_since(inner.last_persist) >= Duration::from_secs(15)
        };
        if should_persist {
            let mut inner = self.inner.lock().await;
            inner.last_persist = now;
            let snapshot_plan = inner.plan.clone();
            let work = inner.machine.work_seconds;
            let phase = inner.machine.phase.as_str().to_string();
            let cycle = inner.machine.cycle_index;
            drop(inner);
            let _ = db::set_setting(
                &self.pool,
                CHECKPOINT_KEY,
                json!({
                    "planId": snapshot_plan.map(|p| p.id),
                    "workSeconds": work,
                    "phase": phase,
                    "cycleIndex": cycle,
                    "savedAt": Utc::now().to_rfc3339(),
                }),
            )
            .await;
        }

        Ok(())
    }

    async fn handle_trigger(&self, trigger: Trigger, status: &EngineStatus) -> AppResult<()> {
        match trigger {
            Trigger::Reminder | Trigger::ReminderRepeated => {
                let title = "该起来活动一下了";
                let body = status
                    .plan_name
                    .clone()
                    .map(|name| format!("{name} 的一个工作周期结束，活动 3~5 分钟再继续。"))
                    .unwrap_or_else(|| "已经连续坐了一段时间，起来活动一下。".to_string());
                let wants_notification = status.reminder_methods.iter().any(|m| m == "notification");
                if wants_notification && self.notification_enabled().await {
                    self.notify(title, &body);
                }
                db::record_event(
                    &self.pool,
                    "reminder_triggered",
                    json!({
                        "planId": status.plan_id,
                        "workSeconds": status.work_seconds,
                        "cycleIndex": status.cycle_index,
                    }),
                )
                .await;
                let _ = self.app.emit(EVT_REMINDER, status);
            }
            Trigger::BreakElapsed => {
                let mut inner = self.inner.lock().await;
                let break_state = inner.break_state.take();
                let keep_timing = inner.plan.is_some();
                inner.machine.end_break(true, keep_timing);
                inner.awaiting_confirm = false;
                drop(inner);

                if let Some(b) = break_state {
                    self.record_break_session(&b, b.planned_seconds, "completed")
                        .await;
                }
                if self.notification_enabled().await {
                    self.notify("活动完成", "本次活动已记录，下一轮工作开始。");
                }
            }
            Trigger::NextCycle => {}
        }
        Ok(())
    }

    // ------------------------------------------------------------ 对外命令

    pub async fn start(&self) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        if inner.plan.is_none() {
            return Err(AppError::State("尚未选择提醒计划".into()));
        }
        if !in_work_window_at(inner.plan.as_ref(), &inner.schedules, Local::now()) {
            return Err(AppError::State(
                "当前不在计划设定的工作时间（含适用工作日），暂不开始计时".into(),
            ));
        }
        inner.machine.start().map_err(AppError::State)?;
        inner.suspend_reason = None;
        let plan_id = inner
            .plan
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_default();
        drop(inner);

        let session = self.open_session(&plan_id).await?;
        let mut inner = self.inner.lock().await;
        inner.session = Some(session);
        if matches!(inner.machine.phase, Phase::Working) && inner.machine.dnd_remaining.is_some() {
            inner.machine.clear_dnd();
        }
        db::record_event(&self.pool, "engine_started", json!({ "planId": plan_id })).await;
        let status = Self::status_of(&inner);
        drop(inner);
        self.emit_status(&status);
        Ok(status)
    }

    pub async fn pause(&self) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        inner.machine.pause().map_err(AppError::State)?;
        self.flush_session(&mut inner, "pause").await;
        let status = Self::status_of(&inner);
        drop(inner);
        db::record_event(&self.pool, "engine_paused", json!({})).await;
        self.emit_status(&status);
        Ok(status)
    }

    pub async fn resume(&self) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        inner.machine.resume().map_err(AppError::State)?;
        inner.suspend_reason = None;
        let plan_id = inner
            .plan
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_default();
        drop(inner);

        let session = self.open_session(&plan_id).await?;
        let mut inner = self.inner.lock().await;
        inner.session = Some(session);
        let status = Self::status_of(&inner);
        drop(inner);
        db::record_event(&self.pool, "engine_resumed", json!({})).await;
        self.emit_status(&status);
        Ok(status)
    }

    pub async fn stop(&self) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        self.flush_session(&mut inner, "stop").await;
        if let Some(b) = inner.break_state.take() {
            drop(inner);
            self.record_break_session(&b, 0, "interrupted").await;
            inner = self.inner.lock().await;
        }
        inner.machine.stop();
        inner.awaiting_confirm = false;
        inner.suspend_reason = None;
        let status = Self::status_of(&inner);
        let plan_id = inner.plan.as_ref().map(|p| p.id.clone());
        let work_seconds = inner.machine.work_seconds;
        let phase = inner.machine.phase.as_str().to_string();
        let cycle_index = inner.machine.cycle_index;
        drop(inner);
        db::record_event(&self.pool, "engine_stopped", json!({})).await;
        self.persist_checkpoint(plan_id, work_seconds, &phase, cycle_index)
            .await;
        self.emit_status(&status);
        Ok(status)
    }

    /// 稍后提醒
    pub async fn snooze(&self) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        if inner.machine.phase != Phase::Reminding {
            return Err(AppError::State("当前没有待处理的提醒".into()));
        }
        let seconds = inner
            .plan
            .as_ref()
            .map(|p| p.snooze_seconds.max(60) as u64)
            .unwrap_or(300);
        inner.machine.snooze(seconds);
        let status = Self::status_of(&inner);
        drop(inner);
        db::record_event(
            &self.pool,
            "reminder_snoozed",
            json!({ "seconds": seconds }),
        )
        .await;
        self.emit_status(&status);
        Ok(status)
    }

    /// 跳过本次提醒（不计入活动完成）。番茄模式要求必须完成活动，因此不允许跳过。
    pub async fn skip(&self) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        if inner.machine.phase != Phase::Reminding {
            return Err(AppError::State("当前没有待处理的提醒".into()));
        }
        if inner.plan.as_ref().map(|p| p.mode.as_str()) == Some("pomodoro") {
            return Err(AppError::State(
                "番茄模式要求完成一次活动，请选择「开始活动」或「稍后提醒」".into(),
            ));
        }
        inner.machine.skip();
        inner.awaiting_confirm = false;
        let plan_id = inner
            .plan
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_default();
        drop(inner);
        db::record_event(&self.pool, "reminder_skipped", json!({})).await;

        let session = self.open_session(&plan_id).await?;
        let mut inner = self.inner.lock().await;
        inner.session = Some(session);
        let status = Self::status_of(&inner);
        drop(inner);
        self.emit_status(&status);
        Ok(status)
    }

    /// 已经活动：直接完成本次提醒，按自述记录一次完成
    pub async fn acknowledge(&self) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        if inner.machine.phase != Phase::Reminding {
            return Err(AppError::State("当前没有待处理的提醒".into()));
        }
        let plan_id = inner
            .plan
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_default();
        let planned = inner
            .machine
            .plan
            .as_ref()
            .map(|p| p.break_seconds as i64)
            .unwrap_or(300);
        inner.machine.acknowledge_activity();
        drop(inner);

        let started = db::now_iso();
        let _ = db::insert_activity_session(
            &self.pool,
            NewActivitySession {
                activity_id: None,
                activity_name: "自主活动",
                category: Some("自述完成"),
                plan_id: Some(&plan_id),
                started_at: &started,
                duration_seconds: 0,
                planned_seconds: planned,
                completion: "completed",
                source: "self_reported",
            },
        )
        .await;
        db::record_event(&self.pool, "reminder_acknowledged", json!({})).await;

        let session = self.open_session(&plan_id).await?;
        let mut inner = self.inner.lock().await;
        inner.session = Some(session);
        let status = Self::status_of(&inner);
        drop(inner);
        let _ = self.app.emit(
            EVT_RECORDED,
            json!({ "activityName": "自主活动", "completion": "completed" }),
        );
        self.emit_status(&status);
        Ok(status)
    }

    /// 开始活动（从提醒或首页「立即活动」进入）
    pub async fn start_break(&self, activity_id: Option<String>) -> AppResult<EngineStatus> {
        let explicit = activity_id.is_some();
        let activity = match activity_id {
            Some(id) => Some(db::get_activity(&self.pool, &id).await?),
            None => self.default_activity().await?,
        };

        let mut inner = self.inner.lock().await;
        let from_reminder = inner.machine.phase == Phase::Reminding;
        let (name, category, seconds, activity_id) = match activity {
            Some(Activity {
                id,
                name,
                category,
                duration_seconds,
                ..
            }) => {
                let plan_break = inner
                    .plan
                    .as_ref()
                    .map(|p| p.break_seconds as u64)
                    .unwrap_or(duration_seconds as u64);
                // 用户主动挑活动时按其建议时长；从提醒进入时按计划的活动时长
                let seconds = if explicit {
                    duration_seconds as u64
                } else {
                    plan_break.max(60)
                };
                (name, Some(category), seconds, Some(id))
            }
            None => ("起身活动".to_string(), None, 300u64, None),
        };

        // 离开工作状态：收尾久坐会话
        self.flush_session(&mut inner, "break").await;
        inner.awaiting_confirm = false;

        inner
            .machine
            .begin_break(activity_id.clone(), name.clone(), category.clone(), seconds);
        inner.break_state = Some(BreakState {
            started_at: db::now_iso(),
            activity_id: activity_id.clone(),
            activity_name: name.clone(),
            category: category.clone(),
            planned_seconds: seconds as i64,
            source: if from_reminder {
                "reminder".into()
            } else {
                "manual".into()
            },
        });
        let status = Self::status_of(&inner);
        drop(inner);

        db::record_event(
            &self.pool,
            "break_started",
            json!({ "activityId": activity_id, "activityName": name, "seconds": seconds }),
        )
        .await;
        self.emit_status(&status);
        Ok(status)
    }

    async fn default_activity(&self) -> AppResult<Option<Activity>> {
        let list = db::list_activities(&self.pool).await?;
        Ok(list
            .into_iter()
            .find(|a| a.enabled)
            .map(Some)
            .unwrap_or(None))
    }

    /// 结束活动（completed=false 记为提前结束）
    pub async fn finish_break(&self, completed: bool) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        if inner.machine.phase != Phase::Break {
            return Err(AppError::State("当前不在活动中".into()));
        }
        let actual = inner.machine.break_seconds as i64;
        let break_state = inner.break_state.take();
        let keep_timing = inner.plan.is_some();
        inner.machine.end_break(completed, keep_timing);
        inner.awaiting_confirm = false;
        let should_continue = inner.machine.phase == Phase::Working;
        let plan_id = inner.plan.as_ref().map(|p| p.id.clone());
        let status = Self::status_of(&inner);
        drop(inner);

        if let Some(b) = break_state {
            let completion = if completed { "completed" } else { "partial" };
            self.record_break_session(&b, actual, completion).await;
        }

        if should_continue {
            if let Some(plan_id) = plan_id {
                let session = self.open_session(&plan_id).await?;
                let mut inner = self.inner.lock().await;
                inner.session = Some(session);
            }
        }

        Ok(status)
    }

    /// 切换提醒计划。切换前收尾当前会话，避免丢失已有记录（PRD 4.3.4）。
    pub async fn switch_plan(&self, plan_id: String) -> AppResult<EngineStatus> {
        let plan = db::get_plan(&self.pool, &plan_id).await?;

        let mut inner = self.inner.lock().await;
        let was_active = matches!(inner.machine.phase, Phase::Working | Phase::Reminding);
        self.flush_session(&mut inner, "plan_switch").await;
        if let Some(b) = inner.break_state.take() {
            drop(inner);
            self.record_break_session(&b, 0, "interrupted").await;
            inner = self.inner.lock().await;
        }
        inner.machine.set_plan(PlanSpec {
            work_seconds: plan.work_seconds as u64,
            break_seconds: plan.break_seconds as u64,
            completion_mode: plan.completion_mode.clone(),
            repeat_times: plan.repeat_times as u32,
            repeat_interval_seconds: plan.repeat_interval_seconds as u64,
        });
        inner.machine.work_seconds = 0;
        inner.machine.phase = if was_active {
            Phase::Working
        } else {
            Phase::Idle
        };
        inner.suspend_reason = None;
        inner.plan = Some(plan.clone());
        inner.schedules = db::list_work_schedules(&self.pool, Some(&plan.id))
            .await
            .unwrap_or_default();
        let status = Self::status_of(&inner);
        drop(inner);

        db::set_active_plan(&self.pool, &plan_id).await?;
        db::record_event(&self.pool, "plan_switched", json!({ "planId": plan_id })).await;

        if was_active {
            let session = self.open_session(&plan_id).await?;
            let mut inner = self.inner.lock().await;
            inner.session = Some(session);
        }
        self.emit_status(&status);
        Ok(status)
    }

    /// 工作时间表被修改后重新载入，让正在运行的计时立刻按新时段判断。
    pub async fn reload_schedules(&self) -> AppResult<EngineStatus> {
        let plan_id = {
            let inner = self.inner.lock().await;
            inner.plan.as_ref().map(|p| p.id.clone())
        };
        let schedules = match plan_id {
            Some(id) => db::list_work_schedules(&self.pool, Some(&id))
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let mut inner = self.inner.lock().await;
        inner.schedules = schedules;
        let status = Self::status_of(&inner);
        drop(inner);
        self.emit_status(&status);
        Ok(status)
    }

    /// 当前在用计划被编辑后，重新从库载入计划快照与时间表。
    /// 只刷新配置（工作日、时段、时长、提醒方式等），不清空当前已累计的久坐进度、
    /// 也不切换计划；若保存的不是当前在用计划，则不做改动。
    pub async fn reload_plan_if_active(&self, plan_id: &str) -> AppResult<EngineStatus> {
        let current_id = {
            let inner = self.inner.lock().await;
            inner.plan.as_ref().map(|p| p.id.clone())
        };
        let Some(current_id) = current_id else {
            return Ok(self.status().await);
        };
        if current_id != plan_id {
            return Ok(self.status().await);
        }

        let plan = db::get_plan(&self.pool, plan_id).await?;
        let schedules = db::list_work_schedules(&self.pool, Some(&plan.id))
            .await
            .unwrap_or_default();

        let mut inner = self.inner.lock().await;
        inner.machine.set_plan(PlanSpec {
            work_seconds: plan.work_seconds as u64,
            break_seconds: plan.break_seconds as u64,
            completion_mode: plan.completion_mode.clone(),
            repeat_times: plan.repeat_times as u32,
            repeat_interval_seconds: plan.repeat_interval_seconds as u64,
        });
        inner.plan = Some(plan);
        inner.schedules = schedules;
        let status = Self::status_of(&inner);
        drop(inner);
        self.emit_status(&status);
        Ok(status)
    }

    /// 被删除的计划正是当前在用计划时，把引擎快照回退到库内默认计划；
    /// 库内已无计划则清空计划并回到未启动，避免继续用已删计划判断工作时间。
    pub async fn detach_deleted_plan(&self, deleted_id: &str) -> AppResult<EngineStatus> {
        let current_id = {
            let inner = self.inner.lock().await;
            inner.plan.as_ref().map(|p| p.id.clone())
        };
        let Some(current_id) = current_id else {
            return Ok(self.status().await);
        };
        if current_id != deleted_id {
            return Ok(self.status().await);
        }

        let mut inner = self.inner.lock().await;
        self.flush_session(&mut inner, "plan_deleted").await;
        if let Some(b) = inner.break_state.take() {
            drop(inner);
            self.record_break_session(&b, 0, "interrupted").await;
            inner = self.inner.lock().await;
        }

        let plan = db::default_plan(&self.pool).await?;
        match plan {
            Some(p) => {
                inner.machine.set_plan(PlanSpec {
                    work_seconds: p.work_seconds as u64,
                    break_seconds: p.break_seconds as u64,
                    completion_mode: p.completion_mode.clone(),
                    repeat_times: p.repeat_times as u32,
                    repeat_interval_seconds: p.repeat_interval_seconds as u64,
                });
                inner.machine.work_seconds = 0;
                inner.machine.phase = Phase::Idle;
                inner.awaiting_confirm = false;
                inner.suspend_reason = None;
                inner.plan = Some(p.clone());
                inner.schedules = db::list_work_schedules(&self.pool, Some(&p.id))
                    .await
                    .unwrap_or_default();
                let status = Self::status_of(&inner);
                drop(inner);
                let _ = db::set_active_plan(&self.pool, &p.id).await;
                db::record_event(
                    &self.pool,
                    "plan_detached",
                    json!({ "deletedId": deleted_id, "fallbackPlanId": p.id }),
                )
                .await;
                self.emit_status(&status);
                Ok(status)
            }
            None => {
                inner.machine.stop();
                inner.machine.plan = None;
                inner.plan = None;
                inner.schedules = Vec::new();
                inner.awaiting_confirm = false;
                inner.suspend_reason = None;
                let status = Self::status_of(&inner);
                drop(inner);
                let _ = db::set_setting(&self.pool, "activePlanId", json!(null)).await;
                db::record_event(
                    &self.pool,
                    "plan_detached",
                    json!({ "deletedId": deleted_id, "fallbackPlanId": null }),
                )
                .await;
                self.emit_status(&status);
                Ok(status)
            }
        }
    }

    /// 数据被整体重置后，清空当前引擎快照并回退到库内默认计划（重新播种的内置计划）。
    pub async fn reset_to_default(&self) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        self.flush_session(&mut inner, "data_reset").await;
        if let Some(b) = inner.break_state.take() {
            drop(inner);
            self.record_break_session(&b, 0, "interrupted").await;
            inner = self.inner.lock().await;
        }

        let plan = db::default_plan(&self.pool).await?;
        match plan {
            Some(p) => {
                inner.machine.set_plan(PlanSpec {
                    work_seconds: p.work_seconds as u64,
                    break_seconds: p.break_seconds as u64,
                    completion_mode: p.completion_mode.clone(),
                    repeat_times: p.repeat_times as u32,
                    repeat_interval_seconds: p.repeat_interval_seconds as u64,
                });
                inner.machine.work_seconds = 0;
                inner.machine.phase = Phase::Idle;
                inner.awaiting_confirm = false;
                inner.suspend_reason = None;
                inner.plan = Some(p.clone());
                inner.schedules = db::list_work_schedules(&self.pool, Some(&p.id))
                    .await
                    .unwrap_or_default();
                let status = Self::status_of(&inner);
                drop(inner);
                let _ = db::set_active_plan(&self.pool, &p.id).await;
                db::record_event(
                    &self.pool,
                    "data_reset",
                    json!({ "fallbackPlanId": p.id }),
                )
                .await;
                self.emit_status(&status);
                Ok(status)
            }
            None => {
                inner.machine.stop();
                inner.machine.plan = None;
                inner.plan = None;
                inner.schedules = Vec::new();
                inner.awaiting_confirm = false;
                inner.suspend_reason = None;
                let status = Self::status_of(&inner);
                drop(inner);
                let _ = db::set_setting(&self.pool, "activePlanId", json!(null)).await;
                db::record_event(
                    &self.pool,
                    "data_reset",
                    json!({ "fallbackPlanId": null }),
                )
                .await;
                self.emit_status(&status);
                Ok(status)
            }
        }
    }

    /// 勿扰模式：minutes<=0 表示取消
    pub async fn set_dnd(&self, minutes: i64) -> AppResult<EngineStatus> {
        let mut inner = self.inner.lock().await;
        if minutes <= 0 {
            inner.machine.clear_dnd();
        } else {
            inner.machine.set_dnd(minutes as u64 * 60);
        }
        let status = Self::status_of(&inner);
        drop(inner);
        db::record_event(&self.pool, "dnd_changed", json!({ "minutes": minutes })).await;
        self.emit_status(&status);
        Ok(status)
    }

    /// 退出前收尾：保存状态、关闭数据库会话
    pub async fn shutdown(&self) {
        let mut inner = self.inner.lock().await;
        self.flush_session(&mut inner, "app_exit").await;
        if let Some(b) = inner.break_state.take() {
            drop(inner);
            self.record_break_session(&b, 0, "interrupted").await;
            inner = self.inner.lock().await;
        }
        let plan_id = inner.plan.as_ref().map(|p| p.id.clone());
        let work_seconds = inner.machine.work_seconds;
        let phase = inner.machine.phase.as_str().to_string();
        let cycle_index = inner.machine.cycle_index;
        self.persist_checkpoint(plan_id, work_seconds, &phase, cycle_index)
            .await;
        inner.machine.phase = Phase::Idle;
        inner.suspend_reason = None;
    }
}

/// 根据「是否离开座位 / 是否在工作时间」决定本轮要做的挂起动作。
#[derive(Debug, PartialEq, Eq)]
enum PresenceAction {
    None,
    SuspendWork,
    Restore,
}

fn presence_action(phase: Phase, away: bool, in_window: bool) -> PresenceAction {
    match phase {
        Phase::Working if away || !in_window => PresenceAction::SuspendWork,
        Phase::Suspended if !away && in_window => PresenceAction::Restore,
        _ => PresenceAction::None,
    }
}

/// 把秒数格式化成 mm:ss / h:mm:ss。
fn fmt_clock(seconds: i64) -> String {
    let total = seconds.max(0);
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

fn tray_text(status: &EngineStatus) -> String {
    match status.phase.as_str() {
        "working" => format!(
            "SitHalt · 下次提醒 {}",
            fmt_clock(status.work_remaining_seconds)
        ),
        "reminding" => "SitHalt · 该起来活动了".to_string(),
        "break" => format!(
            "SitHalt · 活动中 {}",
            fmt_clock(status.break_remaining_seconds)
        ),
        "paused" => "SitHalt · 已暂停".to_string(),
        "suspended" => match status.suspend_reason.as_deref() {
            Some("idle") => "SitHalt · 已离开座位，计时暂停".to_string(),
            Some("outside_work_hours") => "SitHalt · 当前不在工作时间".to_string(),
            _ => "SitHalt · 已挂起".to_string(),
        },
        _ => "SitHalt · 息坐（未启动）".to_string(),
    }
}

/// 解析 "HH:MM"，失败返回 None。
fn parse_hhmm(text: &str) -> Option<i64> {
    let mut parts = text.split(':');
    let hour: i64 = parts.next()?.trim().parse().ok()?;
    let minute: i64 = parts.next()?.trim().parse().ok()?;
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) {
        return None;
    }
    Some(hour * 60 + minute)
}

/// 判断某个时刻是否落在计划的工作时间内。
/// 规则：若计划配置了工作时间表（`schedules` 中启用的时段），以时间表为准；
/// 否则用计划的 `workDays` + `workStart`/`workEnd`（两者都为空表示全天生效）。
pub(crate) fn in_work_window_at(
    plan: Option<&ReminderPlan>,
    schedules: &[WorkSchedule],
    at: chrono::DateTime<Local>,
) -> bool {
    let Some(plan) = plan else { return true };
    let weekday = at.weekday().number_from_monday() as i64;
    let minute = (at.hour() as i64) * 60 + at.minute() as i64;

    let has_table = schedules.iter().any(|s| s.enabled);
    if has_table {
        return schedules
            .iter()
            .filter(|s| s.enabled && s.weekday == weekday)
            .any(|s| covers_minute(s.start_minute, s.end_minute, minute));
    }

    if !plan.work_days.is_empty() && !plan.work_days.contains(&weekday) {
        return false;
    }
    let (Some(start), Some(end)) = (plan.work_start.as_deref(), plan.work_end.as_deref()) else {
        return true;
    };
    let (Some(start), Some(end)) = (parse_hhmm(start), parse_hhmm(end)) else {
        return true;
    };
    covers_minute(start, end, minute)
}

/// 判断某一分钟是否落在 [start, end) 区间内；end < start 时按跨零点处理。
fn covers_minute(start: i64, end: i64, minute: i64) -> bool {
    if start <= end {
        minute >= start && minute < end
    } else {
        minute >= start || minute < end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn plan_with(days: Vec<i64>, start: Option<&str>, end: Option<&str>) -> ReminderPlan {
        ReminderPlan {
            id: "p".into(),
            name: "测试".into(),
            description: None,
            mode: "interval".into(),
            work_seconds: 60,
            break_seconds: 300,
            reminder_methods: vec![],
            repeat_interval_seconds: 300,
            repeat_times: 2,
            snooze_seconds: 300,
            completion_mode: "auto".into(),
            idle_detection: true,
            idle_threshold_seconds: 180,
            work_start: start.map(|s| s.to_string()),
            work_end: end.map(|s| s.to_string()),
            work_days: days,
            smart_enabled: false,
            is_builtin: false,
            sort_order: 0,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn work_window_respects_weekday_and_hours() {
        // 2026-11-06 是周五，2026-11-07 是周六
        let friday_10 = Local.with_ymd_and_hms(2026, 11, 6, 10, 0, 0).unwrap();
        let friday_20 = Local.with_ymd_and_hms(2026, 11, 6, 20, 0, 0).unwrap();
        let saturday_10 = Local.with_ymd_and_hms(2026, 11, 7, 10, 0, 0).unwrap();

        let working_hours = plan_with(vec![1, 2, 3, 4, 5], Some("09:00"), Some("18:00"));
        assert!(in_work_window_at(Some(&working_hours), &[], friday_10));
        assert!(!in_work_window_at(Some(&working_hours), &[], friday_20));
        assert!(!in_work_window_at(Some(&working_hours), &[], saturday_10));

        let all_day = plan_with(vec![], None, None);
        assert!(in_work_window_at(Some(&all_day), &[], saturday_10));
    }

    fn schedule(weekday: i64, start: i64, end: i64, enabled: bool) -> WorkSchedule {
        WorkSchedule {
            id: format!("s-{weekday}-{start}-{end}"),
            plan_id: "p".into(),
            weekday,
            start_minute: start,
            end_minute: end,
            enabled,
            created_at: String::new(),
        }
    }

    #[test]
    fn work_schedule_table_overrides_plan_fields() {
        let friday_10 = Local.with_ymd_and_hms(2026, 11, 6, 10, 0, 0).unwrap();
        let friday_13 = Local.with_ymd_and_hms(2026, 11, 6, 13, 0, 0).unwrap();
        // 计划本身写的是全天运行；时间表限定为上午 9-12 点
        let plan = plan_with(vec![], None, None);
        let table = vec![schedule(5, 9 * 60, 12 * 60, true)];
        assert!(in_work_window_at(Some(&plan), &table, friday_10));
        assert!(!in_work_window_at(Some(&plan), &table, friday_13));
    }

    #[test]
    fn disabled_schedule_falls_back_to_plan_window() {
        let friday_13 = Local.with_ymd_and_hms(2026, 11, 6, 13, 0, 0).unwrap();
        let plan = plan_with(vec![1, 2, 3, 4, 5], Some("09:00"), Some("18:00"));
        // 时间表全部禁用 → 退回计划字段，13 点仍在工作时间内
        let table = vec![schedule(5, 9 * 60, 12 * 60, false)];
        assert!(in_work_window_at(Some(&plan), &table, friday_13));
    }

    #[test]
    fn work_window_supports_overnight_range() {
        let plan = plan_with(vec![], Some("22:00"), Some("02:00"));
        let late = Local.with_ymd_and_hms(2026, 11, 6, 23, 30, 0).unwrap();
        let early = Local.with_ymd_and_hms(2026, 11, 6, 1, 30, 0).unwrap();
        let noon = Local.with_ymd_and_hms(2026, 11, 6, 12, 0, 0).unwrap();
        assert!(in_work_window_at(Some(&plan), &[], late));
        assert!(in_work_window_at(Some(&plan), &[], early));
        assert!(!in_work_window_at(Some(&plan), &[], noon));
    }

    #[test]
    fn presence_action_covers_idle_and_work_window() {
        // 计时中离开座位 / 超出工作时间 → 挂起
        assert_eq!(
            presence_action(Phase::Working, true, true),
            PresenceAction::SuspendWork
        );
        assert_eq!(
            presence_action(Phase::Working, false, false),
            PresenceAction::SuspendWork
        );
        // 计时中人在座位且在工作时间 → 不做任何处理
        assert_eq!(
            presence_action(Phase::Working, false, true),
            PresenceAction::None
        );
        // 挂起后回到座位 + 回到工作时间 → 恢复
        assert_eq!(
            presence_action(Phase::Suspended, false, true),
            PresenceAction::Restore
        );
        // 挂起后仍有任一条件不满足 → 继续挂起
        assert_eq!(
            presence_action(Phase::Suspended, true, true),
            PresenceAction::None
        );
        assert_eq!(
            presence_action(Phase::Suspended, false, false),
            PresenceAction::None
        );
        // 提醒中不挂起，避免用户还没看到提醒就被吞掉
        assert_eq!(
            presence_action(Phase::Reminding, true, false),
            PresenceAction::None
        );
        // 主动暂停不会被自动恢复
        assert_eq!(
            presence_action(Phase::Paused, false, true),
            PresenceAction::None
        );
    }

    #[test]
    fn tray_text_reflects_phase() {
        let mut status = EngineStatus::default();
        status.phase = "working".into();
        status.work_remaining_seconds = 754;
        assert_eq!(tray_text(&status), "SitHalt · 下次提醒 12:34");

        status.phase = "suspended".into();
        status.suspend_reason = Some("idle".into());
        assert_eq!(tray_text(&status), "SitHalt · 已离开座位，计时暂停");
    }
}
