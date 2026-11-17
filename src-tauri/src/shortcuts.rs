//! 全局快捷键：四个动作可绑定到全局热键，支持改绑、冲突检测与恢复默认。
//!
//! 绑定关系存于 `app_settings` 的 `shortcuts` 键（JSON 对象：action -> accelerator）。
//! 插件只负责注册与派发，具体动作复用引擎与主窗口句柄。

use serde_json::{json, Map, Value};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// 可绑定的动作标识（前后端约定一致）。
pub const ACTIONS: [(&str, &str); 4] = [
    ("toggleMain", "显示 / 隐藏主界面"),
    ("pauseResume", "暂停 / 恢复提醒"),
    ("startBreak", "立即活动"),
    ("dnd", "开启 / 关闭勿扰"),
];

/// 缺省热键（仅在设置缺失或恢复默认时使用）。
/// macOS 用 Command(Meta) + Option；Windows / Linux 用 Ctrl + Alt。
#[cfg(target_os = "macos")]
const DEFAULTS: [(&str, &str); 4] = [
    ("toggleMain", "Meta+Alt+S"),
    ("pauseResume", "Meta+Alt+P"),
    ("startBreak", "Meta+Alt+B"),
    ("dnd", "Meta+Alt+D"),
];

#[cfg(not(target_os = "macos"))]
const DEFAULTS: [(&str, &str); 4] = [
    ("toggleMain", "Ctrl+Alt+S"),
    ("pauseResume", "Ctrl+Alt+P"),
    ("startBreak", "Ctrl+Alt+B"),
    ("dnd", "Ctrl+Alt+D"),
];

fn default_map() -> Map<String, Value> {
    let mut map = Map::new();
    for (action, combo) in DEFAULTS {
        map.insert(action.to_string(), json!(combo));
    }
    map
}

/// 读取当前绑定：以默认值为底，用已保存的覆盖；缺动作补默认。
pub async fn load_map(pool: &SqlitePool) -> Map<String, Value> {
    let mut map = default_map();
    if let Ok(Some(raw)) = db::get_setting_raw(pool, "shortcuts").await {
        if let Ok(Value::Object(saved)) = serde_json::from_str::<Value>(&raw) {
            for (action, combo) in saved {
                if let Some(s) = combo.as_str() {
                    map.insert(action, json!(s));
                }
            }
        }
    }
    map
}

fn combo_of(map: &Map<String, Value>, action: &str) -> Option<String> {
    map.get(action).and_then(|v| v.as_str()).map(str::to_string)
}

/// 触发某个动作：在独立异步任务里操作引擎或主窗口，不阻塞热键回调线程。
fn dispatch(app: &AppHandle, action: String) {
    let handle = app.clone();
    let state = app.state::<AppState>();
    let engine = state.engine.clone();
    tauri::async_runtime::spawn(async move {
        match action.as_str() {
            "toggleMain" => {
                if let Some(win) = handle.get_webview_window("main") {
                    let visible = win.is_visible().unwrap_or(false);
                    if visible {
                        let _ = win.hide();
                    } else {
                        let _ = win.show();
                        let _ = win.unminimize();
                        let _ = win.set_focus();
                    }
                }
            }
            "pauseResume" => {
                let status = engine.status().await;
                match status.phase.as_str() {
                    "working" | "reminding" | "break" => {
                        let _ = engine.pause().await;
                    }
                    "paused" | "suspended" => {
                        let _ = engine.resume().await;
                    }
                    _ => {}
                }
            }
            "startBreak" => {
                let _ = engine.start_break(None).await;
            }
            "dnd" => {
                let status = engine.status().await;
                let in_dnd = status.dnd_remaining_seconds.map_or(false, |s| s > 0);
                let _ = engine.set_dnd(if in_dnd { 0 } else { 30 }).await;
            }
            _ => {}
        }
    });
}

fn register_one(app: &AppHandle, action: &str, combo: &str) {
    let gs = app.global_shortcut();
    let action_owned = action.to_string();
    let action_for_err = action_owned.clone();
    let app_clone = app.clone();
    if let Err(err) = gs.on_shortcut(combo, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            dispatch(&app_clone, action_owned.clone());
        }
    }) {
        eprintln!("[shortcut] 注册 {action_for_err} ({combo}) 失败：{err}");
    }
}

/// 启动时按已保存绑定注册全部热键；单个注册失败不阻断应用启动。
pub async fn register_initial(app: &AppHandle, pool: &SqlitePool) {
    let map = load_map(pool).await;
    for (action, combo) in &map {
        if let Some(combo) = combo.as_str() {
            register_one(app, action, combo);
        }
    }
}

/// 改绑：校验动作与冲突 → 注销旧键 → 注册新键（失败回滚旧键）→ 落库。
pub async fn set_shortcut(
    app: &AppHandle,
    pool: &SqlitePool,
    action: &str,
    combo: &str,
) -> AppResult<Map<String, Value>> {
    if !ACTIONS.iter().any(|(a, _)| *a == action) {
        return Err(AppError::Invalid(format!("未知快捷键动作：{action}")));
    }
    let combo = combo.trim();
    if combo.is_empty() {
        return Err(AppError::Invalid("快捷键不能为空".into()));
    }

    let mut map = load_map(pool).await;

    // 同动作改绑：先记录旧键；跨动作重复即冲突。
    let old = combo_of(&map, action);
    for (other_action, other_combo) in &map {
        if other_action != action && other_combo.as_str() == Some(combo) {
            return Err(AppError::Invalid(format!(
                "该快捷键已绑定到「{}」",
                ACTIONS
                    .iter()
                    .find(|(a, _)| *a == other_action.as_str())
                    .map(|(_, label)| *label)
                    .unwrap_or(other_action)
            )));
        }
    }

    let gs = app.global_shortcut();
    if let Some(old) = &old {
        let _ = gs.unregister(old.as_str());
    }

    let action_owned = action.to_string();
    let app_clone = app.clone();
    if let Err(err) = gs.on_shortcut(combo, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            dispatch(&app_clone, action_owned.clone());
        }
    }) {
        // 注册失败（非法组合或被其它应用占用）：回滚旧键并报错。
        if let Some(old) = &old {
            register_one(app, action, old);
        }
        return Err(AppError::Invalid(format!("快捷键注册失败：{err}")));
    }

    map.insert(action.to_string(), json!(combo));
    db::set_setting(pool, "shortcuts", Value::Object(map.clone())).await?;
    Ok(map)
}

/// 恢复默认：注销全部，按默认值重新注册并落库。
pub async fn reset_shortcuts(app: &AppHandle, pool: &SqlitePool) -> AppResult<Map<String, Value>> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let map = default_map();
    for (action, combo) in &map {
        register_one(app, action, combo.as_str().unwrap_or_default());
    }
    db::set_setting(pool, "shortcuts", Value::Object(map.clone())).await?;
    Ok(map)
}
