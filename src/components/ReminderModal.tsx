import { useEffect, useState } from "react";
import { Button, Modal, Select, Space, Typography } from "antd";

import { api } from "@/api/ipc";
import { useAppStore } from "@/stores/useAppStore";
import { formatDuration } from "@/utils/format";

/**
 * 提醒窗口：必须保留明确的「稍后 / 跳过 / 已经活动」出口，
 * 不允许强制阻断用户（PRD 4.2.3 / 13.4）。
 */
export function ReminderModal() {
  const status = useAppStore((s) => s.status);
  const activities = useAppStore((s) => s.activities);
  const runEngineAction = useAppStore((s) => s.runEngineAction);
  const [activityId, setActivityId] = useState<string | undefined>(undefined);

  useEffect(() => {
    if (!activityId && activities.length > 0) {
      setActivityId(activities.find((a) => a.enabled)?.id ?? activities[0]?.id);
    }
  }, [activities, activityId]);

  const open =
    status?.phase === "reminding" && (status.snoozeRemainingSeconds ?? 0) === 0;

  return (
    <Modal
      open={Boolean(open)}
      title="该起来活动一下了"
      maskClosable={false}
      onCancel={() => runEngineAction(() => api.snooze())}
      footer={[
        <Button key="skip" onClick={() => runEngineAction(() => api.skip())}>
          跳过本次
        </Button>,
        <Button key="ack" onClick={() => runEngineAction(() => api.acknowledge())}>
          已经活动
        </Button>,
        <Button key="snooze" onClick={() => runEngineAction(() => api.snooze())}>
          稍后提醒
        </Button>,
        <Button
          key="start"
          type="primary"
          onClick={() => runEngineAction(() => api.startBreak(activityId ?? null))}
        >
          开始活动
        </Button>,
      ]}
      centered
    >
      <Space direction="vertical" size={12} style={{ width: "100%" }}>
        <Typography.Text>
          你已经在座位连续工作了{" "}
          <Typography.Text strong>
            {formatDuration(status?.workSeconds ?? 0)}
          </Typography.Text>
          ，建议起身活动一下。
        </Typography.Text>

        <Space direction="vertical" size={6} style={{ width: "100%" }}>
          <Typography.Text type="secondary">选择活动内容</Typography.Text>
          <Select
            style={{ width: "100%" }}
            value={activityId}
            onChange={setActivityId}
            options={activities
              .filter((a) => a.enabled)
              .map((a) => ({
                label: `${a.name} · ${a.category} · ${formatDuration(a.durationSeconds)}`,
                value: a.id,
              }))}
          />
        </Space>

        <Typography.Text type="secondary">
          「稍后提醒」会按计划延后；「跳过本次」不会计入活动完成次数。
        </Typography.Text>
      </Space>
    </Modal>
  );
}
