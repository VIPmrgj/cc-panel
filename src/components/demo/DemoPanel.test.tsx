import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DemoPanel } from "./DemoPanel";

describe("DemoPanel", () => {
  it("clearly separates the no-API sandbox and validates the user id", async () => {
    const onRunSandbox = vi.fn().mockResolvedValue({
      userId: "小明",
      fileName: "hello_小明.html",
      displayPath: "桌面/hello_小明.html",
      content: "<html><body>hello</body></html>",
      createdAtMs: 1,
    });
    const onCompleted = vi.fn();
    render(<DemoPanel onRunSandbox={onRunSandbox} onCompleted={onCompleted} />);

    expect(
      screen.getByRole("heading", { name: "动手体验 Agent 流程" }),
    ).toBeInTheDocument();
    expect(screen.getByText("这是演示模式，不是 AI 对话")).toBeInTheDocument();
    expect(screen.getByText(/不调用模型、不需要 API Key/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "开始第 1 步" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "请先输入名字或用户 ID。",
    );
    expect(onRunSandbox).not.toHaveBeenCalled();

    await userEvent.type(
      screen.getByRole("textbox", { name: "先输入你的名字或用户 ID" }),
      "小明",
    );
    await userEvent.click(screen.getByRole("button", { name: "开始第 1 步" }));
    expect(
      screen.getByText("第 2 步已展示。点击下一步后，才会在桌面创建文件。"),
    ).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "下一步：在桌面创建文件" }),
    );
    await waitFor(() => expect(onRunSandbox).toHaveBeenCalledWith("小明"));
    expect(onCompleted).toHaveBeenCalledTimes(1);
    expect(screen.getByText("桌面/hello_小明.html")).toBeInTheDocument();
  });
});
