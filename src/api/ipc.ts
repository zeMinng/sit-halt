/**
 * 前后端通信的唯一出口。前端不直接访问数据库，
 * 所有读写都通过 Rust command（PRD 9.2 职责划分）。
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  Activity,
  AppInfo,
  AppSettings,
  ActivitySession,
  BackupInfo,
  DailyPoint,
  DbHealth,
  EngineStatus,
  MonthSummary,
  ReminderPlan,
  ReminderPlanInput,
  SittingSession,
  TodayStats,
  WorkSchedule,
  WorkScheduleInput,
} from "@/types/contracts";

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** 浏览器里直接 `pnpm dev` 预览时给出空实现，避免白屏，实际功能需要 Tauri 运行时。 */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) {
    throw new Error(`需要在 Tauri 桌面环境中运行（缺失 command: ${cmd}）`);
  }
  return invoke<T>(cmd, args);
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  dbHealth: () => call<DbHealth>("db_health"),

  getSettings: () => call<AppSettings>("get_settings"),
  setSetting: (key: string, value: unknown) =>
    call<AppSettings>("set_setting", { key, value }),

  listPlans: () => call<ReminderPlan[]>("list_plans"),
  savePlan: (plan: ReminderPlanInput) => call<ReminderPlan>("save_plan", { plan }),
  deletePlan: (id: string) => call<void>("delete_plan", { id }),
  activatePlan: (id: string) => call<EngineStatus>("activate_plan", { id }),

  listWorkSchedules: (planId?: string | null) =>
    call<WorkSchedule[]>("list_work_schedules", { planId: planId ?? null }),
  saveWorkSchedule: (schedule: WorkScheduleInput) =>
    call<WorkSchedule>("save_work_schedule", { schedule }),
  deleteWorkSchedule: (id: string) => call<EngineStatus>("delete_work_schedule", { id }),

  backupNow: () => call<BackupInfo>("backup_now"),
  listBackups: () => call<BackupInfo[]>("list_backups"),

  listActivities: () => call<Activity[]>("list_activities"),
  saveActivity: (activity: Partial<Activity> & { name: string; category: string; durationSeconds: number }) =>
    call<Activity>("save_activity", { activity }),
  deleteActivity: (id: string) => call<void>("delete_activity", { id }),

  engineStatus: () => call<EngineStatus>("engine_status"),
  start: () => call<EngineStatus>("engine_start"),
  pause: () => call<EngineStatus>("engine_pause"),
  resume: () => call<EngineStatus>("engine_resume"),
  stop: () => call<EngineStatus>("engine_stop"),
  snooze: () => call<EngineStatus>("engine_snooze"),
  skip: () => call<EngineStatus>("engine_skip"),
  acknowledge: () => call<EngineStatus>("engine_acknowledge"),
  startBreak: (activityId?: string | null) =>
    call<EngineStatus>("engine_start_break", { activityId: activityId ?? null }),
  finishBreak: (completed: boolean) => call<EngineStatus>("engine_finish_break", { completed }),
  setDnd: (minutes: number) => call<EngineStatus>("engine_set_dnd", { minutes }),

  todayStats: () => call<TodayStats>("today_stats"),
  trend: (days: number) => call<DailyPoint[]>("trend", { days }),
  monthSummary: () => call<MonthSummary>("month_summary"),
  history: (params: {
    limit?: number;
    offset?: number;
    category?: string | null;
    keyword?: string | null;
    completion?: string | null;
  }) => call<ActivitySession[]>("history_sessions", params),
  sittingHistory: (limit = 50) => call<SittingSession[]>("sitting_history", { limit }),
  tableCount: (table: string) => call<number>("table_count", { table }),

  setAutostart: (enabled: boolean) => call<boolean>("set_autostart", { enabled }),
};

type Handler<T> = (payload: T) => void;

export const events = {
  onStatus: (handler: Handler<EngineStatus>): Promise<UnlistenFn> =>
    listen<EngineStatus>("engine:status", (e) => handler(e.payload)),
  onReminder: (handler: Handler<EngineStatus>): Promise<UnlistenFn> =>
    listen<EngineStatus>("engine:reminder", (e) => handler(e.payload)),
  onRecorded: (handler: Handler<{ activityName: string; completion: string }>): Promise<UnlistenFn> =>
    listen("engine:recorded", (e) =>
      handler(e.payload as { activityName: string; completion: string }),
    ),
};

export const isDesktop = isTauri;
