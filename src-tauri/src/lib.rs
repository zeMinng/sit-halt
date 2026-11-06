mod commands;
mod db;
mod engine;
mod error;
mod models;
mod platform;
mod state;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
use tauri_plugin_notification::NotificationExt;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let handle = app.handle().clone();

            // 1. 初始化 SQLite（文件落在 app_data_dir）+ 执行迁移
            let database = tauri::async_runtime::block_on(db::init(&handle))?;

            // 2. 收尾上次异常退出遗留的会话，避免把离线时间计入久坐
            tauri::async_runtime::block_on(db::close_dangling_sessions(&database.pool));
            tauri::async_runtime::block_on(db::record_event(
                &database.pool,
                "app_started",
                serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }),
            ));

            // 2.1 按设置决定是否需要做一次数据库备份（缺省 7 天一次、保留 5 份）
            {
                let pool = database.pool.clone();
                let path = database.path.clone();
                tauri::async_runtime::spawn(async move {
                    db::maybe_auto_backup(&pool, &path).await;
                });
            }

            // 3. 启动 Rust 计时引擎与每秒 tick 循环
            let engine = tauri::async_runtime::block_on(engine::Engine::new(
                handle.clone(),
                database.pool.clone(),
            ));
            engine.spawn_ticker();

            app.manage(AppState {
                db: database.pool.clone(),
                db_path: database.path.clone(),
                engine,
            });

            // 4. 系统托盘
            build_tray(app)?;

            // 5. 关闭窗口 = 隐藏到托盘，应用与提醒引擎继续运行（PRD 4.7.2）
            if let Some(window) = app.get_webview_window("main") {
                let win = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::db_health,
            commands::get_settings,
            commands::set_setting,
            commands::list_plans,
            commands::save_plan,
            commands::delete_plan,
            commands::activate_plan,
            commands::list_work_schedules,
            commands::save_work_schedule,
            commands::delete_work_schedule,
            commands::backup_now,
            commands::list_backups,
            commands::list_activities,
            commands::save_activity,
            commands::delete_activity,
            commands::engine_status,
            commands::engine_start,
            commands::engine_pause,
            commands::engine_resume,
            commands::engine_stop,
            commands::engine_snooze,
            commands::engine_skip,
            commands::engine_acknowledge,
            commands::engine_start_break,
            commands::engine_finish_break,
            commands::engine_set_dnd,
            commands::today_stats,
            commands::trend,
            commands::month_summary,
            commands::history_sessions,
            commands::sitting_history,
            commands::table_count,
            commands::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("启动 SitHalt 失败");
}

fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "打开主界面", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "start", "开始提醒", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "暂停提醒", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "恢复提醒", true, None::<&str>)?;
    let activity = MenuItem::with_id(app, "activity", "立即活动", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "查看下一次提醒", true, None::<&str>)?;
    let dnd = MenuItem::with_id(app, "dnd", "勿扰 30 分钟", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出应用", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show,
            &PredefinedMenuItem::separator(app)?,
            &start,
            &pause,
            &resume,
            &activity,
            &next,
            &PredefinedMenuItem::separator(app)?,
            &dnd,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let icon = app.default_window_icon().cloned();

    let mut builder = TrayIconBuilder::with_id("sithalt-tray")
        .tooltip("SitHalt · 息坐")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let engine = state.engine.clone();
                let result = match id.as_str() {
                    "show" => {
                        show_main_window(&handle);
                        Ok(())
                    }
                    "start" => engine.start().await.map(|_| ()),
                    "pause" => engine.pause().await.map(|_| ()),
                    "resume" => engine.resume().await.map(|_| ()),
                    "activity" => engine.start_break(None).await.map(|_| ()),
                    "next" => {
                        let status = engine.status().await;
                        let body = match status.phase.as_str() {
                            "working" => format!(
                                "下次提醒还有 {}，本轮已坐 {}。",
                                humanize(status.work_remaining_seconds),
                                humanize(status.work_seconds)
                            ),
                            "reminding" => "现在就是提醒时间，起来活动一下。".to_string(),
                            "break" => format!(
                                "活动进行中，剩余 {}。",
                                humanize(status.break_remaining_seconds)
                            ),
                            "suspended" => match status.suspend_reason.as_deref() {
                                Some("idle") => "你已离开座位，计时已暂停。".to_string(),
                                Some(_) => "当前不在计划设定的工作时间。".to_string(),
                                None => "当前已挂起计时。".to_string(),
                            },
                            "paused" => "计时已暂停，恢复后继续。".to_string(),
                            _ => "当前没有正在进行的计时，点「开始提醒」即可。".to_string(),
                        };
                        let _ = handle
                            .notification()
                            .builder()
                            .title("SitHalt · 下一次提醒")
                            .body(body)
                            .show();
                        Ok(())
                    }
                    "dnd" => engine.set_dnd(30).await.map(|_| ()),
                    "quit" => {
                        engine.shutdown().await;
                        handle.exit(0);
                        Ok(())
                    }
                    _ => Ok(()),
                };
                if let Err(err) = result {
                    eprintln!("[tray] 操作失败：{err}");
                }
            });
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 把秒数写成「X 分 Y 秒」，用于通知文案。
fn humanize(seconds: i64) -> String {
    let total = seconds.max(0);
    let minutes = total / 60;
    let secs = total % 60;
    if minutes > 0 {
        format!("{minutes} 分 {secs} 秒")
    } else {
        format!("{secs} 秒")
    }
}
