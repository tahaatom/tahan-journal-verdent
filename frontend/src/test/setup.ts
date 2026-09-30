import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// jsdom پیاده‌سازی Blob.arrayBuffer ندارد (WebView2 دارد). شبیه‌سازی برای
// آزمون‌های ایمپورت که بایت‌های فایل را می‌خوانند.
if (typeof Blob !== "undefined" && typeof Blob.prototype.arrayBuffer !== "function") {
  Blob.prototype.arrayBuffer = function arrayBuffer(this: Blob): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as ArrayBuffer);
      reader.onerror = () => reject(reader.error);
      reader.readAsArrayBuffer(this);
    });
  };
}

// jsdom پیاده‌سازی canvas ندارد؛ zrender (موتور ECharts) برای اندازه‌گیری
// متن به آن نیاز دارد. کاذب ساده، اجرای نمودار در آزمون را بی‌صدا ممکن می‌کند.
if (typeof HTMLCanvasElement !== "undefined") {
  HTMLCanvasElement.prototype.getContext = vi.fn(() => ({
    measureText: () => ({ width: 0 }),
    font: "",
    save: () => undefined,
    restore: () => undefined,
    translate: () => undefined,
    clearRect: () => undefined,
    rect: () => undefined,
    clip: () => undefined,
    beginPath: () => undefined,
    closePath: () => undefined,
    moveTo: () => undefined,
    lineTo: () => undefined,
    stroke: () => undefined,
    fill: () => undefined,
    fillText: () => undefined,
    strokeText: () => undefined,
    arc: () => undefined,
    setLineDash: () => undefined,
    createLinearGradient: () => ({ addColorStop: () => undefined }),
    createRadialGradient: () => ({ addColorStop: () => undefined }),
  })) as unknown as HTMLCanvasElement["getContext"];
}
