import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DemoPanel } from "./DemoPanel";

describe("DemoPanel", () => {
  it("clearly separates the no-API sandbox and validates the user id", async () => {
    const onRunSandbox = vi.fn();
    render(
      <DemoPanel
        onRunSandbox={onRunSandbox}
        onExit={vi.fn()}
        onEnterRealAgent={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("heading", { name: "演示模式" }),
    ).toBeInTheDocument();
    expect(screen.getByText("这是演示模式，不是 AI 对话")).toBeInTheDocument();
    expect(screen.getByText(/不调用模型、不需要 API Key/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "开始沙盒演示" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "请先输入名字或用户 ID。",
    );
    expect(onRunSandbox).not.toHaveBeenCalled();
  });
});
