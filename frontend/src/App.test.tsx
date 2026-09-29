import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import App from "./App";
import { applyTheme, readStoredTheme } from "./theme";
import {
  renderOutcome,
  schemaViolation,
  type DeclarativeExtension,
} from "./extensions";
import "./i18n";

const widget = (over: Partial<DeclarativeExtension>): DeclarativeExtension => ({
  plugin_id: "plg-test",
  kind: "dashboard_widget",
  id: "w1",
  title: "ویجت آزمون",
  display_order: 0,
  schema: { type: "stat", title: "سود امروز" },
  ...over,
});

beforeEach(() => {
  localStorage.clear();
  document.documentElement.classList.remove("dark");
});

describe("App shell", () => {
  it("renders the Persian RTL shell with sidebar navigation", () => {
    render(<App />);
    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("ژورنال طهان");
    expect(screen.getByTestId("nav-dashboard")).toHaveTextContent("داشبورد");
    expect(screen.getByTestId("nav-settings")).toHaveTextContent("تنظیمات");
    expect(document.querySelector("div[dir='rtl']")).not.toBeNull();
  });

  it("navigates between pages with Persian titles and forms/empty states", async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(screen.getByTestId("page-title")).toHaveTextContent("داشبورد");
    await user.click(screen.getByTestId("nav-journal"));
    expect(screen.getByTestId("page-title")).toHaveTextContent("ژورنال معاملات");
    // فاز ۱.۱۲: صفحه ژورنال تب فهرست/ثبت دارد؛ فرم ثبت در تب ثبت معامله است
    expect(await screen.findByTestId("page-journal")).toBeInTheDocument();
    expect(screen.getByTestId("journal-tab-list")).toBeInTheDocument();
    await user.click(screen.getByTestId("journal-tab-register"));
    expect(screen.getByTestId("mode-fast")).toHaveTextContent("سریع");
    expect(screen.getByTestId("mode-full")).toHaveTextContent("کامل");
    await user.click(screen.getByTestId("nav-plugins"));
    expect(screen.getByTestId("page-title")).toHaveTextContent("سلامت پلاگین‌ها");
    await user.click(screen.getByTestId("nav-backup"));
    expect(screen.getByTestId("page-title")).toHaveTextContent("پشتیبان‌گیری و بازیابی");
  });

  it("marks the active navigation item", async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(screen.getByTestId("nav-dashboard")).toHaveAttribute("data-active", "true");
    await user.click(screen.getByTestId("nav-settings"));
    expect(screen.getByTestId("nav-dashboard")).toHaveAttribute("data-active", "false");
    expect(screen.getByTestId("nav-settings")).toHaveAttribute("data-active", "true");
  });
});

describe("Theme switching", () => {
  it("toggles dark class on document root and persists", async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    await user.click(screen.getByTestId("theme-toggle"));
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(readStoredTheme()).toBe("dark");
    await user.click(screen.getByTestId("theme-toggle"));
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(readStoredTheme()).toBe("light");
  });

  it("applyTheme writes the class directly", () => {
    applyTheme("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    applyTheme("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("theme toggle also exists on settings page", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByTestId("nav-settings"));
    expect(screen.getByTestId("settings-theme-toggle")).toBeInTheDocument();
  });
});

describe("Command palette", () => {
  it("opens via Ctrl+K and navigates with Persian labels", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.keyboard("{Control>}k");
    expect(screen.getByTestId("command-palette")).toBeInTheDocument();
    expect(screen.getByTestId("palette-input")).toHaveAttribute("dir", "rtl");
    await user.click(screen.getByTestId("palette-cmd-nav-trades"));
    expect(screen.queryByTestId("command-palette")).not.toBeInTheDocument();
    expect(screen.getByTestId("page-title")).toHaveTextContent("لیست معاملات");
  });

  it("closes on Escape and shows empty result for unknown query", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByTestId("palette-open"));
    await user.type(screen.getByTestId("palette-input"), "ناکسستان");
    expect(screen.getByTestId("palette-empty")).toHaveTextContent("فرمانی یافت نشد");
    await user.keyboard("{Escape}");
    expect(screen.queryByTestId("command-palette")).not.toBeInTheDocument();
  });
});

