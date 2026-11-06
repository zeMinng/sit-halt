import { useEffect, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Descriptions,
  Divider,
  InputNumber,
  List,
  Radio,
  Slider,
  Space,
  Switch,
  Tag,
  Typography,
} from "antd";

import { api } from "@/api/ipc";
import { useAppStore } from "@/stores/useAppStore";
import type { BackupInfo } from "@/types/contracts";

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

export function SettingsPage() {
  const settings = useAppStore((s) => s.settings);
  const appInfo = useAppStore((s) => s.appInfo);
  const dbHealth = useAppStore((s) => s.dbHealth);
  const updateSetting = useAppStore((s) => s.updateSetting);
  const [healthRefreshing, setHealthRefreshing] = useState(false);
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [backupLoading, setBackupLoading] = useState(false);
  const [backupResult, setBackupResult] = useState<string | null>(null);

  useEffect(() => {
    api
      .listBackups()
      .then(setBackups)
      .catch(() => setBackups([]));
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

  const soundVolume = Number(settings.soundVolume ?? 0.7);

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
        <List
          size="small"
          bordered
          locale={{ emptyText: "还没有备份文件" }}
          dataSource={backups}
          renderItem={(item) => (
            <List.Item>
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

      <Card title="关于应用" size="small">
        <Descriptions size="small" column={2}>
          <Descriptions.Item label="应用名称">SitHalt · 息坐</Descriptions.Item>
          <Descriptions.Item label="版本">{appInfo?.version ?? "0.1.0"}</Descriptions.Item>
          <Descriptions.Item label="应用标识">{appInfo?.identifier ?? "—"}</Descriptions.Item>
          <Descriptions.Item label="当前平台">{appInfo?.platform ?? "—"}</Descriptions.Item>
        </Descriptions>
        <Divider style={{ margin: "8px 0" }} />
        <Typography.Text type="secondary">
          本地优先：无账号、无服务器、不上传任何数据；键鼠空闲检测只读取系统空闲时长，不记录输入内容。
        </Typography.Text>
      </Card>
    </Space>
  );
}
