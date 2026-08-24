import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { InstallProgressView, SetupCenter } from "./SetupCenter";

function renderSetup(
  overrides: Partial<React.ComponentProps<typeof SetupCenter>> = {},
) {
  const props: React.ComponentProps<typeof SetupCenter> = {
    claudeInstalled: false,
    claudeAuthenticated: false,
    gitAvailable: false,
    projectReady: false,
    modelReady: false,
    onInstall: vi.fn(),
    onOpenModels: vi.fn(),
    onRecheck: vi.fn(),
    onOpenSetup: vi.fn(),
    ...overrides,
  };
  return { ...render(<SetupCenter {...props} />), props };
}

describe("SetupCenter", () => {
  it("offers one-click installation and keeps the prompt", async () => {
    const user = userEvent.setup();
    const { props } = renderSetup();

    expect(
      screen.getByRole("button", { name: "一键准备国内环境" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/输入内容会保留/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "一键准备国内环境" }));
    expect(props.onInstall).toHaveBeenCalledTimes(1);
  });

  it("moves from installation to login without reopening onboarding", () => {
    renderSetup({ claudeInstalled: true, gitAvailable: true });

    expect(
      screen.getByRole("button", { name: "打开模型配置" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "一键准备国内环境" }),
    ).toBeNull();
  });

  it("shows the current installation step and busy state", () => {
    render(
      <InstallProgressView
        progress={{
          step: 2,
          totalSteps: 5,
          phase: "git",
          status: "running",
          message: null,
        }}
      />,
    );

    const status = screen.getByRole("status");
    expect(status).toHaveAttribute("aria-busy", "true");
    expect(
      within(status).getByText("Git", { selector: "strong" }),
    ).toBeInTheDocument();
    expect(within(status).getByText(/正在处理\s*Git/)).toBeInTheDocument();
  });
  it("does not render when the real agent is ready", () => {
    const { container } = renderSetup({
      claudeInstalled: true,
      claudeAuthenticated: true,
      gitAvailable: true,
      projectReady: true,
      modelReady: true,
    });

    expect(container.firstChild).toBeNull();
  });
});
