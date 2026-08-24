import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { OllamaStatus } from "../../api/dto";
import { OnboardingDialog } from "./OnboardingDialog";

const ollama: OllamaStatus = {
  online: true,
  baseUrl: "http://localhost:11434",
  selectedModel: "qwen2.5:3b",
  models: [{ name: "qwen2.5:3b" }],
  autoSelected: false,
  message: "ready",
};

function makeProps(
  overrides: Partial<Parameters<typeof OnboardingDialog>[0]> = {},
) {
  return {
    open: true,
    claudeCliAvailable: true,
    claudeAuthenticated: true,
    gitAvailable: true,
    projectLabel: "C:\\work",
    modelReady: true,
    experienceMode: "guided" as const,
    ollama,
    busy: false,

    ollamaSaving: false,
    onExperienceModeChange: vi.fn(),
    onInstallClaude: vi.fn(),
    onOpenModelConfig: vi.fn(),
    onRecheckClaude: vi.fn(),
    onSelectProject: vi.fn(),
    onAddModel: vi.fn(),
    onSelectOllamaModel: vi.fn(),
    onRunDemo: vi.fn().mockResolvedValue({
      userId: "小明",
      fileName: "hello_小明.html",
      displayPath: "桌面/hello_小明.html",
      content: "<html></html>",
      createdAtMs: 1,
    }),
    onOpenDemoFile: vi.fn().mockResolvedValue(undefined),
    onClose: vi.fn(),
    ...overrides,
  };
}

describe("OnboardingDialog", () => {
  it("renders nothing when closed", () => {
    const props = makeProps({ open: false });
    const { container } = render(<OnboardingDialog {...props} />);
    expect(container.firstChild).toBeNull();
  });

  it("shows the demo only as the final onboarding step", async () => {
    const props = makeProps();
    render(<OnboardingDialog {...props} />);
    expect(
      screen.queryByRole("heading", { name: "动手体验 Agent 流程" }),
    ).not.toBeInTheDocument();
    for (let index = 0; index < 5; index += 1) {
      await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    }
    expect(
      screen.getByRole("heading", { name: "动手体验 Agent 流程" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "完成引导" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "进入演示" }));
    await userEvent.type(
      screen.getByRole("textbox", { name: "名字或用户 ID" }),
      "小明",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "下一步：查看演示计划" }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "下一步：创建桌面文件" }),
    );
    expect(props.onRunDemo).toHaveBeenCalledWith("小明");
    expect(screen.getByRole("button", { name: "完成引导" })).toBeEnabled();
  });

  it("offers Claude installation and recheck when Claude is missing", async () => {
    const props = makeProps({ claudeCliAvailable: false });
    render(<OnboardingDialog {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(
      screen.getByRole("button", { name: "一键准备国内环境" }),
    );
    expect(props.onInstallClaude).toHaveBeenCalledTimes(1);
    expect(
      screen.getByRole("button", { name: "重新检测" }),
    ).toBeInTheDocument();
  });

  it("lets the user choose a project directory", async () => {
    const props = makeProps({ projectLabel: null });
    render(<OnboardingDialog {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "选择项目目录" }));
    expect(props.onSelectProject).toHaveBeenCalledTimes(1);
  });

  it("lets the user add a model and choose the display experience", async () => {
    const props = makeProps({ modelReady: false });
    render(<OnboardingDialog {...props} />);
    await userEvent.click(screen.getByRole("button", { name: /^完整体验/ }));
    expect(props.onExperienceModeChange).toHaveBeenCalledWith("complete");
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "添加模型配置" }));
    expect(props.onAddModel).toHaveBeenCalledTimes(1);
  });

  it("allows skipping even when setup is incomplete", async () => {
    const props = makeProps({
      claudeCliAvailable: false,
      projectLabel: null,
      modelReady: false,
    });
    render(<OnboardingDialog {...props} />);
    expect(screen.getByRole("button", { name: "跳过全部" })).toBeEnabled();
    await userEvent.click(screen.getByRole("button", { name: "跳过全部" }));
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });

  it("can select or disable local prompt optimization", async () => {
    const props = makeProps();
    render(<OnboardingDialog {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "选择本地 Prompt 优化模型" }),
      "",
    );
    expect(props.onSelectOllamaModel).toHaveBeenCalledWith(null);
  });
});
