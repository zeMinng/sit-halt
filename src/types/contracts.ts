/** 与 Rust 侧 `models.rs` 一一对应的前端类型定义，字段命名遵循 camelCase。 */

export type Phase = "idle" | "working" | "reminding" | "break" | "paused" | "suspended";

export interface EngineStatus {
  phase: Phase;
  planId: string | null;
  planName: string | null;
  /** 当前计划启用的提醒方式：notification / popup / sound / fullscreen / widget */
  reminderMethods: string[];
  workSeconds: number;
  workTargetSeconds: number;
  workRemainingSeconds: number;
  breakSeconds: number;
  breakTargetSeconds: number;
  breakRemainingSeconds: number;
  activityId: string | null;
  activityName: string | null;
  completionMode: "auto" | "manual" | "ask" | null;
  reminderElapsedSeconds: number;
  snoozeRemainingSeconds: number | null;
  dndRemainingSeconds: number | null;
  cycleIndex: number;
  ignoredCount: number;
  canResume: boolean;
  /** 系统空闲秒数（键鼠无操作时长），取不到时为 null */
  idleSeconds: number | null;
  /** idle（离开座位）| outsideWorkHours（不在工作时间） */
  suspendReason: string | null;
  /** 当前是否落在计划设定的工作时间内 */
  inWorkWindow: boolean;
  /** 当前计划启用的工作时段条数（0 表示沿用计划的起止时间） */
  scheduleCount: number;
  serverTime: string;
}

/** 独立的每周工作时间表：一条记录代表「星期 X 的某个时段」 */
export interface WorkSchedule {
  id: string;
  planId: string;
  /** 1 = 周一 … 7 = 周日 */
  weekday: number;
  startMinute: number;
  endMinute: number;
  enabled: boolean;
  createdAt: string;
}

export type WorkScheduleInput = Omit<WorkSchedule, "id" | "createdAt"> & { id?: string | null };

export type ShortcutAction = "toggleMain" | "pauseResume" | "startBreak" | "dnd";

/** 动作 -> 全局快捷键 accelerator（如 "Ctrl+Alt+S"） */
export type ShortcutMap = Record<ShortcutAction, string>;

export interface BackupInfo {
  file: string;
  sizeBytes: number;
  createdAt: string;
}

export interface ReminderPlan {
  id: string;
  name: string;
  description: string | null;
  mode: string;
  workSeconds: number;
  breakSeconds: number;
  reminderMethods: string[];
  repeatIntervalSeconds: number;
  repeatTimes: number;
  snoozeSeconds: number;
  completionMode: "auto" | "manual" | "ask";
  idleDetection: boolean;
  idleThresholdSeconds: number;
  workStart: string | null;
  workEnd: string | null;
  workDays: number[];
  smartEnabled: boolean;
  isBuiltin: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export type ReminderPlanInput = Omit<
  ReminderPlan,
  "id" | "isBuiltin" | "createdAt" | "updatedAt"
> & { id?: string | null };

export interface Activity {
  id: string;
  name: string;
  category: string;
  description: string | null;
  durationSeconds: number;
  steps: string[];
  image: string | null;
  cautions: string | null;
  enabled: boolean;
  isBuiltin: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface ActivitySession {
  id: string;
  activityId: string | null;
  activityName: string;
  category: string | null;
  planId: string | null;
  startedAt: string;
  endedAt: string | null;
  durationSeconds: number;
  plannedSeconds: number;
  completion: string;
  source: string;
  createdAt: string;
}

export interface SittingSession {
  id: string;
  planId: string | null;
  startedAt: string;
  endedAt: string | null;
  durationSeconds: number;
  interruptedBy: string | null;
  createdAt: string;
}

export interface DailyPoint {
  date: string;
  sittingSeconds: number;
  activitySeconds: number;
  activityCount: number;
}

export interface TodayStats {
  date: string;
  sittingSeconds: number;
  activityCount: number;
  activitySeconds: number;
  longestSittingSeconds: number;
  expectedActivities: number;
  completionRate: number | null;
  trend: DailyPoint[];
  recentActivities: ActivitySession[];
}

export interface AppInfo {
  name: string;
  version: string;
  author: string;
  identifier: string;
  dbPath: string;
  dataDir: string;
  platform: string;
}

export interface DbHealth {
  ok: boolean;
  dbPath: string;
  migrationCount: number;
  tableCount: number;
  serverTime: string;
}

export interface MonthWeeklyPoint {
  week: string;
  activityCount: number;
  activitySeconds: number;
}

export interface MonthSummary {
  month: string;
  sittingSeconds: number;
  activitySeconds: number;
  activityCount: number;
  completionRate: number | null;
  weekly: MonthWeeklyPoint[];
  daily: DailyPoint[];
}

export type AppSettings = {
  theme?: "light" | "dark" | "system";
  language?: string;
  closeToTray?: boolean;
  autostart?: boolean;
  showOnStartup?: boolean;
  notificationEnabled?: boolean;
  soundEnabled?: boolean;
  soundVolume?: number;
  soundWave?: string;
  floatWidget?: boolean;
  backupIntervalDays?: number;
  backupKeep?: number;
  activePlanId?: string | null;
  [key: string]: unknown;
};