// رندرگر اعلانی — ایمپورت استاتیک برای پوشش کامل
import { DeclarativeWidget } from "./components/DeclarativeWidget";
import { ErrorState } from "./components/States";

describe("Declarative widget host", () => {
  it("renders allowed widget types only via fixed components", () => {
    const { container } = render(
      <DeclarativeWidget point="dashboard_widget" ext={widget({})} />,
    );
    expect(screen.getByTestId("ext-w1")).toBeInTheDocument();
    expect(screen.getByTestId("ext-value-w1")).toHaveTextContent("—");
    // divider نوع اعلانی مجاز
    render(
      <DeclarativeWidget
        point="dashboard_widget"
        ext={widget({ id: "d1", schema: { type: "divider" } })}
      />,
      { container },
    );
    expect(screen.getByTestId("ext-d1")).toBeInTheDocument();
  });

  it("renders a clear denial instead of executing unknown types", () => {
    render(
      <DeclarativeWidget
        point="dashboard_widget"
        ext={widget({ id: "evil", schema: { type: "react_component" } })}
      />,
    );
    expect(screen.getByTestId("ext-denied-evil")).toHaveTextContent(
      "تزریق کامپوننت در نسخه ۱ ممنوع است"
    );
  });

  it("denies forbidden injection keys at any depth", () => {
    expect(
      schemaViolation("dashboard_widget", { type: "table", component: "X" })
    ).toBe("injection_denied");
    expect(
      schemaViolation("dashboard_widget", {
        type: "table",
        rows: [{ handler: "fetch()" }],
      })
    ).toBe("injection_denied");
    // هر دو سبک نام‌گذاری — هم‌ارز کرنل
    expect(
      schemaViolation("dashboard_widget", { type: "text", onClick: "x" })
    ).toBe("injection_denied");
    expect(
      schemaViolation("dashboard_widget", { type: "list", items: [{ innerHTML: "x" }] })
    ).toBe("injection_denied");
    expect(schemaViolation("dashboard_widget", { type: "table" })).toBeNull();
  });

  it("type is scoped per extension point like the kernel", () => {
    // stat فقط برای dashboard_widget
    expect(renderOutcome("dashboard_widget", widget({})).kind).toBe("stat");
    expect(renderOutcome("report_page", widget({})).kind).toBe("denied");
    // command فقط برای command_menu
    expect(
      renderOutcome("command_menu", widget({ schema: { type: "command" } })).kind
    ).toBe("command");
    expect(
      renderOutcome("form_field", widget({ schema: { type: "command" } })).kind
    ).toBe("denied");
  });
});

describe("Persian i18n coverage", () => {
  it("exposes Persian labels for every navigation item", () => {
    render(<App />);
    for (const [testid, label] of [
      ["nav-dashboard", "داشبورد"],
      ["nav-journal", "ژورنال"],
      ["nav-trades", "لیست معاملات"],
      ["nav-fields", "مدیریت فیلدها"],
      ["nav-plugins", "سلامت پلاگین‌ها"],
      ["nav-backup", "بکاپ"],
      ["nav-settings", "تنظیمات"],
    ] as const) {
      expect(screen.getByTestId(testid)).toHaveTextContent(label);
    }
  });

  it("shows Persian error state strings", async () => {
    render(<ErrorState onRetry={() => undefined} />);
    expect(screen.getByRole("alert")).toHaveTextContent("خطایی رخ داد");
    expect(screen.getByRole("button")).toHaveTextContent("تلاش دوباره");
    await waitFor(() =>
      expect(document.querySelector("[data-testid='error-state']")).not.toBeNull(),
    );
  });
});
