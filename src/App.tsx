import { Suspense, lazy, useEffect, useMemo, useState } from "react";
import { App as AntdApp, Alert, ConfigProvider, Layout, Menu, Space, Spin, Tag, Typography } from "antd";
import {
  AppstoreOutlined,
  AreaChartOutlined,
  DashboardOutlined,
  SettingOutlined,
  AlertOutlined,
} from "@ant-design/icons";
import { HashRouter, Navigate, Route, Routes, useLocation, useNavigate } from "react-router-dom";

import "./App.css";
import { BreakDialog } from "@/components/BreakDialog";
import { ReminderModal } from "@/components/ReminderModal";
import { useAppStore } from "@/stores/useAppStore";
import { buildTheme } from "@/theme";
import { PHASE_COLOR, PHASE_LABEL, formatClock } from "@/utils/format";

// 页面按需加载：首屏只带首页，提醒中心 / 活动 / 统计（含 ECharts）/ 设置各自成块，
// 避免单个 chunk 达到 2MB 以上（PRD 11.4 启动时间要求）。
const DashboardPage = lazy(() =>
  import("@/pages/Dashboard").then((m) => ({ default: m.DashboardPage })),
);
const ReminderPage = lazy(() =>
  import("@/pages/Reminder").then((m) => ({ default: m.ReminderPage })),
);
const ActivitiesPage = lazy(() =>
  import("@/pages/Activities").then((m) => ({ default: m.ActivitiesPage })),
);
const StatsPage = lazy(() =>
  import("@/pages/Stats").then((m) => ({ default: m.StatsPage })),
);
const SettingsPage = lazy(() =>
  import("@/pages/Settings").then((m) => ({ default: m.SettingsPage })),
);

const NAV_ITEMS = [
  { key: "/dashboard", icon: <DashboardOutlined />, label: "首页" },
  { key: "/reminder", icon: <AlertOutlined />, label: "提醒中心" },
  { key: "/activities", icon: <AppstoreOutlined />, label: "活动中心" },
  { key: "/stats", icon: <AreaChartOutlined />, label: "数据统计" },
  { key: "/settings", icon: <SettingOutlined />, label: "设置" },
];

function HeaderSummary() {
  const status = useAppStore((s) => s.status);
  if (!status) return null;
  const phaseLabel = PHASE_LABEL[status.phase] ?? status.phase;
  const right =
    status.phase === "working"
      ? `下次提醒 ${formatClock(status.workRemainingSeconds)}`
      : status.phase === "reminding"
        ? (status.snoozeRemainingSeconds ?? 0) > 0
          ? `稍后提醒 ${formatClock(status.snoozeRemainingSeconds ?? 0)}`
          : "等待响应"
        : status.phase === "break"
          ? `活动中 ${formatClock(status.breakRemainingSeconds)}`
          : "未计时";

  return (
    <Space size={12}>
      <Tag color={PHASE_COLOR[status.phase]}>{phaseLabel}</Tag>
      <Typography.Text type="secondary">{status.planName ?? "未选择计划"}</Typography.Text>
      <Typography.Text type="secondary">{right}</Typography.Text>
    </Space>
  );
}

function Shell() {
  const navigate = useNavigate();
  const location = useLocation();
  const error = useAppStore((s) => s.error);
  const clearError = useAppStore((s) => s.clearError);

  return (
    <Layout className="app-shell">
      <Layout.Sider width={208} theme="light" className="app-sider">
        <div className="app-brand">
          <div className="app-brand-name">SitHalt</div>
          <div className="app-brand-sub">息坐 · 伏案有时，休憩亦有时</div>
        </div>
        <Menu
          mode="inline"
          selectedKeys={[location.pathname]}
          items={NAV_ITEMS}
          onClick={({ key }) => navigate(key)}
          className="app-menu"
        />
      </Layout.Sider>

      <Layout>
        <Layout.Header className="app-header">
          <HeaderSummary />
        </Layout.Header>
        <Layout.Content className="app-content">
          {error ? (
            <Alert
              type="warning"
              showIcon
              closable
              message={error}
              onClose={clearError}
              style={{ marginBottom: 16 }}
            />
          ) : null}
          <Suspense
            fallback={
              <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 12, padding: 48 }}>
                <Spin size="large" />
                <Typography.Text type="secondary">加载页面…</Typography.Text>
              </div>
            }
          >
            <Routes>
              <Route path="/" element={<Navigate to="/dashboard" replace />} />
              <Route path="/dashboard" element={<DashboardPage />} />
              <Route path="/reminder" element={<ReminderPage />} />
              <Route path="/activities" element={<ActivitiesPage />} />
              <Route path="/stats" element={<StatsPage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Routes>
          </Suspense>
        </Layout.Content>
      </Layout>

      <ReminderModal />
      <BreakDialog />
    </Layout>
  );
}

export default function App() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const themeSetting = useAppStore((s) => s.settings.theme);
  const [systemDark, setSystemDark] = useState(false);

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => setSystemDark(query.matches);
    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, []);

  const dark = themeSetting === "dark" || (themeSetting !== "light" && systemDark);

  const themeConfig = useMemo(() => buildTheme(dark), [dark]);

  return (
    <ConfigProvider theme={themeConfig}>
      <AntdApp>
        <div className={dark ? "theme-dark" : "theme-light"}>
          <HashRouter>
            <Shell />
          </HashRouter>
        </div>
      </AntdApp>
    </ConfigProvider>
  );
}
