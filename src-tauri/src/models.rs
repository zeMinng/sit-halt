use serde::{Deserialize, Serialize};

/// 提醒计划的对外 DTO：`reminderMethods` / `workDays` 以数组形式暴露给前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderPlan {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// interval | pomodoro | smart | custom
    pub mode: String,
    pub work_seconds: i64,
    pub break_seconds: i64,
    pub reminder_methods: Vec<String>,
    pub repeat_interval_seconds: i64,
    pub repeat_times: i64,
    pub snooze_seconds: i64,
    /// auto | manual | ask
    pub completion_mode: String,
    pub idle_detection: bool,
    pub idle_threshold_seconds: i64,
    pub work_start: Option<String>,
    pub work_end: Option<String>,
    pub work_days: Vec<i64>,
    pub smart_enabled: bool,
    pub is_builtin: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 数据库原始行：列表字段以 JSON 文本存储，避免核心业务字段全部塞进 JSON。
#[derive(Debug, sqlx::FromRow)]
pub struct ReminderPlanRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub mode: String,
    pub work_seconds: i64,
    pub break_seconds: i64,
    pub reminder_methods: String,
    pub repeat_interval_seconds: i64,
    pub repeat_times: i64,
    pub snooze_seconds: i64,
    pub completion_mode: String,
    pub idle_detection: bool,
    pub idle_threshold_seconds: i64,
    pub work_start: Option<String>,
    pub work_end: Option<String>,
    pub work_days: String,
    pub smart_enabled: bool,
    pub is_builtin: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl ReminderPlanRow {
    pub fn into_dto(self) -> ReminderPlan {
        ReminderPlan {
            id: self.id,
            name: self.name,
            description: self.description,
            mode: self.mode,
            work_seconds: self.work_seconds,
            break_seconds: self.break_seconds,
            reminder_methods: serde_json::from_str(&self.reminder_methods).unwrap_or_default(),
            repeat_interval_seconds: self.repeat_interval_seconds,
            repeat_times: self.repeat_times,
            snooze_seconds: self.snooze_seconds,
            completion_mode: self.completion_mode,
            idle_detection: self.idle_detection,
            idle_threshold_seconds: self.idle_threshold_seconds,
            work_start: self.work_start,
            work_end: self.work_end,
            work_days: serde_json::from_str(&self.work_days).unwrap_or_default(),
            smart_enabled: self.smart_enabled,
            is_builtin: self.is_builtin,
            sort_order: self.sort_order,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// 新建 / 更新计划时前端提交的内容（id 由后端生成或沿用）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderPlanInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_mode")]
    pub mode: String,
    pub work_seconds: i64,
    pub break_seconds: i64,
    #[serde(default)]
    pub reminder_methods: Vec<String>,
    #[serde(default = "default_repeat_interval")]
    pub repeat_interval_seconds: i64,
    #[serde(default = "default_repeat_times")]
    pub repeat_times: i64,
    #[serde(default = "default_snooze")]
    pub snooze_seconds: i64,
    #[serde(default = "default_completion_mode")]
    pub completion_mode: String,
    #[serde(default = "default_true")]
    pub idle_detection: bool,
    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_seconds: i64,
    #[serde(default)]
    pub work_start: Option<String>,
    #[serde(default)]
    pub work_end: Option<String>,
    #[serde(default = "default_work_days")]
    pub work_days: Vec<i64>,
    #[serde(default)]
    pub smart_enabled: bool,
    #[serde(default)]
    pub sort_order: i64,
}

fn default_mode() -> String {
    "interval".into()
}
fn default_repeat_interval() -> i64 {
    300
}
fn default_repeat_times() -> i64 {
    2
}
fn default_snooze() -> i64 {
    300
}
fn default_completion_mode() -> String {
    "auto".into()
}
fn default_true() -> bool {
    true
}
fn default_idle_threshold() -> i64 {
    180
}
fn default_work_days() -> Vec<i64> {
    vec![1, 2, 3, 4, 5]
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRow {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub duration_seconds: i64,
    pub steps: String,
    pub image: Option<String>,
    pub cautions: Option<String>,
    pub enabled: bool,
    pub is_builtin: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub duration_seconds: i64,
    pub steps: Vec<String>,
    pub image: Option<String>,
    pub cautions: Option<String>,
    pub enabled: bool,
    pub is_builtin: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl ActivityRow {
    pub fn into_dto(self) -> Activity {
        Activity {
            id: self.id,
            name: self.name,
            category: self.category,
            description: self.description,
            duration_seconds: self.duration_seconds,
            steps: serde_json::from_str(&self.steps).unwrap_or_default(),
            image: self.image,
            cautions: self.cautions,
            enabled: self.enabled,
            is_builtin: self.is_builtin,
            sort_order: self.sort_order,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityInput {
    pub id: Option<String>,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub description: Option<String>,
    pub duration_seconds: i64,
    #[serde(default)]
    pub steps: Vec<String>,
    #[serde(default)]
    pub cautions: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySessionRow {
    pub id: String,
    pub activity_id: Option<String>,
    pub activity_name: String,
    pub category: Option<String>,
    pub plan_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: i64,
    pub planned_seconds: i64,
    pub completion: String,
    pub source: String,
    pub created_at: String,
}

/// 活动 / 久坐记录在 UI 上的展示模型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySession {
    pub id: String,
    pub activity_id: Option<String>,
    pub activity_name: String,
    pub category: Option<String>,
    pub plan_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: i64,
    pub planned_seconds: i64,
    pub completion: String,
    pub source: String,
    pub created_at: String,
}

impl From<ActivitySessionRow> for ActivitySession {
    fn from(row: ActivitySessionRow) -> Self {
        Self {
            id: row.id,
            activity_id: row.activity_id,
            activity_name: row.activity_name,
            category: row.category,
            plan_id: row.plan_id,
            started_at: row.started_at,
            ended_at: row.ended_at,
            duration_seconds: row.duration_seconds,
            planned_seconds: row.planned_seconds,
            completion: row.completion,
            source: row.source,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WorkSchedule {
    pub id: String,
    pub plan_id: String,
    /// 1 = 周一 … 7 = 周日
    pub weekday: i64,
    /// 当天的第几分钟（0–1440），与 end_minute 一起表示一个工作时段
    pub start_minute: i64,
    pub end_minute: i64,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkScheduleInput {
    pub id: Option<String>,
    pub plan_id: String,
    pub weekday: i64,
    pub start_minute: i64,
    pub end_minute: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 数据库备份文件的对外描述。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file: String,
    pub size_bytes: i64,
    pub created_at: String,
}

/// 一段久坐会话（用于历史列表展示）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SittingSession {
    pub id: String,
    pub plan_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: i64,
    pub interrupted_by: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DailyPoint {
    pub date: String,
    pub sitting_seconds: i64,
    pub activity_seconds: i64,
    pub activity_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TodayStats {
    pub date: String,
    pub sitting_seconds: i64,
    pub activity_count: i64,
    pub activity_seconds: i64,
    pub longest_sitting_seconds: i64,
    pub expected_activities: i64,
    /// 没有应完成活动时为 None，前端显示“暂无数据”而不是 0%。
    pub completion_rate: Option<f64>,
    pub trend: Vec<DailyPoint>,
    pub recent_activities: Vec<ActivitySession>,
}

/// 计时状态机的对外快照，每秒通过 `engine:status` 事件推给前端。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// idle | working | reminding | break | paused | suspended
    pub phase: String,
    pub plan_id: Option<String>,
    pub plan_name: Option<String>,
    pub work_seconds: i64,
    pub work_target_seconds: i64,
    pub work_remaining_seconds: i64,
    pub break_seconds: i64,
    pub break_target_seconds: i64,
    pub break_remaining_seconds: i64,
    pub activity_id: Option<String>,
    pub activity_name: Option<String>,
    pub completion_mode: Option<String>,
    pub reminder_elapsed_seconds: i64,
    pub snooze_remaining_seconds: Option<i64>,
    pub dnd_remaining_seconds: Option<i64>,
    pub cycle_index: i64,
    pub ignored_count: i64,
    pub can_resume: bool,
    /// 系统空闲秒数（键鼠无操作时长），无法获取时为 None
    pub idle_seconds: Option<i64>,
    /// 挂起原因：idle（离开座位）| outside_work_hours（不在工作时间）
    pub suspend_reason: Option<String>,
    /// 当前是否处于计划设定的工作时间内
    pub in_work_window: bool,
    /// 当前计划配置的工作时段条数（0 表示沿用计划的起止时间）
    pub schedule_count: i64,
    pub server_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub identifier: String,
    pub db_path: String,
    pub data_dir: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbHealth {
    pub ok: bool,
    pub db_path: String,
    pub migration_count: i64,
    pub table_count: i64,
    pub server_time: String,
}
