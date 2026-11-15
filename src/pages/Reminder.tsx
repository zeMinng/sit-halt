import { useEffect, useState } from "react";
import {
  Button,
  Card,
  Checkbox,
  Col,
  Descriptions,
  Divider,
  Empty,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Row,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Typography,
} from "antd";
import { PlusOutlined } from "@ant-design/icons";
import type { ColumnsType } from "antd/es/table";

import { api } from "@/api/ipc";
import { StatusPanel } from "@/components/StatusPanel";
import { useAppStore } from "@/stores/useAppStore";
import type { ReminderPlan, ReminderPlanInput, WorkSchedule } from "@/types/contracts";
import {
  COMPLETION_MODE_LABEL,
  formatDuration,
  minutesToSeconds,
  secondsToMinutes,
} from "@/utils/format";

const METHOD_OPTIONS = [
  { label: "系统通知", value: "notification" },
  { label: "独立弹窗", value: "popup" },
  { label: "声音提醒", value: "sound" },
  { label: "全屏提醒", value: "fullscreen" },
  { label: "桌面悬浮组件", value: "widget" },
];

const WEEKDAY_OPTIONS = [
  { label: "周一", value: 1 },
  { label: "周二", value: 2 },
  { label: "周三", value: 3 },
  { label: "周四", value: 4 },
  { label: "周五", value: 5 },
  { label: "周六", value: 6 },
  { label: "周日", value: 7 },
];

const MODE_OPTIONS = [
  { label: "固定间隔（可跳过）", value: "interval" },
  { label: "番茄模式（必须活动）", value: "pomodoro" },
  { label: "智能模式（按活跃时长计时）", value: "smart" },
  { label: "自定义", value: "custom" },
];

const MODE_LABEL: Record<string, string> = {
  interval: "固定间隔",
  pomodoro: "番茄",
  smart: "智能",
  custom: "自定义",
};

/** "09:30" → 570；格式不合法返回 null */
function parseClock(text: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(text.trim());
  if (!match) return null;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (hour < 0 || hour > 23 || minute < 0 || minute > 59) return null;
  return hour * 60 + minute;
}

/** 570 → "09:30" */
function clockText(minutes: number): string {
  const hour = Math.floor(minutes / 60);
  const minute = minutes % 60;
  return `${String(hour).padStart(2, "0")}:${String(minute).padStart(2, "0")}`;
}

function weekdayLabel(weekday: number): string {
  return WEEKDAY_OPTIONS.find((o) => o.value === weekday)?.label ?? String(weekday);
}

interface ScheduleFormValues {
  weekday: number;
  start: string;
  end: string;
  enabled: boolean;
}

interface PlanFormValues {
  name: string;
  description?: string;
  mode: string;
  workMinutes: number;
  breakMinutes: number;
  completionMode: ReminderPlan["completionMode"];
  snoozeMinutes: number;
  repeatIntervalMinutes: number;
  repeatTimes: number;
  reminderMethods: string[];
  idleDetection: boolean;
  idleThresholdMinutes: number;
  smartEnabled: boolean;
  workStart?: string;
  workEnd?: string;
  workDays: number[];
}

function planToForm(plan: ReminderPlan): PlanFormValues {
  return {
    name: plan.name,
    description: plan.description ?? undefined,
    mode: plan.mode,
    workMinutes: secondsToMinutes(plan.workSeconds),
    breakMinutes: secondsToMinutes(plan.breakSeconds),
    completionMode: plan.completionMode,
    snoozeMinutes: secondsToMinutes(plan.snoozeSeconds),
    repeatIntervalMinutes: secondsToMinutes(plan.repeatIntervalSeconds),
    repeatTimes: plan.repeatTimes,
    reminderMethods: plan.reminderMethods,
    idleDetection: plan.idleDetection,
    idleThresholdMinutes: secondsToMinutes(plan.idleThresholdSeconds),
    smartEnabled: plan.smartEnabled,
    workStart: plan.workStart ?? undefined,
    workEnd: plan.workEnd ?? undefined,
    workDays: plan.workDays,
  };
}

