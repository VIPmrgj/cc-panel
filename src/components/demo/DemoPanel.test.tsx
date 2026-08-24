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
    expect(screen.getByText(/不需要 API Key/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "进入演示" }));
    await userEvent.click(
      screen.getByRole("button", { name: "下一步：查看演示计划" }),
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "请先输入名字或用户 ID。",
    );
    expect(onRunSandbox).not.toHaveBeenCalled();

    await userEvent.type(
      screen.getByRole("textbox", { name: "名字或用户 ID" }),
      "小明",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "下一步：查看演示计划" }),
    );
    expect(screen.getByText("Agent 将按这 3 步完成任务")).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "下一步：创建桌面文件" }),
    );
    await waitFor(() => expect(onRunSandbox).toHaveBeenCalledWith("小明"));
    expect(onCompleted).toHaveBeenCalledTimes(1);
    expect(screen.getByText("hello_小明.html")).toBeInTheDocument();
    expect(screen.getByText("恭喜你，你完成了演示！")).toBeInTheDocument();
  });
});
