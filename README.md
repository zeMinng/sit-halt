# SitHalt · 息坐

伏案有时，休憩亦有时。

一款 Windows 桌面久坐提醒应用：按设定的工作节奏计时，到点用系统通知 + 页面提醒催你起身活动，并记录久坐与活动数据。

## 技术栈

| 层 | 选型 |
| --- | --- |
| 桌面壳 | Tauri 2（React 模板，未换骨架） |
| 前端 | React 19 + TypeScript + antd 6 + zustand + react-router-dom（HashRouter） |
| 图表 | echarts 6（`echarts/core` 按需注册） |
| 后端 | Rust（`src-tauri`）+ 30 秒级状态机 + 托盘 |
| 数据 | SQLite + SQLx 0.8（迁移、连接池，前端不直连数据库） |

## 目录

```
src/                    前端
  main.tsx              入口
  App.tsx               路由骨架（五页均懒加载）
  api/ipc.ts            invoke 封装，前端唯一数据入口
  types/contracts.ts    与 Rust 侧字段一一对应的类型
  stores/useAppStore.ts 全局状态 + 启动引导
  components/           StatusPanel / StatCard / TrendChart / ReminderModal / BreakDialog
  pages/                Dashboard / Reminder / Activities / Stats / Settings
  utils/                时间格式化、Web Audio 提示音

src-tauri/              Rust 侧
  migrations/           0001_init.sql（8 张表 + 索引）
  src/db/mod.rs         连接池、迁移、种子数据、全部 SQL
  src/engine/           计时状态机（machine.rs 为纯逻辑，mod.rs 为调度与事件）
  src/platform/mod.rs   Windows 空闲检测（GetLastInputInfo）
  src/commands.rs       34 个 IPC 命令
  src/lib.rs            Tauri 装配：托盘、通知、自启、事件注册

docs/                   实现进度与待确认项
scripts/                数据库/运行时排查脚本（Python）
```

## 开发与验证

```powershell
pnpm install
pnpm tauri dev        # 启动（前端 1420 + Rust 后端）

pnpm build            # 仅前端类型检查 + 打包
cargo check --all-targets --manifest-path src-tauri/Cargo.toml
cargo test --lib --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml
```

## 数据位置

- 数据库：`%APPDATA%\com.sithalt.app\sithalt.db`
- 备份：`%APPDATA%\com.sithalt.app\backups\sithalt-<时间戳>.db`

## 运行时约定

- 关窗只隐藏到托盘，不退出进程。
- 提醒可延后、跳过或直接开始活动，不强制阻塞。
- 非 Windows 平台空闲检测返回“未知”，不会误判为离开。

PRD 原文见 `reqDoc.md`，已落地能力与已知偏差见 `docs/实现进度与待确认项.md`。
