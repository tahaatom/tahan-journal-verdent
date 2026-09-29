//! پوشه ECharts — مقداردهی امن با رندرکننده SVG و ظرف RTL.

import { useEffect, useRef } from "react";
import * as echarts from "echarts/core";
import { BarChart, HeatmapChart, LineChart } from "echarts/charts";
import {
  GridComponent,
  TooltipComponent,
  VisualMapComponent,
} from "echarts/components";
import { SVGRenderer } from "echarts/renderers";
import type { EChartsCoreOption } from "echarts/core";

echarts.use([
  LineChart,
  BarChart,
  HeatmapChart,
  GridComponent,
  TooltipComponent,
  VisualMapComponent,
  SVGRenderer,
]);

interface EChartProps {
  option: EChartsCoreOption;
  testid: string;
  height?: number;
}

/** نمودار با ظرف راست‌به‌چپ — شکست مقداردهی هرگز رندر صفحه را نمی‌شکند. */
export function EChart({ option, testid, height = 280 }: EChartProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let chart: echarts.ECharts | null = null;
    try {
      chart = echarts.init(el, undefined, {
        renderer: "svg",
        width: el.clientWidth || 600,
        height: el.clientHeight || height,
      });
      chart.setOption(option);
    } catch {
      // در محیط آزمون/بدون اندازه‌گیری نمودار نادیده گرفته می‌شود
    }
    return () => {
      chart?.dispose();
    };
  }, [option, height]);

  return (
    <div
      ref={ref}
      data-testid={testid}
      dir="rtl"
      style={{ width: "100%", height }}
    />
  );
}
