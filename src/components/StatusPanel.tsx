import { Button, Empty, Progress, Space, Tag, Typography } from "antd";
import {
  PauseCircleOutlined,
  PlayCircleOutlined,
  StopOutlined,
} from "@ant-design/icons";

import { api } from "@/api/ipc";
import { useAppStore } from "@/stores/useAppStore";
import { PHASE_COLOR, PHASE_LABEL, formatClock, formatDuration } from "@/utils/format";

export function StatusPanel() {
  const status = useAppStore((s) => s.status);
  const activities = useAppStore((s) => s.activities);
  const runEngineAction = useAppStore((s) => s.runEngineAction);

  if (!status) {
    return <Empty description="尚未连接到后台计时引擎" />;
  }

  const workPercent =
    status.workTargetSeconds > 0
      ? Math.min(100, Math.round((status.workSeconds / status.workTargetSeconds) * 100))
      : 0;

  const snoozed = (status.snoozeRemainingSeconds ?? 0) > 0;

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Space wrap size={12} align="center">
        <Tag color={PHASE_COLOR[status.phase]} style={{ fontSize: 14, padding: "2px 10px" }}>
          {PHASE_LABEL[status.phase] ?? status.phase}
        </Tag>
        <Typography.Text strong>{status.planName ?? "未选择计划"}</Typography.Text>
        <Tag>第 {status.cycleIndex + 1} 个周期</Tag>
        {status.dndRemainingSeconds ? (
          <Tag color="purple">勿扰中 · 剩余 {formatClock(status.dndRemainingSeconds)}</Tag>
        ) : null}
        {status.ignoredCount > 0 ? <Tag color="orange">已跳过 {status.ignoredCount} 次</Tag> : null}
        {status.phase === "suspended" ? (
          <Tag color="blue">
            {status.suspendReason === "idle"
              ? `已离开座位，计时暂停（空闲 ${formatDuration(status.idleSeconds ?? 0)}）`
              : status.suspendReason === "outside_work_hours"
                ? "当前不在工作时间，计时暂停"
                : "已挂起"}
          </Tag>
        ) : null}
        {!status.inWorkWindow ? <Tag>非工作时间</Tag> : null}
      </Space>

      {(status.phase === "working" ||
        status.phase === "paused" ||
        status.phase === "suspended" ||
        status.phase === "reminding") && (
        <div>
          <Space style={{ width: "100%", justifyContent: "space-between" }}>
            <Typography.Text type="secondary">
              本轮已坐 {formatDuration(status.workSeconds)} / 目标 {formatDuration(status.workTargetSeconds)}
            </Typography.Text>
            <Typography.Text type="secondary">
              {status.phase === "reminding"
                ? "等待提醒响应"
                : snoozed
                  ? `稍后提醒 ${formatClock(status.snoozeRemainingSeconds ?? 0)}`
                  : `下次提醒 ${formatClock(status.workRemainingSeconds)}`}
            </Typography.Text>
          </Space>
          <Progress percent={workPercent} showInfo={false} status={status.phase === "reminding" ? "exception" : "active"} />
        </div>
      )}

      {status.phase === "break" && (
        <div>
          <Space style={{ width: "100%", justifyContent: "space-between" }}>
            <Typography.Text type="secondary">
              正在活动：{status.activityName ?? "起身活动"}
            </Typography.Text>
            <Typography.Text strong>{formatClock(status.breakRemainingSeconds)}</Typography.Text>
          </Space>
          <Progress
            percent={
              status.breakTargetSeconds > 0
                ? Math.min(
                    100,
                    Math.round((status.breakSeconds / status.breakTargetSeconds) * 100),
                  )
                : 0
            }
            showInfo={false}
            status="success"
          />
        </div>
      )}

      <Space wrap>
        {status.phase === "idle" && (
          <Button
            type="primary"
            icon={<PlayCircleOutlined />}
            onClick={() => runEngineAction(() => api.start())}
          >
            开始提醒
          </Button>
        )}
        {status.phase === "paused" && (
          <Button
            type="primary"
            icon={<PlayCircleOutlined />}
            onClick={() => runEngineAction(() => api.resume())}
          >
            恢复提醒
          </Button>
        )}
        {status.phase === "suspended" && (
          <Button
            type="primary"
            icon={<PlayCircleOutlined />}
            onClick={() => runEngineAction(() => api.resume())}
          >
            恢复提醒
          </Button>
        )}
        {(status.phase === "working" || status.phase === "reminding") && (
          <Button icon={<PauseCircleOutlined />} onClick={() => runEngineAction(() => api.pause())}>
            暂停提醒
          </Button>
        )}
        {status.phase === "reminding" && (
          <>
            <Button
              type="primary"
              onClick={() =>
                runEngineAction(() =>
                  api.startBreak(activities.find((a) => a.enabled)?.id ?? null),
                )
              }
            >
              开始活动
            </Button>
            <Button onClick={() => runEngineAction(() => api.acknowledge())}>已经活动</Button>
            <Button onClick={() => runEngineAction(() => api.snooze())}>稍后提醒</Button>
            <Button onClick={() => runEngineAction(() => api.skip())}>跳过本次</Button>
          </>
        )}
        {status.phase === "break" && (
          <Button onClick={() => runEngineAction(() => api.finishBreak(false))}>提前结束</Button>
        )}
        {status.phase !== "break" && (
          <Button onClick={() => runEngineAction(() => api.startBreak(null))}>立即活动</Button>
        )}
        {status.phase !== "idle" && (
          <Button danger icon={<StopOutlined />} onClick={() => runEngineAction(() => api.stop())}>
            停止计时
          </Button>
        )}
        <Button onClick={() => runEngineAction(() => api.setDnd(status.dndRemainingSeconds ? 0 : 30))}>
          {status.dndRemainingSeconds ? "关闭勿扰" : "勿扰 30 分钟"}
        </Button>
      </Space>
    </Space>
  );
}
