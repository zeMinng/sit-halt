import { useEffect, useMemo, useState } from "react";
import {
  Button,
  Card,
  Col,
  Divider,
  Empty,
  Form,
  Input,
  InputNumber,
  Modal,
  Row,
  Segmented,
  Select,
  Space,
  Tag,
  Typography,
} from "antd";

import { api } from "@/api/ipc";
import { useAppStore } from "@/stores/useAppStore";
import type { Activity } from "@/types/contracts";
import { formatDuration, minutesToSeconds } from "@/utils/format";

const CATEGORIES = ["全部", "身体活动", "肩颈放松", "腰背放松", "眼部放松", "健康习惯", "自定义"];

const CREATE_CATEGORIES = CATEGORIES.slice(1);

interface ActivityFormValues {
  name: string;
  category: string;
  minutes: number;
  description?: string;
  steps?: string;
  cautions?: string;
}

export function ActivitiesPage() {
  const activities = useAppStore((s) => s.activities);
  const refreshActivities = useAppStore((s) => s.refreshActivities);
  const runEngineAction = useAppStore((s) => s.runEngineAction);

  const [category, setCategory] = useState("全部");
  const [detail, setDetail] = useState<Activity | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [form] = Form.useForm<ActivityFormValues>();

  useEffect(() => {
    void refreshActivities();
  }, [refreshActivities]);

  const filtered = useMemo(() => {
    if (category === "全部") return activities;
    return activities.filter((a) => a.category === category);
  }, [activities, category]);

  const submitCreate = async () => {
    const values = await form.validateFields();
    setSaving(true);
    try {
      await api.saveActivity({
        name: values.name.trim(),
        category: values.category,
        description: values.description?.trim() || null,
        durationSeconds: minutesToSeconds(values.minutes),
        steps: (values.steps ?? "")
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean),
        cautions: values.cautions?.trim() || null,
        enabled: true,
      });
      await refreshActivities();
      setCreateOpen(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Card size="small">
        <Space wrap style={{ width: "100%", justifyContent: "space-between" }}>
          <Segmented
            options={CATEGORIES}
            value={category}
            onChange={(value) => setCategory(String(value))}
          />
          <Button
            type="primary"
            onClick={() => {
              form.setFieldsValue({
                name: "",
                category: "身体活动",
                minutes: 3,
                description: "",
                steps: "从座位站起\n走动 3 分钟",
                cautions: "",
              });
              setCreateOpen(true);
            }}
          >
            自定义活动
          </Button>
        </Space>
      </Card>

      {filtered.length === 0 ? (
        <Empty description="该分类下还没有活动" />
      ) : (
        <Row gutter={[12, 12]}>
          {filtered.map((activity) => (
            <Col xs={24} md={12} xl={8} key={activity.id}>
              <Card
                size="small"
                title={
                  <Space size={8}>
                    <Typography.Text strong>{activity.name}</Typography.Text>
                    <Tag>{activity.category}</Tag>
                    {!activity.enabled ? <Tag color="default">已禁用</Tag> : null}
                  </Space>
                }
                extra={<Typography.Text type="secondary">{formatDuration(activity.durationSeconds)}</Typography.Text>}
              >
                <Typography.Paragraph type="secondary" ellipsis={{ rows: 2 }}>
                  {activity.description ?? "暂无活动说明"}
                </Typography.Paragraph>
                <Space>
                  <Button
                    type="primary"
                    size="small"
                    onClick={() => runEngineAction(() => api.startBreak(activity.id))}
                  >
                    立即开始
                  </Button>
                  <Button size="small" onClick={() => setDetail(activity)}>
                    查看指导
                  </Button>
                </Space>
              </Card>
            </Col>
          ))}
        </Row>
      )}

      <Modal
        open={Boolean(detail)}
        title={detail?.name}
        centered
        onCancel={() => setDetail(null)}
        footer={[
          <Button key="close" onClick={() => setDetail(null)}>
            关闭
          </Button>,
          <Button
            key="start"
            type="primary"
            onClick={() => {
              if (detail) {
                runEngineAction(() => api.startBreak(detail.id));
                setDetail(null);
              }
            }}
          >
            开始这个活动
          </Button>,
        ]}
      >
        {detail ? (
          <Space direction="vertical" size={12} style={{ width: "100%" }}>
            <Space>
              <Tag>{detail.category}</Tag>
              <Typography.Text type="secondary">
                建议时长 {formatDuration(detail.durationSeconds)}
              </Typography.Text>
            </Space>
            <Typography.Text>{detail.description ?? "暂无活动说明"}</Typography.Text>
            <Divider style={{ margin: "4px 0" }} />
            <Typography.Text strong>操作步骤</Typography.Text>
            {detail.steps.length ? (
              <ol style={{ margin: 0, paddingLeft: 20 }}>
                {detail.steps.map((step) => (
                  <li key={step}>{step}</li>
                ))}
              </ol>
            ) : (
              <Typography.Text type="secondary">暂无步骤说明</Typography.Text>
            )}
            {detail.cautions ? (
              <Typography.Text type="warning">注意：{detail.cautions}</Typography.Text>
            ) : null}
          </Space>
        ) : null}
      </Modal>

      <Modal
        open={createOpen}
        title="自定义活动"
        centered
        onCancel={() => setCreateOpen(false)}
        onOk={submitCreate}
        okText="保存"
        confirmLoading={saving}
        destroyOnClose
      >
        <Form form={form} layout="vertical" preserve={false}>
          <Row gutter={12}>
            <Col span={12}>
              <Form.Item name="name" label="活动名称" rules={[{ required: true, message: "请填写活动名称" }]}>
                <Input />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="category" label="分类" rules={[{ required: true }]}>
                <Select options={CREATE_CATEGORIES.map((c) => ({ value: c, label: c }))} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="minutes" label="建议时长（分钟）" rules={[{ required: true }]}>
                <InputNumber min={0.2} max={60} step={0.5} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
          </Row>
          <Form.Item name="description" label="活动说明">
            <Input.TextArea rows={2} />
          </Form.Item>
          <Form.Item name="steps" label="操作步骤（每行一步）">
            <Input.TextArea rows={4} />
          </Form.Item>
          <Form.Item name="cautions" label="注意事项">
            <Input.TextArea rows={2} />
          </Form.Item>
        </Form>
      </Modal>
    </Space>
  );
}
