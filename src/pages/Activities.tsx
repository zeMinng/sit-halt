import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Col,
  Divider,
  Empty,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
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
import { formatDuration, minutesToSeconds, secondsToMinutes } from "@/utils/format";

const CATEGORIES = ["全部", "身体活动", "肩颈放松", "腰背放松", "眼部放松", "健康习惯", "自定义"];

const CREATE_CATEGORIES = CATEGORIES.slice(1);

interface ActivityFormValues {
  id?: string;
  name: string;
  category: string;
  minutes: number;
  description?: string;
  steps?: string;
  cautions?: string;
  image?: string;
}

export function ActivitiesPage() {
  const activities = useAppStore((s) => s.activities);
  const refreshActivities = useAppStore((s) => s.refreshActivities);
  const runEngineAction = useAppStore((s) => s.runEngineAction);

  const [category, setCategory] = useState("全部");
  const [keyword, setKeyword] = useState("");
  const [detail, setDetail] = useState<Activity | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Activity | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [form] = Form.useForm<ActivityFormValues>();

  useEffect(() => {
    void refreshActivities();
  }, [refreshActivities]);

  const filtered = useMemo(() => {
    const kw = keyword.trim().toLowerCase();
    return activities.filter((a) => {
      const matchCategory = category === "全部" || a.category === category;
      const matchKeyword =
        !kw ||
        a.name.toLowerCase().includes(kw) ||
        (a.description ?? "").toLowerCase().includes(kw);
      return matchCategory && matchKeyword;
    });
  }, [activities, category, keyword]);

  const openCreate = () => {
    setEditing(null);
    setModalOpen(true);
  };

  const openEdit = (activity: Activity) => {
    setEditing(activity);
    setModalOpen(true);
  };

  // 弹窗 destroyOnClose，表单在关闭时被销毁；等弹窗打开、Form 挂载后再回填。
  const applyFormValues = () => {
    if (editing) {
      form.setFieldsValue({
        id: editing.id,
        name: editing.name,
        category: editing.category,
        minutes: secondsToMinutes(editing.durationSeconds),
        description: editing.description ?? "",
        steps: editing.steps.join("\n"),
        cautions: editing.cautions ?? "",
        image: editing.image ?? "",
      });
      return;
    }
    form.setFieldsValue({
      name: "",
      category: "身体活动",
      minutes: 3,
      description: "",
      steps: "从座位站起\n走动 3 分钟",
      cautions: "",
      image: "",
    });
  };

  const submitForm = async () => {
    let values: ActivityFormValues;
    try {
      values = await form.validateFields();
    } catch {
      return;
    }
    setSaving(true);
    setSaveError(null);
    try {
      await api.saveActivity({
        id: editing?.id,
        name: values.name.trim(),
        category: values.category,
        description: values.description?.trim() || null,
        durationSeconds: minutesToSeconds(values.minutes),
        steps: (values.steps ?? "")
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean),
        cautions: values.cautions?.trim() || null,
        image: values.image?.trim() || null,
        enabled: editing ? editing.enabled : true,
      });
      await refreshActivities();
      setModalOpen(false);
      setEditing(null);
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : JSON.stringify(error));
    } finally {
      setSaving(false);
    }
  };

  const toggleEnabled = async (activity: Activity) => {
    await api.saveActivity({ ...activity, enabled: !activity.enabled });
    await refreshActivities();
  };

  const remove = async (activity: Activity) => {
    await api.deleteActivity(activity.id);
    await refreshActivities();
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
          <Space wrap>
            <Input.Search
              placeholder="搜索活动名称"
              allowClear
              style={{ width: 180 }}
              onChange={(event) => setKeyword(event.target.value)}
            />
            <Button type="primary" onClick={openCreate}>
              自定义活动
            </Button>
          </Space>
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
                extra={
                  <Typography.Text type="secondary">
                    {formatDuration(activity.durationSeconds)}
                  </Typography.Text>
                }
              >
                <Typography.Paragraph type="secondary" ellipsis={{ rows: 2 }}>
                  {activity.description ?? "暂无活动说明"}
                </Typography.Paragraph>
                <Space wrap>
                  <Button
                    type="primary"
                    size="small"
                    disabled={!activity.enabled}
                    onClick={() => runEngineAction(() => api.startBreak(activity.id))}
                  >
                    立即开始
                  </Button>
                  <Button size="small" onClick={() => setDetail(activity)}>
                    查看指导
                  </Button>
                  <Button size="small" onClick={() => openEdit(activity)}>
                    编辑
                  </Button>
                  <Button size="small" onClick={() => void toggleEnabled(activity)}>
                    {activity.enabled ? "禁用" : "启用"}
                  </Button>
                  {!activity.isBuiltin ? (
                    <Popconfirm
                      title="删除该活动？"
                      description="删除后不可恢复"
                      okText="删除"
                      okButtonProps={{ danger: true }}
                      cancelText="取消"
                      onConfirm={() => void remove(activity)}
                    >
                      <Button size="small" danger>
                        删除
                      </Button>
                    </Popconfirm>
                  ) : null}
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
            disabled={!detail?.enabled}
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
            {detail.image ? (
              <img
                src={detail.image}
                alt={detail.name}
                style={{
                  maxWidth: "100%",
                  maxHeight: 220,
                  borderRadius: 8,
                  objectFit: "cover",
                }}
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = "none";
                }}
              />
            ) : null}
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
        open={modalOpen}
        title={editing ? "编辑活动" : "自定义活动"}
        centered
        onCancel={() => {
          setModalOpen(false);
          setEditing(null);
        }}
        afterOpenChange={(open) => {
          if (open) applyFormValues();
        }}
        onOk={() => void submitForm()}
        okText="保存"
        confirmLoading={saving}
        destroyOnClose
      >
        {saveError ? (
          <Alert
            type="error"
            showIcon
            style={{ marginBottom: 12 }}
            message={`保存失败：${saveError}`}
          />
        ) : null}
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
          <Form.Item
            name="image"
            label="图片链接"
            tooltip="可选：填网络图片地址；留空则不显示配图"
          >
            <Input placeholder="https://… 图片链接" allowClear />
          </Form.Item>
        </Form>
      </Modal>
    </Space>
  );
}
