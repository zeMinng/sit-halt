import { Alert, Button, Modal, Progress, Space, Steps, Typography } from "antd";

import { api } from "@/api/ipc";
import { useAppStore } from "@/stores/useAppStore";
import { formatClock, formatDuration } from "@/utils/format";

/** 活动引导：名称、说明、倒计时、操作步骤、提前结束与完成（PRD 4.4.3）。 */
export function BreakDialog() {
  const status = useAppStore((s) => s.status);
  const activities = useAppStore((s) => s.activities);
  const runEngineAction = useAppStore((s) => s.runEngineAction);

  const open = status?.phase === "break";
  const activity = activities.find((a) => a.id === status?.activityId);
  const remaining = status?.breakRemainingSeconds ?? 0;
  const askNow = remaining === 0 && status?.completionMode !== "auto";

  const percent =
    status && status.breakTargetSeconds > 0
      ? Math.min(100, Math.round((status.breakSeconds / status.breakTargetSeconds) * 100))
      : 0;

  return (
    <Modal
      open={Boolean(open)}
      title={`活动进行中 · ${status?.activityName ?? "起身活动"}`}
      closable={false}
      maskClosable={false}
      footer={[
        <Button key="end" onClick={() => runEngineAction(() => api.finishBreak(false))}>
          提前结束
        </Button>,
        <Button
          key="done"
          type="primary"
          onClick={() => runEngineAction(() => api.finishBreak(true))}
        >
          完成活动
        </Button>,
      ]}
      centered
    >
      <Space direction="vertical" size={16} style={{ width: "100%" }}>
        <div style={{ textAlign: "center" }}>
          <div className="break-countdown">{formatClock(remaining)}</div>
          <Typography.Text type="secondary">
            目标时长 {formatDuration(status?.breakTargetSeconds ?? 0)}
          </Typography.Text>
          <Progress percent={percent} showInfo={false} status="success" />
        </div>

        {askNow ? (
          <Alert
            type="warning"
            showIcon
            message="倒计时已结束"
            description="这个计划设置为「倒计时结束后询问」，请确认本次活动是否完成。"
          />
        ) : null}

        {activity ? (
          <Space direction="vertical" size={8} style={{ width: "100%" }}>
            {activity.description ? (
              <Typography.Text type="secondary">{activity.description}</Typography.Text>
            ) : null}
            {activity.steps.length > 0 ? (
              <Steps
                direction="vertical"
                size="small"
                current={-1}
                items={activity.steps.map((step) => ({ title: step }))}
              />
            ) : null}
            {activity.cautions ? (
              <Alert type="info" showIcon message={activity.cautions} />
            ) : null}
          </Space>
        ) : null}

        <Typography.Text type="secondary">
          活动期间不累计久坐时间，完成后会自动开始下一个工作周期。
        </Typography.Text>
      </Space>
    </Modal>
  );
}
