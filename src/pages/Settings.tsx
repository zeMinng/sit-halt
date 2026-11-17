import { useEffect, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Descriptions,
  Divider,
  InputNumber,
  List,
  Modal,
  Radio,
  Select,
  Slider,
  Space,
  Switch,
  Tag,
  Typography,
} from "antd";

import { api, fileDialog } from "@/api/ipc";
import { useAppStore } from "@/stores/useAppStore";
import type { BackupInfo, ShortcutAction, ShortcutMap } from "@/types/contracts";
import { playReminderSound, SOUND_WAVE_OPTIONS, type SoundWave } from "@/utils/sound";

function SettingRow({
  title,
  description,
  control,
}: {
  title: string;
  description?: string;
  control: React.ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        flexWrap: "wrap",
        gap: 12,
        padding: "10px 0",
      }}
    >
      <div style={{ minWidth: 0, flex: "1 1 220px" }}>
        <div>{title}</div>
        {description ? (
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {description}
          </Typography.Text>
        ) : null}
      </div>
      <div style={{ flex: "0 0 auto" }}>{control}</div>
    </div>
  );
}

const SHORTCUT_ACTIONS: { action: ShortcutAction; label: string; description: string }[] = [
  { action: "toggleMain", label: "显示 / 隐藏主界面", description: "在托盘后台与主窗口之间切换" },
  { action: "pauseResume", label: "暂停 / 恢复提醒", description: "暂停后停止久坐计时，恢复后继续" },
  { action: "startBreak", label: "立即活动", description: "跳过当前周期，直接开始一次活动" },
  { action: "dnd", label: "勿扰 30 分钟", description: "开启 30 分钟免打扰，期间不触发提醒" },
];

const MODIFIER_KEYS = ["Control", "Alt", "Shift", "Meta"];

const IS_MAC = typeof navigator !== "undefined" && /mac/i.test(navigator.platform);
const MODIFIER_HINT = IS_MAC ? "Cmd / Option / Shift" : "Ctrl / Alt / Shift";

/** 从按键事件拼出 accelerator（如 Ctrl+Alt+S）；未带修饰键或只按了修饰键时返回 null。 */
function comboFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.includes(e.key)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Meta");
  if (parts.length === 0) return null;
  const main = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  parts.push(main);
  return parts.join("+");
}

