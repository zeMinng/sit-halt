/**
 * SitHalt 配色：「暖橙 + 信息蓝 + 中性灰白」。
 *
 * 取色思路（对照参考界面）：
 * - 主操作 / 品牌强调用暖橙 #F97316；
 * - 选中态 / 链接 / 信息提示用 iOS 蓝 #0A84FF，选中卡片为蓝淡染底 #EAF3FF；
 * - 侧边栏浅灰 #F5F5F7，主内容与卡片纯白，边框中性灰；
 * - 深色模式用同一套色相提亮，中性深灰底，避免切主题后是另一套观感。
 */

import { theme as antTheme, type ThemeConfig } from "antd";

const PRIMARY = "#F97316";
const INFO = "#0A84FF";
const INFO_STRONG = "#0070D8";
const BORDER = "#E4E4E9";
const BORDER_SOFT = "#EEEEF1";
const TEXT = "#18181B";
const TEXT_SECONDARY = "#5B616E";
const TEXT_TERTIARY = "#9CA1AB";
const LAYOUT_BG = "#FFFFFF";
const SIDER_BG = "#F5F5F7";
const SURFACE = "#FFFFFF";
const MENU_SELECTED_BG = "#E9E9EF";

/** 深色模式用同一套色相提亮，避免切主题后是另一套观感。 */
const DARK = {
  primary: "#FB8A3D",
  info: "#409CFF",
  infoStrong: "#74B7FF",
  border: "#2C2C31",
  borderSoft: "#242428",
  text: "#ECEDEF",
  textSecondary: "#A6ABB5",
  textTertiary: "#7E838E",
  layoutBg: "#141416",
  siderBg: "#17171A",
  surface: "#1D1D20",
  menuSelectedBg: "#26262A",
};

export function buildTheme(dark: boolean): ThemeConfig {
  const p = dark ? DARK.primary : PRIMARY;
  const info = dark ? DARK.info : INFO;

  return {
    algorithm: dark ? antTheme.darkAlgorithm : antTheme.defaultAlgorithm,
    token: {
      colorPrimary: p,
      colorInfo: info,
      colorLink: info,
      colorSuccess: dark ? "#5FBF8A" : "#2E9E62",
      colorWarning: dark ? "#E0A852" : "#D08A2C",
      colorError: dark ? "#E37B72" : "#C9554C",

      colorBgLayout: dark ? DARK.layoutBg : LAYOUT_BG,
      colorBgContainer: dark ? DARK.surface : SURFACE,
      colorBgElevated: dark ? DARK.surface : SURFACE,
      colorBorder: dark ? DARK.border : BORDER,
      colorBorderSecondary: dark ? DARK.borderSoft : BORDER_SOFT,
      colorSplit: dark ? "rgba(255,255,255,0.08)" : "rgba(24,24,27,0.08)",

      colorText: dark ? DARK.text : TEXT,
      colorTextSecondary: dark ? DARK.textSecondary : TEXT_SECONDARY,
      colorTextTertiary: dark ? DARK.textTertiary : TEXT_TERTIARY,
      colorTextQuaternary: dark ? "#5D616B" : "#C2C6CD",

      colorFill: dark ? "rgba(255,255,255,0.08)" : "#F2F2F5",
      colorFillSecondary: dark ? "rgba(255,255,255,0.06)" : "#F5F5F7",
      colorFillTertiary: dark ? "rgba(255,255,255,0.04)" : "#F7F7F9",
      colorFillQuaternary: dark ? "rgba(255,255,255,0.03)" : "#FAFAFB",

      borderRadius: 8,
      borderRadiusLG: 14,
      borderRadiusSM: 6,
      fontSize: 14,
      wireframe: false,

      // 焦点光晕改成信息蓝淡染，替代 antd 默认的蓝色描边
      controlOutline: dark ? "rgba(64,156,255,0.18)" : "rgba(10,132,255,0.14)",
      controlOutlineWidth: 3,

      // 阴影统一：无纯黑，用中性灰，低透明度大扩散
      boxShadow: dark
        ? "0 2px 4px rgba(0,0,0,0.30), 0 10px 28px rgba(0,0,0,0.36)"
        : "0 2px 4px rgba(24,24,27,0.04), 0 10px 28px rgba(24,24,27,0.08)",
      boxShadowSecondary: dark
        ? "0 1px 2px rgba(0,0,0,0.28), 0 6px 18px rgba(0,0,0,0.30)"
        : "0 1px 2px rgba(24,24,27,0.04), 0 6px 18px rgba(24,24,27,0.06)",
      boxShadowTertiary: dark
        ? "0 1px 2px rgba(0,0,0,0.26)"
        : "0 1px 2px rgba(24,24,27,0.05)",
    },
    components: {
      Layout: {
        bodyBg: dark ? DARK.layoutBg : LAYOUT_BG,
        headerBg: "transparent",
        siderBg: dark ? DARK.siderBg : SIDER_BG,
        headerPadding: "0 24px",
      },
      Menu: {
        itemBg: "transparent",
        itemColor: dark ? DARK.textSecondary : "#3F4450",
        itemHoverBg: dark ? "rgba(255,255,255,0.06)" : "#F0F0F3",
        itemHoverColor: dark ? DARK.text : "#18181B",
        itemSelectedBg: dark ? DARK.menuSelectedBg : MENU_SELECTED_BG,
        itemSelectedColor: dark ? DARK.text : "#18181B",
        itemBorderRadius: 10,
        itemMarginInline: 10,
        itemHeight: 42,
        itemMarginBlock: 4,
        iconSize: 16,
        activeBarBorderWidth: 0,
      },
      Card: {
        headerBg: "transparent",
        headerFontSize: 15,
        headerFontSizeSM: 14,
        colorBorderSecondary: dark ? DARK.borderSoft : BORDER_SOFT,
      },
      Table: {
        headerBg: dark ? "rgba(255,255,255,0.04)" : "#F7F7F9",
        headerColor: dark ? DARK.textSecondary : "#414651",
        headerSplitColor: "transparent",
        headerBorderRadius: 10,
        rowHoverBg: dark ? "rgba(64,156,255,0.08)" : "#F5F5F7",
        borderColor: dark ? DARK.borderSoft : "#EEEEF1",
        cellPaddingBlockSM: 8,
      },
      Segmented: {
        trackBg: dark ? "rgba(255,255,255,0.06)" : "#F0F0F3",
        itemSelectedBg: dark ? DARK.surface : SURFACE,
        itemSelectedColor: dark ? DARK.infoStrong : INFO_STRONG,
        itemColor: dark ? DARK.textSecondary : TEXT_SECONDARY,
        trackPadding: 3,
      },
      Button: {
        primaryShadow: dark
          ? "0 2px 6px rgba(251,138,61,0.24)"
          : "0 2px 6px rgba(249,115,22,0.22)",
        defaultShadow: "none",
        dangerShadow: "none",
        fontWeight: 500,
      },
      Tag: {
        defaultBg: dark ? "rgba(255,255,255,0.06)" : "#F2F2F5",
        defaultColor: dark ? DARK.textSecondary : TEXT_SECONDARY,
      },
      Modal: {
        headerBg: "transparent",
        titleFontSize: 17,
        borderRadiusLG: 16,
      },
      Progress: {
        defaultColor: p,
        remainingColor: dark ? "rgba(255,255,255,0.10)" : "#EEEEF1",
      },
      Alert: {
        borderRadiusLG: 10,
      },
      Divider: {
        colorSplit: dark ? "rgba(255,255,255,0.08)" : "rgba(24,24,27,0.08)",
      },
    },
  };
}
