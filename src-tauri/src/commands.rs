use serde_json::Value;
use tauri::{Manager, State};

use crate::db;
use crate::error::AppResult;
use crate::models::{
    Activity, ActivityInput, ActivitySession, AppInfo, BackupInfo, DbHealth, EngineStatus,
    ReminderPlan, ReminderPlanInput, SittingSession, TodayStats, WorkSchedule, WorkScheduleInput,
};
use crate::platform;
use crate::state::AppState;

#[tauri::command]
pub fn app_info(state: State<'_, AppState>, app: tauri::AppHandle) -> AppInfo {
    let data_dir = app
        .path()
        .app_data_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| String::from("unknown"));
    AppInfo {
        name: "SitHalt".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        identifier: app.config().identifier.clone(),
        db_path: state.db_path.display().to_string(),
        data_dir,
        platform: platform::platform_name().into(),
    }
}

#[tauri::command]
pub async fn db_health(state: State<'_, AppState>) -> AppResult<DbHealth> {
    db::health(&state.db, &state.db_path).await
}

// ---------------------------------------------------------------- 设置

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Value> {
    db::get_settings(&state.db).await
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: Value,
) -> AppResult<Value> {
    db::set_setting(&state.db, &key, value).await?;
    db::get_settings(&state.db).await
}

// ---------------------------------------------------------------- 提醒计划

#[tauri::command]
pub async fn list_plans(state: State<'_, AppState>) -> AppResult<Vec<ReminderPlan>> {
    db::list_plans(&state.db).await
}

#[tauri::command]
pub async fn save_plan(
    state: State<'_, AppState>,
    plan: ReminderPlanInput,
) -> AppResult<ReminderPlan> {
    db::save_plan(&state.db, plan).await
}

#[tauri::command]
pub async fn delete_plan(state: State<'_, AppState>, id: String) -> AppResult<()> {
    db::delete_plan(&state.db, &id).await
}

#[tauri::command]
pub async fn activate_plan(state: State<'_, AppState>, id: String) -> AppResult<EngineStatus> {
    state.engine.switch_plan(id).await
}

// ---------------------------------------------------------------- 工作时间表

#[tauri::command]
pub async fn list_work_schedules(
    state: State<'_, AppState>,
    plan_id: Option<String>,
) -> AppResult<Vec<WorkSchedule>> {
    db::list_work_schedules(&state.db, plan_id.as_deref()).await
}

#[tauri::command]
pub async fn save_work_schedule(
    state: State<'_, AppState>,
    schedule: WorkScheduleInput,
) -> AppResult<WorkSchedule> {
    let saved = db::save_work_schedule(&state.db, schedule).await?;
    let _ = state.engine.reload_schedules().await;
    Ok(saved)
}

#[tauri::command]
pub async fn delete_work_schedule(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<EngineStatus> {
    db::delete_work_schedule(&state.db, &id).await?;
    state.engine.reload_schedules().await
}

// ---------------------------------------------------------------- 备份

#[tauri::command]
pub async fn backup_now(state: State<'_, AppState>) -> AppResult<BackupInfo> {
    let keep = db::get_setting_raw(&state.db, "backupKeep")
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<i64>(&raw).ok())
        .unwrap_or(5)
        .max(1) as usize;
    db::backup_now(&state.db, &state.db_path, keep).await
}

#[tauri::command]
pub async fn list_backups(state: State<'_, AppState>) -> AppResult<Vec<BackupInfo>> {
    db::list_backups(&state.db_path)
}

// ---------------------------------------------------------------- 活动库

#[tauri::command]
pub async fn list_activities(state: State<'_, AppState>) -> AppResult<Vec<Activity>> {
    db::list_activities(&state.db).await
}

#[tauri::command]
pub async fn save_activity(
    state: State<'_, AppState>,
    activity: ActivityInput,
) -> AppResult<Activity> {
    db::save_activity(&state.db, activity).await
}

#[tauri::command]
pub async fn delete_activity(state: State<'_, AppState>, id: String) -> AppResult<()> {
    db::delete_activity(&state.db, &id).await
}

// ---------------------------------------------------------------- 计时引擎

#[tauri::command]
pub async fn engine_status(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    Ok(state.engine.status().await)
}

#[tauri::command]
pub async fn engine_start(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    state.engine.start().await
}

#[tauri::command]
pub async fn engine_pause(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    state.engine.pause().await
}

#[tauri::command]
pub async fn engine_resume(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    state.engine.resume().await
}

#[tauri::command]
pub async fn engine_stop(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    state.engine.stop().await
}

#[tauri::command]
pub async fn engine_snooze(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    state.engine.snooze().await
}

#[tauri::command]
pub async fn engine_skip(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    state.engine.skip().await
}

#[tauri::command]
pub async fn engine_acknowledge(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    state.engine.acknowledge().await
}

#[tauri::command]
pub async fn engine_start_break(
    state: State<'_, AppState>,
    activity_id: Option<String>,
) -> AppResult<EngineStatus> {
    state.engine.start_break(activity_id).await
}

#[tauri::command]
pub async fn engine_finish_break(
    state: State<'_, AppState>,
    completed: bool,
) -> AppResult<EngineStatus> {
    state.engine.finish_break(completed).await
}

#[tauri::command]
pub async fn engine_set_dnd(state: State<'_, AppState>, minutes: i64) -> AppResult<EngineStatus> {
    state.engine.set_dnd(minutes).await
}

// ---------------------------------------------------------------- 统计

#[tauri::command]
pub async fn today_stats(state: State<'_, AppState>) -> AppResult<TodayStats> {
    db::today_stats(&state.db).await
}

#[tauri::command]
pub async fn trend(
    state: State<'_, AppState>,
    days: Option<i64>,
) -> AppResult<Vec<crate::models::DailyPoint>> {
    db::trend_points(&state.db, days.unwrap_or(7).clamp(1, 90)).await
}

#[tauri::command]
pub async fn month_summary(state: State<'_, AppState>) -> AppResult<Value> {
    db::month_summary(&state.db).await
}

#[tauri::command]
pub async fn history_sessions(
    state: State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    category: Option<String>,
    keyword: Option<String>,
    completion: Option<String>,
) -> AppResult<Vec<ActivitySession>> {
    db::history_sessions(
        &state.db,
        limit.unwrap_or(50).clamp(1, 500),
        offset.unwrap_or(0),
        category,
        keyword,
        completion,
    )
    .await
}

#[tauri::command]
pub async fn sitting_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<SittingSession>> {
    db::sitting_history(&state.db, limit.unwrap_or(50).clamp(1, 500)).await
}

/// 供界面调试使用：查询指定表行数。
#[tauri::command]
pub async fn table_count(state: State<'_, AppState>, table: String) -> AppResult<i64> {
    db::count_rows(&state.db, &table).await
}

/// 开机自动启动（PRD 4.7.3）。由插件写入系统启动项，失败时返回明确错误而不是让应用崩溃。
#[tauri::command]
pub async fn set_autostart(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    enabled: bool,
) -> AppResult<bool> {
    use tauri_plugin_autostart::ManagerExt;

    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| crate::error::AppError::State(format!("开机自启设置失败：{e}")))?;
    let now_enabled = manager.is_enabled().unwrap_or(enabled);
    db::set_setting(&state.db, "autostart", serde_json::json!(now_enabled)).await?;
    Ok(now_enabled)
}