export function SettingsPage() {
  const settings = useAppStore((s) => s.settings);
  const appInfo = useAppStore((s) => s.appInfo);
  const dbHealth = useAppStore((s) => s.dbHealth);
  const updateSetting = useAppStore((s) => s.updateSetting);
  const refreshPlans = useAppStore((s) => s.refreshPlans);
  const refreshActivities = useAppStore((s) => s.refreshActivities);
  const refreshToday = useAppStore((s) => s.refreshToday);
  const refreshStatus = useAppStore((s) => s.refreshStatus);
  const refreshSettings = useAppStore((s) => s.refreshSettings);
  const [healthRefreshing, setHealthRefreshing] = useState(false);
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [backupLoading, setBackupLoading] = useState(false);
  const [backupResult, setBackupResult] = useState<string | null>(null);
  const [restoreTarget, setRestoreTarget] = useState<BackupInfo | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [restoreResult, setRestoreResult] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [ioResult, setIoResult] = useState<string | null>(null);
  const [updateNote, setUpdateNote] = useState<string | null>(null);
  const [importTarget, setImportTarget] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [resetOpen, setResetOpen] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [shortcuts, setShortcuts] = useState<ShortcutMap | null>(null);
  const [recordingAction, setRecordingAction] = useState<ShortcutAction | null>(null);
  const [shortcutMessage, setShortcutMessage] = useState<string | null>(null);

  useEffect(() => {
    api
      .listBackups()
      .then(setBackups)
      .catch(() => setBackups([]));
    api
      .getShortcuts()
      .then(setShortcuts)
      .catch(() => setShortcuts(null));
  }, []);

  const runBackup = async () => {
    setBackupLoading(true);
    try {
      const info = await api.backupNow();
      setBackups(await api.listBackups());
      setBackupResult(`已生成备份：${info.file}（${(info.sizeBytes / 1024 / 1024).toFixed(2)} MB）`);
    } catch (error) {
      setBackupResult(`备份失败：${String(error)}`);
    } finally {
      setBackupLoading(false);
    }
  };

  const runRestore = async () => {
    if (!restoreTarget) return;
    setRestoring(true);
    try {
      const fallback = await api.restoreBackup(restoreTarget.file);
      setBackups(await api.listBackups());
      setRestoreTarget(null);
      setRestoreResult(
        `已准备恢复到所选备份；当前数据已先兜底备份（${(fallback.sizeBytes / 1024 / 1024).toFixed(2)} MB）。请重启应用后完成恢复。`,
      );
    } catch (error) {
      setRestoreResult(`恢复失败：${String(error)}`);
    } finally {
      setRestoring(false);
    }
  };

  const runExportJson = async () => {
    try {
      const target = await fileDialog.saveJson();
      if (!target) return;
      setExporting(true);
      await api.exportJson(target);
      setIoResult(`已导出全部数据：${target}`);
    } catch (error) {
      setIoResult(`导出失败：${String(error)}`);
    } finally {
      setExporting(false);
    }
  };

  const runExportCsv = async () => {
    try {
      const target = await fileDialog.saveCsv();
      if (!target) return;
      setExporting(true);
      await api.exportCsv(target);
      setIoResult(`已导出活动记录：${target}`);
    } catch (error) {
      setIoResult(`导出失败：${String(error)}`);
    } finally {
      setExporting(false);
    }
  };

  const pickImport = async () => {
    try {
      const source = await fileDialog.openJson();
      if (source) setImportTarget(source);
    } catch (error) {
      setIoResult(`选择文件失败：${String(error)}`);
    }
  };

  const runImport = async () => {
    if (!importTarget) return;
    setImporting(true);
    try {
      const fallback = await api.importJson(importTarget);
      setImportTarget(null);
      await Promise.all([refreshPlans(), refreshActivities(), refreshToday(), refreshStatus(), refreshSettings()]);
      setIoResult(
        `导入完成；当前数据已先兜底备份（${(fallback.sizeBytes / 1024 / 1024).toFixed(2)} MB）。建议重启应用，使计划引擎按新数据运行。`,
      );
    } catch (error) {
      setIoResult(`导入失败：${String(error)}`);
    } finally {
      setImporting(false);
    }
  };

  const runReset = async () => {
    setResetting(true);
    try {
      await api.resetAllData();
      setResetOpen(false);
      await Promise.all([refreshPlans(), refreshActivities(), refreshToday(), refreshStatus(), refreshSettings()]);
      api.getShortcuts().then(setShortcuts).catch(() => setShortcuts(null));
      setIoResult("已恢复出厂：全部数据已清空，并重新生成内置计划与活动。");
    } catch (error) {
      setIoResult(`重置失败：${String(error)}`);
    } finally {
      setResetting(false);
    }
  };

  const saveShortcut = async (action: ShortcutAction, combo: string) => {
    try {
      setShortcuts(await api.setShortcut(action, combo));
      setShortcutMessage(null);
    } catch (error) {
      setShortcutMessage(`绑定失败：${String(error)}`);
    }
  };

  const resetShortcuts = async () => {
    try {
      setShortcuts(await api.resetShortcuts());
      setShortcutMessage(null);
    } catch (error) {
      setShortcutMessage(`恢复默认失败：${String(error)}`);
    }
  };

  // 录制期间全局捕获按键，拼出组合键后直接提交。
  useEffect(() => {
    if (!recordingAction) return;
    const handler = (e: KeyboardEvent) => {
      const combo = comboFromEvent(e);
      if (!combo) return;
      e.preventDefault();
      void saveShortcut(recordingAction, combo);
      setRecordingAction(null);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [recordingAction]);

  const soundVolume = Number(settings.soundVolume ?? 0.7);
  const soundWave = (settings.soundWave as SoundWave) || "sine";

  const previewSound = () => playReminderSound(soundVolume, soundWave);

  const checkUpdate = () =>
    setUpdateNote("当前为开发版本，暂未接入在线更新通道；正式发布后将在此检查新版本。");

  const toggleAutostart = async (checked: boolean) => {
    try {
      const enabled = await api.setAutostart(checked);
      await updateSetting("autostart", enabled);
    } catch {
      await updateSetting("autostart", checked);
    }
  };

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Card title="通用设置" size="small">
        <SettingRow
          title="开机自动启动"
          description="系统启动后在托盘后台运行，不主动弹出主窗口"
          control={
            <Switch checked={Boolean(settings.autostart)} onChange={(checked) => void toggleAutostart(checked)} />
          }
        />
        <Divider style={{ margin: "4px 0" }} />
        <SettingRow
          title="关闭窗口时隐藏到托盘"
          description="为保证提醒持续生效，当前版本固定为隐藏而不是退出（如需退出请使用托盘菜单）"
          control={<Switch checked disabled />}
        />
        <Divider style={{ margin: "4px 0" }} />
        <SettingRow
          title="启动时显示主窗口"
          description="关闭后会静默启动，只在托盘出现图标"
          control={
            <Switch
              checked={Boolean(settings.showOnStartup)}
              onChange={(checked) => void updateSetting("showOnStartup", checked)}
            />
          }
        />
      </Card>

      <Card title="提醒设置" size="small">
        <SettingRow
          title="系统通知"
          description="提醒触发时通过操作系统通知中心提示（受系统通知权限影响）"
          control={
            <Switch
              checked={settings.notificationEnabled !== false}
              onChange={(checked) => void updateSetting("notificationEnabled", checked)}
            />
          }
        />
        <Divider style={{ margin: "4px 0" }} />
        <SettingRow
          title="提示音"
          description="提醒触发时播放三声短促提示音"
          control={
            <Switch
              checked={settings.soundEnabled !== false}
              onChange={(checked) => void updateSetting("soundEnabled", checked)}
            />
          }
        />
        <Divider style={{ margin: "4px 0" }} />
        <SettingRow
          title="提示音音量"
          description="0 为静音，1 为最大音量"
          control={
            <Slider
              style={{ width: 200 }}
              min={0.05}
              max={1}
              step={0.05}
              value={soundVolume}
              onChangeComplete={(value) => void updateSetting("soundVolume", value)}
            />
          }
        />
        <Divider style={{ margin: "4px 0" }} />
        <SettingRow
          title="提示音音色"
          description="不同波形对应不同听感，可点试听"
          control={
            <Space>
              <Select
                style={{ width: 160 }}
                value={soundWave}
                options={SOUND_WAVE_OPTIONS}
                onChange={(value) => void updateSetting("soundWave", value)}
              />
              <Button size="small" onClick={previewSound}>
                试听
              </Button>
            </Space>
          }
        />
      </Card>

      <Card
        title="快捷键设置"
        size="small"
        extra={
          <Button size="small" onClick={() => void resetShortcuts()}>
            恢复默认
          </Button>
        }
      >
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 8 }}
          message={`快捷键在应用后台运行时全局生效；点击按钮后按下组合键即可绑定，需包含 ${MODIFIER_HINT} 至少一个修饰键。被其它应用占用时会提示注册失败。`}
        />
        {SHORTCUT_ACTIONS.map(({ action, label, description }, index) => {
          const recording = recordingAction === action;
          return (
            <div key={action}>
              {index > 0 ? <Divider style={{ margin: "4px 0" }} /> : null}
              <SettingRow
                title={label}
                description={description}
                control={
                  <Button
                    size="small"
                    type={recording ? "primary" : "default"}
                    onClick={() => setRecordingAction(recording ? null : action)}
                  >
                    {recording ? "请按下组合键…" : shortcuts?.[action] || "未绑定"}
                  </Button>
                }
              />
            </div>
          );
        })}
        {shortcutMessage ? (
          <Alert
            style={{ marginTop: 8 }}
            type={shortcutMessage.startsWith("绑定失败") ? "warning" : "success"}
            showIcon
            message={shortcutMessage}
            closable
            onClose={() => setShortcutMessage(null)}
          />
        ) : null}
      </Card>

      <Card title="外观设置" size="small">
        <SettingRow
          title="主题"
          description="跟随系统时会随操作系统深浅色自动切换"
          control={
            <Radio.Group
              value={(settings.theme as string) ?? "system"}
              onChange={(event) => void updateSetting("theme", event.target.value)}
            >
              <Radio.Button value="light">浅色</Radio.Button>
              <Radio.Button value="dark">深色</Radio.Button>
              <Radio.Button value="system">跟随系统</Radio.Button>
            </Radio.Group>
          }
        />
        <Divider style={{ margin: "4px 0" }} />
        <SettingRow
          title="界面语言"
          description="中英文国际化属于 Phase 4，当前版本仅简体中文"
          control={<Tag>简体中文</Tag>}
        />
      </Card>

      <Card
        title="数据管理"
        size="small"
        extra={
          <Button
            size="small"
            loading={healthRefreshing}
            onClick={async () => {
              setHealthRefreshing(true);
              try {
                const health = await api.dbHealth();
                useAppStore.setState({ dbHealth: health });
              } finally {
                setHealthRefreshing(false);
              }
            }}
          >
            刷新状态
          </Button>
        }
      >
        <Descriptions size="small" column={1} bordered>
          <Descriptions.Item label="数据库文件">
            <Typography.Text copyable>{appInfo?.dbPath ?? "未知"}</Typography.Text>
          </Descriptions.Item>
          <Descriptions.Item label="数据目录">
            <Typography.Text copyable>{appInfo?.dataDir ?? "未知"}</Typography.Text>
          </Descriptions.Item>
          <Descriptions.Item label="迁移记录">
            {dbHealth?.migrationCount ?? 0} 条 · 数据表 {dbHealth?.tableCount ?? 0} 张
          </Descriptions.Item>
        </Descriptions>
        <Alert
          style={{ marginTop: 12 }}
          type="info"
          showIcon
          message="本地备份"
          description="启动时按间隔自动备份到数据库同级的 backups 目录（使用 SQLite 的 VACUUM INTO 生成一致性快照）；超过保留份数时删除最早的备份。数据不进云端。"
        />
        <Divider style={{ margin: "12px 0" }} />
        <SettingRow
          title="自动备份间隔（天）"
          description="0 表示关闭自动备份"
          control={
            <InputNumber
              min={0}
              max={30}
              style={{ width: 100 }}
              value={Number(settings.backupIntervalDays ?? 7)}
              onChange={(value) => void updateSetting("backupIntervalDays", value ?? 7)}
            />
          }
        />
        <Divider style={{ margin: "4px 0" }} />
        <SettingRow
          title="保留备份份数"
          description="超出后自动删除最早的一份"
          control={
            <InputNumber
              min={1}
              max={30}
              style={{ width: 100 }}
              value={Number(settings.backupKeep ?? 5)}
              onChange={(value) => void updateSetting("backupKeep", value ?? 5)}
            />
          }
        />
        <Divider style={{ margin: "4px 0" }} />
        <Space style={{ marginBottom: 8 }}>
          <Button type="primary" size="small" loading={backupLoading} onClick={() => void runBackup()}>
            立即备份
          </Button>
          <Typography.Text type="secondary">共 {backups.length} 份备份</Typography.Text>
        </Space>
        <Divider style={{ margin: "4px 0" }} />
        <Space wrap style={{ marginBottom: 8 }}>
          <Button size="small" loading={exporting} onClick={() => void runExportJson()}>
            导出 JSON
          </Button>
          <Button size="small" loading={exporting} onClick={() => void runExportCsv()}>
            导出 CSV
          </Button>
          <Button size="small" danger loading={importing} onClick={() => void pickImport()}>
            导入 JSON
          </Button>
        </Space>
        <Divider style={{ margin: "4px 0" }} />
        <Space style={{ marginBottom: 8 }}>
          <Button danger size="small" loading={resetting} onClick={() => setResetOpen(true)}>
            删除所有数据
          </Button>
          <Typography.Text type="secondary">清空计划、活动库、全部记录与设置，恢复到首次启动状态</Typography.Text>
        </Space>
        {ioResult ? (
          <Alert
            style={{ marginBottom: 8 }}
            type={ioResult.includes("失败") ? "warning" : "success"}
            showIcon
            message={ioResult}
            closable
            onClose={() => setIoResult(null)}
          />
        ) : null}
        {backupResult ? (
          <Alert
            style={{ marginBottom: 8 }}
            type={backupResult.startsWith("备份失败") ? "warning" : "success"}
            showIcon
            message={backupResult}
            closable
            onClose={() => setBackupResult(null)}
          />
        ) : null}
        {restoreResult ? (
          <Alert
            style={{ marginBottom: 8 }}
            type={restoreResult.startsWith("恢复失败") ? "warning" : "success"}
            showIcon
            message={restoreResult}
            closable
            onClose={() => setRestoreResult(null)}
          />
        ) : null}
        <List
          size="small"
          bordered
          locale={{ emptyText: "还没有备份文件" }}
          dataSource={backups}
          renderItem={(item) => (
            <List.Item
              actions={[
                <Button
                  key="restore"
                  size="small"
                  danger
                  onClick={() => {
                    setRestoreResult(null);
                    setRestoreTarget(item);
                  }}
                >
                  恢复
                </Button>,
              ]}
            >
              <Space direction="vertical" size={0} style={{ width: "100%" }}>
                <Typography.Text ellipsis copyable style={{ fontSize: 12 }}>
                  {item.file}
                </Typography.Text>
                <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                  {(item.sizeBytes / 1024 / 1024).toFixed(2)} MB · {item.createdAt}
                </Typography.Text>
              </Space>
            </List.Item>
          )}
        />
      </Card>

      <Card
        title="关于应用"
        size="small"
        extra={
          <Button size="small" onClick={checkUpdate}>
            检查更新
          </Button>
        }
      >
        <div className="about-brand">
          <img className="about-brand-logo" src="/logo.png" alt="SitHalt" />
          <div className="about-brand-text">
            <div className="about-brand-name">SitHalt · 息坐</div>
            <div className="about-brand-sub">本地优先的久坐间歇提醒工具</div>
          </div>
        </div>
        <Descriptions size="small" column={2}>
          <Descriptions.Item label="应用名称">SitHalt · 息坐</Descriptions.Item>
          <Descriptions.Item label="版本">{appInfo?.version ?? "0.1.0"}</Descriptions.Item>
          <Descriptions.Item label="作者">{appInfo?.author ?? "zeMinng"}</Descriptions.Item>
          <Descriptions.Item label="应用标识">{appInfo?.identifier ?? "—"}</Descriptions.Item>
          <Descriptions.Item label="当前平台">{appInfo?.platform ?? "—"}</Descriptions.Item>
        </Descriptions>
        <Divider style={{ margin: "8px 0" }} />
        <Typography.Text type="secondary">
          定时提醒你从座位起身活动，跟踪久坐与活动情况，帮助养成间歇起身、放松肩颈腰背的习惯。本地优先：无账号、无服务器、不上传任何数据；键鼠空闲检测只读取系统空闲时长，不记录输入内容。
        </Typography.Text>
        {updateNote && (
          <Alert style={{ marginTop: 10 }} type="info" showIcon message={updateNote} />
        )}
        <Divider style={{ margin: "8px 0" }} />
        <Typography.Text type="secondary">
          本应用基于 React、Ant Design、Tauri 及 Rust 生态等开源组件构建，各第三方组件许可证随其依赖发布；本项目开源许可待正式发布时确定。
        </Typography.Text>
      </Card>

      <Modal
        open={Boolean(restoreTarget)}
        title="恢复备份"
        centered
        okText="确认恢复"
        okButtonProps={{ danger: true, loading: restoring }}
        cancelText="取消"
        onCancel={() => setRestoreTarget(null)}
        onOk={() => void runRestore()}
      >
        <Space direction="vertical" size={12} style={{ width: "100%" }}>
          <Typography.Text>
            将把应用数据恢复到所选备份：
          </Typography.Text>
          <Typography.Text ellipsis copyable type="secondary" style={{ fontSize: 12 }}>
            {restoreTarget?.file}
          </Typography.Text>
          <Alert
            type="warning"
            showIcon
            message="恢复前会自动备份当前数据，便于回退；本次恢复需重启应用后才会生效，重启前页面数据保持不变。"
          />
        </Space>
      </Modal>

      <Modal
        open={Boolean(importTarget)}
        title="导入 JSON 数据"
        centered
        okText="确认导入"
        okButtonProps={{ danger: true, loading: importing }}
        cancelText="取消"
        onCancel={() => setImportTarget(null)}
        onOk={() => void runImport()}
      >
        <Space direction="vertical" size={12} style={{ width: "100%" }}>
          <Typography.Text>将从以下文件导入全部业务数据，当前计划、活动与记录会被覆盖：</Typography.Text>
          <Typography.Text ellipsis copyable type="secondary" style={{ fontSize: 12 }}>
            {importTarget}
          </Typography.Text>
          <Alert
            type="warning"
            showIcon
            message="导入前会自动备份当前数据，便于回退；导入失败会自动回滚、原始数据不受影响。建议完成后重启应用。"
          />
        </Space>
      </Modal>

      <Modal
        open={resetOpen}
        title="删除所有数据"
        centered
        okText="确认删除"
        okButtonProps={{ danger: true, loading: resetting }}
        cancelText="取消"
        onCancel={() => setResetOpen(false)}
        onOk={() => void runReset()}
      >
        <Space direction="vertical" size={12} style={{ width: "100%" }}>
          <Typography.Text>
            将清空全部计划、活动库、活动 / 久坐记录、统计与个人设置，并重新生成内置计划和活动，等同恢复出厂。
          </Typography.Text>
          <Alert
            type="warning"
            showIcon
            message="此操作不可撤销，且本次不生成备份；建议先「导出 JSON」留存一份。删除后建议重启应用。"
          />
        </Space>
      </Modal>
    </Space>
  );
}
