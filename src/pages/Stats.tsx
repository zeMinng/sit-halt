import { useEffect, useState } from "react";
import { Card, Col, Empty, Input, Row, Segmented, Select, Space, Table, Tag, Typography } from "antd";
import type { ColumnsType } from "antd/es/table";

import { api } from "@/api/ipc";
import { StatCard } from "@/components/StatCard";
import { TrendChart } from "@/components/TrendChart";
import type { ActivitySession, DailyPoint, MonthSummary, SittingSession } from "@/types/contracts";
import {
  COMPLETION_LABEL,
  formatDateTime,
  formatDuration,
} from "@/utils/format";

const CATEGORY_OPTIONS = [
  { value: "", label: "全部分类" },
  { value: "身体活动", label: "身体活动" },
  { value: "肩颈放松", label: "肩颈放松" },
  { value: "腰背放松", label: "腰背放松" },
  { value: "眼部放松", label: "眼部放松" },
  { value: "健康习惯", label: "健康习惯" },
  { value: "自述完成", label: "自述完成" },
];

const COMPLETION_OPTIONS = [
  { value: "", label: "全部状态" },
  { value: "completed", label: "已完成" },
  { value: "partial", label: "提前结束" },
  { value: "interrupted", label: "被打断" },
];

export function StatsPage() {
  const [days, setDays] = useState(7);
  const [trendData, setTrendData] = useState<DailyPoint[]>([]);
  const [month, setMonth] = useState<MonthSummary | null>(null);
  const [sessions, setSessions] = useState<ActivitySession[]>([]);
  const [sitting, setSitting] = useState<SittingSession[]>([]);
  const [category, setCategory] = useState("");
  const [completion, setCompletion] = useState("");
  const [keyword, setKeyword] = useState("");
  const [page, setPage] = useState(1);

  useEffect(() => {
    void api.trend(days).then(setTrendData);
  }, [days]);

  useEffect(() => {
    void api.monthSummary().then(setMonth);
    void api.sittingHistory(20).then(setSitting);
  }, []);

  useEffect(() => {
    void api
      .history({
        limit: 10,
        offset: (page - 1) * 10,
        category: category || null,
        keyword: keyword || null,
        completion: completion || null,
      })
      .then(setSessions);
  }, [page, category, completion, keyword]);

  const historyColumns: ColumnsType<ActivitySession> = [
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
      title: "实际时长",
      dataIndex: "durationSeconds",
      key: "durationSeconds",
      render: (value: number) => formatDuration(value),
    },
    {
      title: "计划时长",
      dataIndex: "plannedSeconds",
      key: "plannedSeconds",
      render: (value: number) => formatDuration(value),
    },
    {
      title: "来源",
      dataIndex: "source",
      key: "source",
      render: (value: string) =>
        ({ reminder: "提醒", manual: "立即活动", self_reported: "自述" }[value] ?? value),
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

  const sittingColumns: ColumnsType<SittingSession> = [
    {
      title: "开始时间",
      dataIndex: "startedAt",
      key: "startedAt",
      render: (value: string) => formatDateTime(value),
    },
    {
      title: "结束时间",
      dataIndex: "endedAt",
      key: "endedAt",
      render: (value: string | null) => formatDateTime(value),
    },
    {
      title: "有效久坐",
      dataIndex: "durationSeconds",
      key: "durationSeconds",
      render: (value: number) => formatDuration(value),
    },
    {
      title: "结束原因",
      dataIndex: "interruptedBy",
      key: "interruptedBy",
      render: (value: string | null) =>
        ({
          reminder: "触发提醒",
          pause: "用户暂停",
          stop: "停止计时",
          break: "开始活动",
          "plan-switch": "切换计划",
          plan_switch: "切换计划",
          app_exit: "退出应用",
          app_restart: "应用重启",
        }[value ?? ""] ?? (value ?? "—")),
    },
  ];

  const totalSitting = trendData.reduce((sum, d) => sum + d.sittingSeconds, 0);
  const totalActivity = trendData.reduce((sum, d) => sum + d.activitySeconds, 0);
  const totalCount = trendData.reduce((sum, d) => sum + d.activityCount, 0);

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Card
        title="趋势"
        size="small"
        extra={
          <Segmented
            options={[
              { label: "最近 7 天", value: 7 },
              { label: "最近 30 天", value: 30 },
            ]}
            value={days}
            onChange={(value) => setDays(Number(value))}
          />
        }
      >
        <Row gutter={[12, 12]} style={{ marginBottom: 12 }}>
          <Col xs={12} md={6}>
            <StatCard title="区间久坐总时长" value={formatDuration(totalSitting)} tone="primary" />
          </Col>
          <Col xs={12} md={6}>
            <StatCard title="区间活动总时长" value={formatDuration(totalActivity)} />
          </Col>
          <Col xs={12} md={6}>
            <StatCard title="区间活动次数" value={`${totalCount} 次`} />
          </Col>
          <Col xs={12} md={6}>
            <StatCard
              title={`${month?.month ?? "本月"}活动次数`}
              value={`${month?.activityCount ?? 0} 次`}
              hint={`月度久坐 ${formatDuration(month?.sittingSeconds ?? 0)}`}
            />
          </Col>
        </Row>
        {trendData.some(
          (d) => d.sittingSeconds > 0 || d.activitySeconds > 0 || d.activityCount > 0,
        ) ? (
          <TrendChart data={trendData} height={300} />
        ) : (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description="这段时间还没有数据，开始一次计时后这里会有趋势"
          />
        )}
      </Card>

      <Card
        title="活动记录"
        size="small"
        extra={
          <Space wrap>
            <Select
              style={{ width: 130 }}
              value={category}
              options={CATEGORY_OPTIONS}
              onChange={(value) => {
                setCategory(value);
                setPage(1);
              }}
            />
            <Select
              style={{ width: 120 }}
              value={completion}
              options={COMPLETION_OPTIONS}
              onChange={(value) => {
                setCompletion(value);
                setPage(1);
              }}
            />
            <Input.Search
              placeholder="搜索活动名称"
              allowClear
              style={{ width: 180 }}
              onSearch={(value) => {
                setKeyword(value.trim());
                setPage(1);
              }}
            />
          </Space>
        }
      >
        <Table
          rowKey="id"
          size="small"
          scroll={{ x: "max-content" }}
          columns={historyColumns}
          dataSource={sessions}
          pagination={{
            current: page,
            pageSize: 10,
            showSizeChanger: false,
            onChange: setPage,
            showTotal: (total) => `共 ${total} 条`,
          }}
          locale={{
            emptyText: (
              <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="还没有活动记录" />
            ),
          }}
        />
      </Card>

      <Card title="久坐记录" size="small" extra={<Typography.Text type="secondary">最近的 20 段</Typography.Text>}>
        <Table
          rowKey="id"
          size="small"
          scroll={{ x: "max-content" }}
          columns={sittingColumns}
          dataSource={sitting}
          pagination={false}
          locale={{
            emptyText: (
              <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="还没有久坐记录" />
            ),
          }}
        />
      </Card>
    </Space>
  );
}
