/** 时长与时间格式化工具。所有时长在后端以整数秒传输。 */

export function formatDuration(totalSeconds: number): string {
  const seconds = Math.max(0, Math.round(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) {
    return minutes > 0 ? `${hours} 小时 ${minutes} 分` : `${hours} 小时`;
  }
  if (minutes > 0) {
    return `${minutes} 分`;
  }
  return `${seconds} 秒`;
}

export function formatClock(totalSeconds: number): string {
  const seconds = Math.max(0, Math.round(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  const mm = String(minutes).padStart(2, "0");
  const ss = String(rest).padStart(2, "0");
  if (hours > 0) {
    return `${hours}:${mm}:${ss}`;
  }
  return `${mm}:${ss}`;
}

export function formatTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
}

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function minutesToSeconds(minutes: number): number {
  return Math.round(minutes * 60);
}

export function secondsToMinutes(seconds: number): number {
  return Math.round(seconds / 60);
}

export const PHASE_LABEL: Record<string, string> = {
  idle: "未启动",
  working: "工作计时中",
  reminding: "等待提醒响应",
  break: "活动中",
  paused: "已暂停",
  suspended: "已挂起",
};

export const PHASE_COLOR: Record<string, string> = {
  idle: "default",
  working: "processing",
  reminding: "warning",
  break: "success",
  paused: "default",
  suspended: "default",
};

export const COMPLETION_LABEL: Record<string, string> = {
  completed: "已完成",
  partial: "提前结束",
  interrupted: "被打断",
};

export const COMPLETION_MODE_LABEL: Record<string, string> = {
  auto: "倒计时结束自动完成",
  manual: "手动点击完成",
  ask: "倒计时结束后询问",
};
