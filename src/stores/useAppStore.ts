import { create } from "zustand";

import { api, events, isDesktop } from "@/api/ipc";
import type {
  Activity,
  AppInfo,
  AppSettings,
  DbHealth,
  EngineStatus,
  ReminderPlan,
  TodayStats,
} from "@/types/contracts";
import { playReminderSound } from "@/utils/sound";

interface AppStore {
  ready: boolean;
  error: string | null;
  status: EngineStatus | null;
  today: TodayStats | null;
  plans: ReminderPlan[];
  activities: Activity[];
  settings: AppSettings;
  appInfo: AppInfo | null;
  dbHealth: DbHealth | null;

  bootstrap: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  refreshToday: () => Promise<void>;
  refreshPlans: () => Promise<void>;
  refreshActivities: () => Promise<void>;
  updateSetting: (key: string, value: unknown) => Promise<void>;
  runEngineAction: (action: () => Promise<EngineStatus>) => Promise<void>;
  clearError: () => void;
}

let subscribed = false;

function describe(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return JSON.stringify(error);
}

export const useAppStore = create<AppStore>((set, get) => ({
  ready: false,
  error: null,
  status: null,
  today: null,
  plans: [],
  activities: [],
  settings: {},
  appInfo: null,
  dbHealth: null,

  bootstrap: async () => {
    if (!isDesktop) {
      set({ ready: true, error: "当前是浏览器预览模式，完整功能需要在 Tauri 桌面应用中运行。" });
      return;
    }
    try {
      const [appInfo, dbHealth, settings, plans, activities, status, today] = await Promise.all([
        api.appInfo(),
        api.dbHealth(),
        api.getSettings(),
        api.listPlans(),
        api.listActivities(),
        api.engineStatus(),
        api.todayStats(),
      ]);
      set({ appInfo, dbHealth, settings, plans, activities, status, today, ready: true, error: null });

      if (!subscribed) {
        subscribed = true;
        await events.onStatus((next) => {
          set({ status: next });
          if (next.phase === "break" || next.phase === "idle") {
            void get().refreshToday();
          }
        });
        await events.onReminder((payload) => {
          set({ status: payload });
          const soundEnabled = get().settings.soundEnabled !== false;
          const volume = Number(get().settings.soundVolume ?? 0.7);
          if (soundEnabled) {
            playReminderSound(volume);
          }
        });
        await events.onRecorded(() => {
          void get().refreshToday();
        });
      }
    } catch (error) {
      set({ ready: true, error: describe(error) });
    }
  },

  refreshStatus: async () => {
    try {
      set({ status: await api.engineStatus() });
    } catch (error) {
      set({ error: describe(error) });
    }
  },

  refreshToday: async () => {
    try {
      set({ today: await api.todayStats() });
    } catch {
      // 统计刷新失败不应打断主流程
    }
  },

  refreshPlans: async () => {
    try {
      set({ plans: await api.listPlans() });
    } catch (error) {
      set({ error: describe(error) });
    }
  },

  refreshActivities: async () => {
    try {
      set({ activities: await api.listActivities() });
    } catch (error) {
      set({ error: describe(error) });
    }
  },

  updateSetting: async (key, value) => {
    try {
      const settings = await api.setSetting(key, value);
      set({ settings });
    } catch (error) {
      set({ error: describe(error) });
    }
  },

  runEngineAction: async (action) => {
    try {
      const status = await action();
      set({ status });
      void get().refreshToday();
    } catch (error) {
      set({ error: describe(error) });
    }
  },

  clearError: () => set({ error: null }),
}));
