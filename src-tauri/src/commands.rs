use serde_json::Value;
use tauri::{Manager, State};

use crate::db;
use crate::error::AppResult;
use crate::models::{
    Activity, ActivityInput, ActivitySession, AppInfo, BackupInfo, DbHealth, EngineStatus,
    ReminderPlan, ReminderPlanInput, SittingSession, TodayStats, WorkSchedule, WorkScheduleInput,
};
use crate::platform;
use crate::shortcuts;
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
        author: "zeMinng".into(),
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
    let saved = db::save_plan(&state.db, plan).await?;
    // 若编辑的是当前在用计划，刷新引擎内存快照，避免旧工作日/时段继续参与工作时间判断
    let _ = state.engine.reload_plan_if_active(&saved.id).await;
    Ok(saved)
}

#[tauri::command]
pub async fn delete_plan(state: State<'_, AppState>, id: String) -> AppResult<()> {
    db::delete_plan(&state.db, &id).await?;
    // 若删的是当前在用计划，引擎回退到默认计划（或清空），避免继续引用已删快照
    let _ = state.engine.detach_deleted_plan(&id).await;
    Ok(())
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

#[tauri::command]
pub async fn restore_backup(
    state: State<'_, AppState>,
    file: String,
) -> AppResult<BackupInfo> {
    db::prepare_restore(&state.db, &state.db_path, &file).await
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
    from: Option<String>,
    to: Option<String>,
) -> AppResult<Vec<ActivitySession>> {
    db::history_sessions(
        &state.db,
        limit.unwrap_or(50).clamp(1, 500),
        offset.unwrap_or(0),
        category,
        keyword,
        completion,
        from,
        to,
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

// ---------------------------------------------------------------- 全局快捷键

#[tauri::command]
pub async fn get_shortcuts(state: State<'_, AppState>) -> AppResult<Value> {
    Ok(Value::Object(shortcuts::load_map(&state.db).await))
}

#[tauri::command]
pub async fn set_shortcut(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action: String,
    combo: String,
) -> AppResult<Value> {
    let map = shortcuts::set_shortcut(&app, &state.db, &action, &combo).await?;
    Ok(Value::Object(map))
}

#[tauri::command]
pub async fn reset_shortcuts(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Value> {
    let map = shortcuts::reset_shortcuts(&app, &state.db).await?;
    Ok(Value::Object(map))
}

// ---------------------------------------------------------------- 数据导出 / 导入

/// 把全部业务数据导出为 JSON，写入前端对话框选定的目标文件。
#[tauri::command]
pub async fn export_json(state: State<'_, AppState>, target: String) -> AppResult<()> {
    let data = db::export_all_data(&state.db).await?;
    let text = serde_json::to_string_pretty(&data)
        .map_err(|e| crate::error::AppError::Invalid(format!("序列化导出数据失败：{e}")))?;
    std::fs::write(&target, text)?;
    Ok(())
}

/// 把活动记录导出为 CSV，写入前端对话框选定的目标文件。
#[tauri::command]
pub async fn export_csv(state: State<'_, AppState>, target: String) -> AppResult<()> {
    let csv = db::export_activity_csv(&state.db).await?;
    std::fs::write(&target, csv)?;
    Ok(())
}

/// 从 JSON 文件导入全部业务数据；导入前自动兜底备份当前库，失败时事务回滚、原始数据保留。
#[tauri::command]
pub async fn import_json(
    state: State<'_, AppState>,
    source: String,
) -> AppResult<BackupInfo> {
    let text = std::fs::read_to_string(&source)?;
    let payload: Value = serde_json::from_str(&text)
        .map_err(|e| crate::error::AppError::Invalid(format!("JSON 解析失败：{e}")))?;

    let keep = db::get_setting_raw(&state.db, "backupKeep")
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<i64>(&raw).ok())
        .unwrap_or(5)
        .max(1) as usize;
    let fallback = db::backup_now(&state.db, &state.db_path, keep).await?;
    db::import_all_data(&state.db, &payload).await?;
    db::record_event(
        &state.db,
        "data_imported",
        serde_json::json!({ "source": source }),
    )
    .await;
    Ok(fallback)
}

/// 恢复出厂：清空全部业务数据并重新播种内置计划 / 活动 / 默认设置；引擎快照回退到新默认计划。不做备份。
#[tauri::command]
pub async fn reset_all_data(state: State<'_, AppState>) -> AppResult<EngineStatus> {
    db::reset_all_data(&state.db).await?;
    state.engine.reset_to_default().await
}
