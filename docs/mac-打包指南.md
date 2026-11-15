# macOS 打包指南

## 为什么不能在 Windows 上直接出 mac 包

Tauri 2 产出 macOS 的 `.app` / `.dmg` **必须在 macOS 上构建**：链接 CoreGraphics / AppKit / WebKit 等系统框架、生成 `.app` bundle 与 DMG、代码签名与公证都依赖 macOS 工具链（Xcode Command Line Tools、codesign、hdiutil、notarytool）。Windows / Linux 主机上即使加装 Rust 的 apple target，也无法完成链接与打包。

当前开发机为 Windows，已在代码层完成 macOS 适配并通过 Windows target 编译测试；**下面的构建步骤需要在你的 Mac 上执行**。

## 一、Mac 环境准备（首次）

1. **Xcode Command Line Tools**
   ```bash
   xcode-select --install
   ```

2. **Rust 工具链**（如未装）
   ```bash
   # 安装 rustup（https://rustup.rs），装完重开终端
   rustup target add aarch64-apple-darwin   # Apple Silicon（M 系列）
   rustup target add x86_64-apple-darwin   # Intel 芯片；只打本机架构可省略
   ```

3. **Node 与 pnpm**
   ```bash
   node -v        # 需要 Node 18+
   npm i -g pnpm  # 或用 fnm/volta 管理
   ```

## 二、把项目拷到 Mac

从 Windows 拷贝源码目录时，**不要带** `node_modules/`、`dist/`、`src-tauri/target/`（这些是 Windows 产物，体积大且在 Mac 上需重新生成）。拷贝后在 Mac 项目根目录：

```bash
pnpm install
```

## 三、先在 Mac 上跑通开发模式

首次在 Mac 编译，先验证 CoreGraphics 空闲检测 FFI 能正常链接、应用能启动：

```bash
pnpm tauri dev
```

重点核对：
- 窗口正常打开、前端页面正常渲染
- 托盘 / 菜单栏图标出现
- 通知能弹出（首次会请求通知权限）
- 全局快捷键可注册并触发
- 空闲检测：静置一段时间后，引擎能识别“离开座位”

## 四、打包

在项目根目录执行：

```bash
# 同时产出 .app 和 .dmg（默认本机架构）
pnpm tauri build

# 只出 .app
pnpm tauri build --bundles app

# 只出 .dmg
pnpm tauri build --bundles dmg
```

产物位置：

```
src-tauri/target/release/bundle/macos/SitHalt.app
src-tauri/target/release/bundle/dmg/SitHalt_0.1.0_<arch>.dmg
```

### 通用二进制（同时支持 Apple Silicon 与 Intel，可选）

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm tauri build --target universal-apple-darwin
```

> 注意：通用二进制 / App Store 分发建议把 `tauri.conf.json > bundle > macOS > minimumSystemVersion` 提到 `12.0`；当前设为 `10.15`，仅本机架构安装可保持。

## 五、未签名包在本机打开

未经过 Apple 签名的包，首次双击会被 Gatekeeper 拦截。本机自用可任选其一：

- 在访达中**右键（或按住 Control 点按）`SitHalt.app` → 打开**，在弹窗里再点“打开”；
- 或移除隔离属性后再启动：
  ```bash
  xattr -dr com.apple.quarantine /Applications/SitHalt.app
  ```

## 六、分发给他人：签名与公证（可选）

要让别的 Mac 不被“应用已损坏 / 无法打开”拦截，需要 Apple Developer 证书签名并公证：

1. 在 Apple Developer 后台申请证书，安装到 Mac 钥匙串；查看身份：
   ```bash
   security find-identity -v -p codesigning
   ```
2. 在 `tauri.conf.json > bundle > macOS` 配置 `signingIdentity`、`hardenedRuntime`、`entitlements`（详见 Tauri 官方 macOS Code Signing 文档）。
3. 设置环境变量后重新构建并公证：
   ```bash
   export APPLE_ID="你的 Apple ID 邮箱"
   export APPLE_PASSWORD="应用专用密码"
   export APPLE_TEAM_ID="团队 ID"
   pnpm tauri build --bundles dmg
   ```

## 七、Mac 真机待闭环清单

代码层已适配，但以下项必须在 Mac 上首次编译 / 运行时确认：

- [ ] `cargo build` 阶段 CoreGraphics FFI 链接通过（无 framework 链接错误）
- [ ] 空闲检测读秒正常，静置后能挂起提醒、有输入后恢复
- [ ] 菜单栏托盘图标、菜单点击、左键显示主窗口正常
- [ ] 通知弹出与权限授权流程正常
- [ ] 开机自启（LaunchAgent）可启用 / 关闭
- [ ] 全局快捷键（默认 `Cmd+Option+S/P/B/D`）注册与触发正常
- [ ] 关窗隐藏到托盘、进程不退出
- [ ] 菜单栏模板图标：当前为黑底方块，深浅菜单栏下观感待确认；如需自动深浅自适应，需补一张透明底单色 Z 素材后开启 `icon_as_template`
- [ ] Dock 图标是否隐藏（纯菜单栏 Accessory 模式）待产品决策
