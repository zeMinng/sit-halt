import { memo, useEffect, useRef } from "react";
import * as echarts from "echarts/core";
import type { ECharts } from "echarts/core";
import { BarChart, LineChart } from "echarts/charts";
import {
  GridComponent,
  LegendComponent,
  TooltipComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";

import { useAppStore } from "@/stores/useAppStore";
import type { DailyPoint } from "@/types/contracts";
import { formatDuration } from "@/utils/format";

// 按需注册 echarts 模块：只用柱状图 + 折线图，避免打包完整 echarts。
echarts.use([
  BarChart,
  LineChart,
  GridComponent,
  LegendComponent,
  TooltipComponent,
  CanvasRenderer,
]);

interface TrendChartProps {
  data: DailyPoint[];
  height?: number;
}

/**
 * 直接用 echarts/core 初始化，不依赖任何 React 包装库：
 * 包装库以 CJS 分发时，其子路径的默认导入在 Vite 开发模式下可能被解析成模块对象，
 * 作为组件渲染会报 "Element type is invalid" 并让整页白屏。
 */
export const TrendChart = memo(function TrendChart({ data, height = 280 }: TrendChartProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<ECharts | null>(null);
  // 主题变化时重新取色（CSS 变量挂在 .theme-light / .theme-dark 上）
  const themeSetting = useAppStore((s) => s.settings.theme);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const chart = echarts.init(el);
    chartRef.current = chart;

    let resizeTimer: number | undefined;
    let lastWidth = 0;
    let lastHeight = 0;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const { width, height } = entry.contentRect;
      if (width === 0 || height === 0) return;
      // 尺寸没有真正变化时不 resize，避免无谓重绘把悬停提示清掉
      if (width === lastWidth && height === lastHeight) return;
      lastWidth = width;
      lastHeight = height;
      if (resizeTimer) window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => chart.resize(), 120);
    });
    observer.observe(el);

    return () => {
      observer.disconnect();
      if (resizeTimer) window.clearTimeout(resizeTimer);
      chart.dispose();
      chartRef.current = null;
    };
  }, []);

  // 按内容值比对：引用变化但数据没变时不重建配置，避免清空当前悬停提示
  const dataKey = JSON.stringify(data);

  useEffect(() => {
    const chart = chartRef.current;
    const el = containerRef.current;
    if (!chart || !el) return;

    // 从当前主题容器读取配色变量，保证深浅色都能对上界面
    const css = getComputedStyle(el);
    const primary = css.getPropertyValue("--app-primary").trim() || "#f97316";
    const primaryStrong = css.getPropertyValue("--app-primary-strong").trim() || "#e8640c";
    const infoStrong = css.getPropertyValue("--app-info-strong").trim() || "#0070d8";
    const textMuted = css.getPropertyValue("--app-text-muted").trim() || "#5b616e";
    const border = css.getPropertyValue("--app-border").trim() || "#e4e4e9";
    const surface = css.getPropertyValue("--app-surface").trim() || "#ffffff";
    const text = css.getPropertyValue("--app-text").trim() || "#18181b";

    // 第二根柱用信息蓝的浅色调，折线用暖琥珀，与「暖橙 + 信息蓝」同一家族
    const activityBar = themeSetting === "dark" ? "#2E5C86" : "#BFD9FF";
    const countLine = themeSetting === "dark" ? "#D7A45F" : "#C98A3C";

    const option = {
      textStyle: { fontFamily: "inherit", color: textMuted },
      tooltip: {
        trigger: "axis",
        backgroundColor: surface,
        borderColor: border,
        borderWidth: 1,
        padding: [8, 12],
        hideDelay: 300,
        textStyle: { color: text, fontSize: 12 },
        extraCssText:
          "border-radius:10px;box-shadow:0 6px 18px rgba(24,24,27,0.10);",
        axisPointer: { type: "line", lineStyle: { color: border } },
        formatter: (
          params: Array<{ seriesName: string; value: number; axisValue: string }>,
        ) => {
          const lines = params.map((p) => {
            if (p.seriesName === "活动次数") {
              return `${p.seriesName}：${p.value} 次`;
            }
            return `${p.seriesName}：${formatDuration(p.value)}`;
          });
          return `${params[0]?.axisValue ?? ""}<br/>${lines.join("<br/>")}`;
        },
      },
      legend: {
        data: ["久坐时长", "活动时长", "活动次数"],
        bottom: 0,
        itemWidth: 10,
        itemHeight: 10,
        itemGap: 18,
        textStyle: { color: textMuted, fontSize: 12 },
      },
      grid: { left: 8, right: 16, top: 24, bottom: 44, containLabel: true },
      xAxis: {
        type: "category",
        data: data.map((d) => d.date),
        axisTick: { show: false },
        axisLine: { lineStyle: { color: border } },
        axisLabel: { color: textMuted, fontSize: 11 },
      },
      yAxis: [
        {
          type: "value",
          name: "分钟",
          nameTextStyle: { color: textMuted, fontSize: 11 },
          axisLine: { show: false },
          splitLine: { lineStyle: { color: border, type: "dashed" } },
          axisLabel: {
            color: textMuted,
            fontSize: 11,
            formatter: (v: number) => `${Math.round(v / 60)}`,
          },
        },
        {
          type: "value",
          name: "次数",
          minInterval: 1,
          nameTextStyle: { color: textMuted, fontSize: 11 },
          axisLine: { show: false },
          splitLine: { show: false },
          axisLabel: { color: textMuted, fontSize: 11, formatter: "{value}" },
        },
      ],
      series: [
        {
          name: "久坐时长",
          type: "bar",
          barMaxWidth: 22,
          itemStyle: { color: primary, borderRadius: [6, 6, 0, 0] },
          emphasis: { itemStyle: { color: primaryStrong } },
          data: data.map((d) => d.sittingSeconds),
        },
        {
          name: "活动时长",
          type: "bar",
          barMaxWidth: 22,
          itemStyle: { color: activityBar, borderRadius: [6, 6, 0, 0] },
          emphasis: { itemStyle: { color: infoStrong } },
          data: data.map((d) => d.activitySeconds),
        },
        {
          name: "活动次数",
          type: "line",
          yAxisIndex: 1,
          smooth: true,
          symbolSize: 6,
          itemStyle: { color: countLine },
          lineStyle: { color: countLine, width: 2 },
          data: data.map((d) => d.activityCount),
        },
      ],
    };
    // 合并更新而非全量重建：数据变化时保留当前悬停状态，不打断 tooltip
    chart.setOption(option, false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dataKey, themeSetting]);

  return <div ref={containerRef} style={{ width: "100%", height }} />;
});
