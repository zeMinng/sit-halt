/**
 * SitHalt 配色：「竹青 + 暖白」。
 *
 * 取色思路：
 * - 主色用低饱和的竹青（jade），久坐提醒类工具长时间停留，重色会疲劳；
 * - 中性色带一点绿灰（而非纯灰），与主色同源，界面更整；
 * - 选中态统一用「主色 8% 淡染」，不再用高饱和大色块；
 * - 阴影统一为「冷绿灰 + 低透明度 + 大扩散」，避免默认黑色的厚重感。
 */

import { theme as antTheme, type ThemeConfig } from "antd";

const PRIMARY = "#12876F";
const PRIMARY_SOFT = "#E7F4EF";
const BORDER = "#DDE7E2";
const BORDER_SOFT = "#EAF0ED";
const TEXT = "#152420";
const TEXT_SECONDARY = "#53635D";
const TEXT_TERTIARY = "#88968F";
const LAYOUT_BG = "#ECF3EF";
const SURFACE = "#FFFFFF";

/** 深色模式用同一套色相提亮，避免切主题后是另一套观感。 */
const DARK = {
  primary: "#3FBFA4",
  primarySoft: "#173A32",
  border: "#2A3B35",
  borderSoft: "#22302B",
  text: "#E8F0EC",
  textSecondary: "#A9BCB4",
  textTertiary: "#7C9089",
  layoutBg: "#101614",
  surface: "#171F1C",
};

export function buildTheme(dark: boolean): ThemeConfig {
  const p = dark ? DARK.primary : PRIMARY;
  const soft = dark ? DARK.primarySoft : PRIMARY_SOFT;

  return {
    algorithm: dark ? antTheme.darkAlgorithm : antTheme.defaultAlgorithm,
    token: {
      colorPrimary: p,
      colorInfo: p,
      colorLink: p,
      colorSuccess: dark ? "#5FBF8A" : "#2E9E62",
      colorWarning: dark ? "#E0A852" : "#D08A2C",
      colorError: dark ? "#E37B72" : "#C9554C",

      colorBgLayout: dark ? DARK.layoutBg : LAYOUT_BG,
      colorBgContainer: dark ? DARK.surface : SURFACE,
      colorBgElevated: dark ? DARK.surface : SURFACE,
      colorBorder: dark ? DARK.border : BORDER,
      colorBorderSecondary: dark ? DARK.borderSoft : BORDER_SOFT,
      colorSplit: dark ? "rgba(255,255,255,0.08)" : "rgba(21,49,41,0.08)",

      colorText: dark ? DARK.text : TEXT,
      colorTextSecondary: dark ? DARK.textSecondary : TEXT_SECONDARY,
      colorTextTertiary: dark ? DARK.textTertiary : TEXT_TERTIARY,
      colorTextQuaternary: dark ? "#5D726B" : "#AFBCB5",

      colorFill: dark ? "rgba(255,255,255,0.08)" : "#EDF2EF",
      colorFillSecondary: dark ? "rgba(255,255,255,0.06)" : "#F1F5F3",
      colorFillTertiary: dark ? "rgba(255,255,255,0.04)" : "#F5F8F6",
      colorFillQuaternary: dark ? "rgba(255,255,255,0.03)" : "#F9FBFA",

      borderRadius: 8,
      borderRadiusLG: 14,
      borderRadiusSM: 6,
      fontSize: 14,
      wireframe: false,

      // 焦点光晕改成主色淡染，替代 antd 默认的蓝色描边
      controlOutline: dark ? "rgba(63,191,164,0.18)" : "rgba(18,135,111,0.14)",
      controlOutlineWidth: 3,

      // 阴影统一：无纯黑，用冷绿灰，低透明度大扩散
      boxShadow: dark
        ? "0 2px 4px rgba(0,0,0,0.30), 0 10px 28px rgba(0,0,0,0.36)"
        : "0 2px 4px rgba(20,45,38,0.04), 0 10px 28px rgba(20,45,38,0.08)",
      boxShadowSecondary: dark
        ? "0 1px 2px rgba(0,0,0,0.28), 0 6px 18px rgba(0,0,0,0.30)"
        : "0 1px 2px rgba(20,45,38,0.04), 0 6px 18px rgba(20,45,38,0.06)",
      boxShadowTertiary: dark
        ? "0 1px 2px rgba(0,0,0,0.26)"
        : "0 1px 2px rgba(20,45,38,0.05)",
    },
    components: {
      Layout: {
        bodyBg: dark ? DARK.layoutBg : LAYOUT_BG,
        headerBg: "transparent",
        siderBg: dark ? DARK.surface : SURFACE,
        headerPadding: "0 24px",
      },
      Menu: {
        itemBg: "transparent",
        itemColor: dark ? DARK.textSecondary : "#3C4A45",
        itemHoverBg: dark ? "rgba(63,191,164,0.10)" : "#F1F6F3",
        itemHoverColor: dark ? DARK.text : "#1B2A25",
        itemSelectedBg: soft,
        itemSelectedColor: dark ? DARK.primary : "#0C6A57",
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
        headerBg: dark ? "rgba(255,255,255,0.04)" : "#F6FAF8",
        headerColor: dark ? DARK.textSecondary : "#41504A",
        headerSplitColor: "transparent",
        headerBorderRadius: 10,
        rowHoverBg: dark ? "rgba(63,191,164,0.08)" : "#F4F9F6",
        borderColor: dark ? DARK.borderSoft : "#EDF2EF",
        cellPaddingBlockSM: 8,
      },
      Segmented: {
        trackBg: dark ? "rgba(255,255,255,0.06)" : "#EEF3F0",
        itemSelectedBg: dark ? DARK.surface : SURFACE,
        itemSelectedColor: dark ? DARK.primary : "#0C6A57",
        itemColor: dark ? DARK.textSecondary : TEXT_SECONDARY,
        trackPadding: 3,
      },
      Button: {
        primaryShadow: dark
          ? "0 2px 6px rgba(63,191,164,0.24)"
          : "0 2px 6px rgba(18,135,111,0.22)",
        defaultShadow: "none",
        dangerShadow: "none",
        fontWeight: 500,
      },
      Tag: {
        defaultBg: dark ? "rgba(255,255,255,0.06)" : "#F2F6F4",
        defaultColor: dark ? DARK.textSecondary : TEXT_SECONDARY,
      },
      Modal: {
        headerBg: "transparent",
        titleFontSize: 17,
        borderRadiusLG: 16,
      },
      Progress: {
        defaultColor: p,
        remainingColor: dark ? "rgba(255,255,255,0.10)" : "#EAF0ED",
      },
      Alert: {
        borderRadiusLG: 10,
      },
      Divider: {
        colorSplit: dark ? "rgba(255,255,255,0.08)" : "rgba(21,49,41,0.08)",
      },
    },
  };
}
