import { useEffect } from "react";
import { Card, Col, Empty, Row, Space, Table, Tag, Typography } from "antd";
import type { ColumnsType } from "antd/es/table";

import { StatusPanel } from "@/components/StatusPanel";
import { StatCard } from "@/components/StatCard";
import { TrendChart } from "@/components/TrendChart";
import { useAppStore } from "@/stores/useAppStore";
import type { ActivitySession } from "@/types/contracts";
import {
  COMPLETION_LABEL,
  formatClock,
  formatDateTime,
  formatDuration,
} from "@/utils/format";

const sessionColumns: ColumnsType<ActivitySession> = [
  { title: "活动名称", dataIndex: "activityName", key: "activityName" },
  {
    title: "分类",
    dataIndex: "category",
    key: "category",
    render: (value: string | null) => value ?? "—",
  },
  {
    title: "开始时间",
    dataIndex: "startedAt",
    key: "startedAt",
    render: (value: string) => formatDateTime(value),
  },
  {
    title: "活动时长",
    dataIndex: "durationSeconds",
    key: "durationSeconds",
    render: (value: number) => formatDuration(value),
  },
  {
    title: "完成状态",
    dataIndex: "completion",
    key: "completion",
    render: (value: string) => (
      <Tag color={value === "completed" ? "green" : value === "partial" ? "orange" : "default"}>
        {COMPLETION_LABEL[value] ?? value}
      </Tag>
    ),
  },
];

export function DashboardPage() {
  const today = useAppStore((s) => s.today);
  const status = useAppStore((s) => s.status);
  const refreshToday = useAppStore((s) => s.refreshToday);

  useEffect(() => {
    void refreshToday();
  }, [refreshToday]);

  const completionRate =
    today?.completionRate === null || today?.completionRate === undefined
      ? "暂无数据"
      : `${today.completionRate}%`;

  const nextReminder =
    status?.phase === "working"
      ? formatClock(status.workRemainingSeconds)
      : status?.phase === "reminding"
        ? (status.snoozeRemainingSeconds ?? 0) > 0
          ? formatClock(status.snoozeRemainingSeconds ?? 0)
          : "待响应"
        : "—";

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Row gutter={[12, 12]}>
        <Col xs={12} md={8} xl={4}>
          <StatCard
            title="今日久坐时长"
            value={formatDuration(today?.sittingSeconds ?? 0)}
            hint="按有效计时规则累计"
            tone="primary"
          />
        </Col>
        <Col xs={12} md={8} xl={4}>
          <StatCard title="今日活动次数" value={`${today?.activityCount ?? 0} 次`} hint="已完成的活动" />
        </Col>
        <Col xs={12} md={8} xl={4}>
          <StatCard
            title="今日活动总时长"
            value={formatDuration(today?.activitySeconds ?? 0)}
            hint="不含键鼠空闲时间"
          />
        </Col>
        <Col xs={12} md={8} xl={4}>
          <StatCard
            title="当前连续久坐"
            value={formatDuration(status?.workSeconds ?? 0)}
            hint={status?.phase === "break" ? "活动期间不计入" : "当前这一轮工作"}
          />
        </Col>
        <Col xs={12} md={8} xl={4}>
          <StatCard title="下一次提醒" value={nextReminder} hint={status?.planName ?? "未选择计划"} />
        </Col>
        <Col xs={12} md={8} xl={4}>
          <StatCard
            title="今日活动完成率"
            value={completionRate}
            hint={
              today && today.expectedActivities > 0
                ? `应完成 ${today.expectedActivities} 次`
                : "今天还没有触发提醒"
            }
          />
        </Col>
      </Row>

      <Card title="当前状态" size="small">
        <StatusPanel />
      </Card>

      <Card
        title="最近 7 天趋势"
        size="small"
        extra={<Typography.Text type="secondary">悬停可查看每日数据</Typography.Text>}
      >
        {today && today.trend.some((d) => d.sittingSeconds > 0 || d.activityCount > 0) ? (
          <TrendChart data={today.trend} />
        ) : (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description="还没有统计数据，开始一次计时后这里会出现趋势"
          />
        )}
      </Card>

      <Card title="最近活动记录" size="small">
        {today?.recentActivities?.length ? (
          <Table
            rowKey="id"
            size="small"
            pagination={false}
            scroll={{ x: "max-content" }}
            columns={sessionColumns}
            dataSource={today.recentActivities}
          />
        ) : (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="还没有活动记录" />
        )}
      </Card>
    </Space>
  );
}