function formToInput(values: PlanFormValues, editing: ReminderPlan | null): ReminderPlanInput {
  return {
    id: editing?.id ?? null,
    name: values.name.trim(),
    description: values.description?.trim() || null,
    mode: values.mode,
    workSeconds: minutesToSeconds(values.workMinutes),
    breakSeconds: minutesToSeconds(values.breakMinutes),
    reminderMethods: values.reminderMethods,
    repeatIntervalSeconds: minutesToSeconds(values.repeatIntervalMinutes),
    repeatTimes: values.repeatTimes,
    snoozeSeconds: minutesToSeconds(values.snoozeMinutes),
    completionMode: values.completionMode,
    idleDetection: values.idleDetection,
    idleThresholdSeconds: minutesToSeconds(values.idleThresholdMinutes),
    workStart: values.workStart ?? null,
    workEnd: values.workEnd ?? null,
    workDays: values.workDays,
    smartEnabled: values.smartEnabled,
    sortOrder: editing?.sortOrder ?? 99,
  };
}

export function ReminderPage() {
  const plans = useAppStore((s) => s.plans);
  const refreshPlans = useAppStore((s) => s.refreshPlans);
  const refreshStatus = useAppStore((s) => s.refreshStatus);
  const runEngineAction = useAppStore((s) => s.runEngineAction);
  const settings = useAppStore((s) => s.settings);
  const status = useAppStore((s) => s.status);

  const [form] = Form.useForm<PlanFormValues>();
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<ReminderPlan | null>(null);
  const [saving, setSaving] = useState(false);

  // 工作时间表（独立于计划字段的每周时段）
  const [scheduleForm] = Form.useForm<ScheduleFormValues>();
  const [schedules, setSchedules] = useState<WorkSchedule[]>([]);
  const [scheduleOpen, setScheduleOpen] = useState(false);
  const [scheduleSaving, setScheduleSaving] = useState(false);

  useEffect(() => {
    void refreshPlans();
    void refreshStatus();
  }, [refreshPlans, refreshStatus]);

  const activePlanId =
    (settings.activePlanId as string | null | undefined) ?? status?.planId ?? null;

  const loadSchedules = async (planId: string | null) => {
    if (!planId) {
      setSchedules([]);
      return;
    }
    try {
      setSchedules(await api.listWorkSchedules(planId));
    } catch {
      setSchedules([]);
    }
  };

  useEffect(() => {
    void loadSchedules(activePlanId);
  }, [activePlanId]);

  const openSchedule = () => {
    scheduleForm.setFieldsValue({ weekday: 1, start: "09:00", end: "18:00", enabled: true });
    setScheduleOpen(true);
  };

  const submitSchedule = async () => {
    if (!activePlanId) return;
    const values = await scheduleForm.validateFields();
    const start = parseClock(values.start);
    const end = parseClock(values.end);
    if (start === null || end === null) return;
    setScheduleSaving(true);
    try {
      await api.saveWorkSchedule({
        planId: activePlanId,
        weekday: values.weekday,
        startMinute: start,
        endMinute: end,
        enabled: values.enabled,
      });
      await loadSchedules(activePlanId);
      await refreshStatus();
      setScheduleOpen(false);
    } finally {
      setScheduleSaving(false);
    }
  };

  const toggleSchedule = async (row: WorkSchedule, enabled: boolean) => {
    await api.saveWorkSchedule({
      id: row.id,
      planId: row.planId,
      weekday: row.weekday,
      startMinute: row.startMinute,
      endMinute: row.endMinute,
      enabled,
    });
    await loadSchedules(activePlanId);
    await refreshStatus();
  };

  const removeSchedule = async (row: WorkSchedule) => {
    await api.deleteWorkSchedule(row.id);
    await loadSchedules(activePlanId);
    await refreshStatus();
  };

  const scheduleColumns: ColumnsType<WorkSchedule> = [
    {
      title: "星期",
      dataIndex: "weekday",
      key: "weekday",
      width: 90,
      render: (value: number) => weekdayLabel(value),
    },
    {
      title: "时段",
      key: "range",
      render: (_, row) =>
        `${clockText(row.startMinute)} ~ ${clockText(row.endMinute)}` +
        (row.endMinute < row.startMinute ? "（跨零点）" : ""),
    },
    {
      title: "启用",
      dataIndex: "enabled",
      key: "enabled",
      width: 80,
      render: (value: boolean, row) => (
        <Switch checked={value} onChange={(checked) => void toggleSchedule(row, checked)} />
      ),
    },
    {
      title: "操作",
      key: "actions",
      width: 80,
      render: (_, row) => (
        <Popconfirm
          title="删除这个时段？"
          okText="删除"
          cancelText="取消"
          onConfirm={() => removeSchedule(row)}
        >
          <Button size="small" type="link" danger>
            删除
          </Button>
        </Popconfirm>
      ),
    },
  ];

  const openCreate = () => {
    setEditing(null);
    form.setFieldsValue({
      name: "自定义模式",
      description: "",
      mode: "interval",
      workMinutes: 45,
      breakMinutes: 5,
      completionMode: "auto",
      snoozeMinutes: 5,
      repeatIntervalMinutes: 5,
      repeatTimes: 2,
      reminderMethods: ["notification", "popup", "sound"],
      idleDetection: true,
      idleThresholdMinutes: 3,
      smartEnabled: false,
      workStart: "09:00",
      workEnd: "18:00",
      workDays: [1, 2, 3, 4, 5],
    });
    setModalOpen(true);
  };

  const openEdit = (plan: ReminderPlan) => {
    setEditing(plan);
    form.setFieldsValue(planToForm(plan));
    setModalOpen(true);
  };

  const submit = async () => {
    const values = await form.validateFields();
    setSaving(true);
    try {
      await api.savePlan(formToInput(values, editing));
      await refreshPlans();
      setModalOpen(false);
    } finally {
      setSaving(false);
    }
  };

  const remove = async (plan: ReminderPlan) => {
    await api.deletePlan(plan.id);
    await refreshPlans();
  };

  const columns: ColumnsType<ReminderPlan> = [
    {
      title: "计划",
      dataIndex: "name",
      key: "name",
      render: (value: string, record) => (
        <Space size={8}>
          <Typography.Text strong>{value}</Typography.Text>
          {record.isBuiltin ? <Tag>内置</Tag> : null}
          {activePlanId === record.id ? <Tag color="blue">使用中</Tag> : null}
        </Space>
      ),
    },
    {
      title: "模式",
      dataIndex: "mode",
      key: "mode",
      render: (value: string) => <Tag>{MODE_LABEL[value] ?? value}</Tag>,
    },
    {
      title: "工作 / 活动",
      key: "duration",
      render: (_, record) =>
        `${formatDuration(record.workSeconds)} / ${formatDuration(record.breakSeconds)}`,
    },
    {
      title: "提醒方式",
      dataIndex: "reminderMethods",
      key: "reminderMethods",
      render: (methods: string[]) =>
        methods.length
          ? methods
              .map((m) => METHOD_OPTIONS.find((o) => o.value === m)?.label ?? m)
              .join("、")
          : "未设置",
    },
    {
      title: "完成方式",
      dataIndex: "completionMode",
      key: "completionMode",
      render: (value: string) => COMPLETION_MODE_LABEL[value] ?? value,
    },
    {
      title: "操作",
      key: "actions",
      width: 220,
      render: (_, record) => (
        <Space size={4}>
          <Button
            size="small"
            type="link"
            onClick={() => runEngineAction(() => api.activatePlan(record.id))}
          >
            使用
          </Button>
          <Button size="small" type="link" onClick={() => openEdit(record)}>
            编辑
          </Button>
          <Popconfirm
            title="删除这个计划？"
            description="只删除计划配置，不影响已有统计记录。"
            okText="删除"
            cancelText="取消"
            disabled={record.isBuiltin}
            onConfirm={() => remove(record)}
          >
            <Button size="small" type="link" danger disabled={record.isBuiltin}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Card title="当前提醒状态" size="small">
        <StatusPanel />
      </Card>

      <Card
        title="提醒计划"
        size="small"
        extra={
          <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
            新建计划
          </Button>
        }
      >
        {plans.length ? (
          <Table
            rowKey="id"
            size="small"
            pagination={false}
            scroll={{ x: "max-content" }}
            columns={columns}
            dataSource={plans}
          />
        ) : (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description="还没有提醒计划，点右上角「新建计划」开始"
          />
        )}
      </Card>

      <Card
        title="每周工作时间表"
        size="small"
        extra={
          <Button
            icon={<PlusOutlined />}
            onClick={openSchedule}
            disabled={!activePlanId}
          >
            新增时段
          </Button>
        }
      >
        {activePlanId ? (
          <>
            <Table
              rowKey="id"
              size="small"
              pagination={false}
              scroll={{ x: "max-content" }}
              columns={scheduleColumns}
              dataSource={schedules}
              locale={{
                emptyText: (
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description="尚未配置时段，将沿用计划自带的「工作时间」"
                  />
                ),
              }}
            />
            <Divider style={{ margin: "12px 0" }} />
            <Typography.Text type="secondary">
              配置了启用时段后，只有落在这些时段内才会计时（不同星期可设多段，支持跨零点）；
              所有时段都禁用时自动退回计划自带的「适用工作日 + 工作时间」。
            </Typography.Text>
          </>
        ) : (
          <Typography.Text type="secondary">
            先在上方「使用」一个提醒计划，再为它配置工作时间表。
          </Typography.Text>
        )}
      </Card>

      <Card title="工作时间与智能规则" size="small">
        {status?.planId ? (
          <Descriptions size="small" column={2} bordered>
            {(() => {
              const plan = plans.find((p) => p.id === status.planId);
              if (!plan) return <Descriptions.Item label="计划">未知</Descriptions.Item>;
              return (
                <>
                  <Descriptions.Item label="适用工作日">
                    {plan.workDays.length
                      ? plan.workDays
                          .map((d) => WEEKDAY_OPTIONS.find((o) => o.value === d)?.label ?? d)
                          .join("、")
                      : "每天"}
                  </Descriptions.Item>
                  <Descriptions.Item label="工作时间">
                    {plan.workStart && plan.workEnd
                      ? `${plan.workStart} ~ ${plan.workEnd}`
                      : "全天运行"}
                  </Descriptions.Item>
                  <Descriptions.Item label="键鼠空闲检测">
                    {plan.idleDetection
                      ? `开启（超过 ${formatDuration(plan.idleThresholdSeconds)} 无操作即暂停计时）`
                      : "关闭"}
                  </Descriptions.Item>
                  <Descriptions.Item label="智能提醒">
                    {plan.smartEnabled ? "开启" : "关闭"}
                  </Descriptions.Item>
                  <Descriptions.Item label="当前是否在工作时间">
                    {status.inWorkWindow ? "是" : "否（暂不计久坐）"}
                  </Descriptions.Item>
                  <Descriptions.Item label="系统空闲">
                    {status.idleSeconds === null
                      ? "不可用"
                      : formatDuration(status.idleSeconds)}
                  </Descriptions.Item>
                </>
              );
            })()}
          </Descriptions>
        ) : (
          <Typography.Text type="secondary">尚未选择提醒计划。</Typography.Text>
        )}
        <Divider style={{ margin: "12px 0" }} />
        <Typography.Text type="secondary">
          说明：工作在计划设定的「适用工作日 + 工作时间」内才计时；超出时间窗、或键鼠空闲超过阈值时，
          计时自动挂起，回到座位 / 回到工作时间后自动恢复。番茄模式（mode = pomodoro）不允许跳过提醒。
        </Typography.Text>
      </Card>

      <Modal
        open={modalOpen}
        title={editing ? `编辑计划 · ${editing.name}` : "新建提醒计划"}
        centered
        onCancel={() => setModalOpen(false)}
        onOk={submit}
        okText="保存"
        confirmLoading={saving}
        width={620}
        destroyOnClose
      >
        <Form form={form} layout="vertical" preserve={false}>
          <Row gutter={12}>
            <Col span={12}>
              <Form.Item
                name="name"
                label="计划名称"
                rules={[{ required: true, message: "请填写计划名称" }]}
              >
                <Input placeholder="例如：深度工作" />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="description" label="计划描述">
                <Input placeholder="一句话说明这个计划的用途" />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="workMinutes"
                label="工作时长（分钟）"
                rules={[{ required: true, message: "请填写工作时长" }]}
              >
                <InputNumber min={1} max={240} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="breakMinutes"
                label="活动时长（分钟）"
                rules={[{ required: true, message: "请填写活动时长" }]}
              >
                <InputNumber min={1} max={60} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="mode" label="计划模式">
                <Select options={MODE_OPTIONS} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="completionMode" label="活动完成方式">
                <Select
                  options={Object.entries(COMPLETION_MODE_LABEL).map(([value, label]) => ({
                    value,
                    label,
                  }))}
                />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="snoozeMinutes" label="稍后提醒时长（分钟）">
                <InputNumber min={1} max={60} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="repeatIntervalMinutes" label="重复提醒间隔（分钟）">
                <InputNumber min={1} max={60} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="repeatTimes" label="重复提醒次数">
                <InputNumber min={0} max={10} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="workStart" label="工作时间开始">
                <Input placeholder="09:00（留空表示全天运行）" />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="workEnd" label="工作时间结束">
                <Input placeholder="18:00（留空表示全天运行）" />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="idleThresholdMinutes" label="键鼠空闲阈值（分钟）">
                <InputNumber min={1} max={30} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="idleDetection" label="启用键鼠空闲检测" valuePropName="checked">
                <Switch />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="smartEnabled" label="启用智能提醒" valuePropName="checked">
                <Switch />
              </Form.Item>
            </Col>
          </Row>

          <Form.Item name="reminderMethods" label="提醒方式（可多选）">
            <Checkbox.Group options={METHOD_OPTIONS} />
          </Form.Item>

          <Form.Item name="workDays" label="适用工作日">
            <Checkbox.Group options={WEEKDAY_OPTIONS} />
          </Form.Item>
        </Form>
      </Modal>

      <Modal
        open={scheduleOpen}
        title="新增工作时段"
        centered
        onCancel={() => setScheduleOpen(false)}
        onOk={submitSchedule}
        okText="保存"
        confirmLoading={scheduleSaving}
        width={460}
        destroyOnClose
      >
        <Form form={scheduleForm} layout="vertical" preserve={false}>
          <Form.Item
            name="weekday"
            label="星期"
            rules={[{ required: true, message: "请选择星期" }]}
          >
            <Select options={WEEKDAY_OPTIONS} />
          </Form.Item>
          <Row gutter={12}>
            <Col span={12}>
              <Form.Item
                name="start"
                label="开始时间"
                rules={[
                  { required: true, message: "请填写开始时间" },
                  {
                    validator: (_, value: string) =>
                      parseClock(value ?? "") === null
                        ? Promise.reject(new Error("格式为 HH:MM，例如 09:00"))
                        : Promise.resolve(),
                  },
                ]}
              >
                <Input placeholder="09:00" />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="end"
                label="结束时间"
                rules={[
                  { required: true, message: "请填写结束时间" },
                  {
                    validator: (_, value: string) =>
                      parseClock(value ?? "") === null
                        ? Promise.reject(new Error("格式为 HH:MM，例如 18:00"))
                        : Promise.resolve(),
                  },
                ]}
              >
                <Input placeholder="18:00" />
              </Form.Item>
            </Col>
          </Row>
          <Form.Item name="enabled" label="立即启用" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </Space>
  );
}
