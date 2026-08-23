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
    projectLabel: "C:\\work",
    modelReady: true,
    experienceMode: "guided" as const,
    ollama,
    busy: false,
    exampleBusy: false,
    ollamaSaving: false,
    onExperienceModeChange: vi.fn(),
    onCopyInstallCommand: vi.fn(),
    onRecheckClaude: vi.fn(),
    onSelectProject: vi.fn(),
    onAddModel: vi.fn(),
    onSelectOllamaModel: vi.fn(),
    onRunExample: vi.fn(),
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

  it("offers Claude installation and recheck when Claude is missing", async () => {
    const props = makeProps({ claudeCliAvailable: false });
    render(<OnboardingDialog {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await userEvent.click(screen.getByRole("button", { name: "复制安装命令" }));
    expect(props.onCopyInstallCommand).toHaveBeenCalledTimes(1);
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
