import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";
import "./i18n";

describe("App shell (Phase 0)", () => {
  it("renders the Persian RTL shell with sidebar navigation", () => {
    render(<App />);
    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("ژورنال طهان");
    expect(screen.getByTestId("nav-dashboard")).toHaveTextContent("داشبورد");
    expect(screen.getByTestId("nav-settings")).toHaveTextContent("تنظیمات");
    expect(document.querySelector("div[dir='rtl']")).not.toBeNull();
  });
});
