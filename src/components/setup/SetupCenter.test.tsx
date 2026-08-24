import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SetupCenter } from "./SetupCenter";

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
    onLogin: vi.fn(),
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
      screen.getByRole("button", { name: "一键安装 Claude Code" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/当前输入内容会保留/)).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "一键安装 Claude Code" }),
    );
    expect(props.onInstall).toHaveBeenCalledTimes(1);
  });

  it("moves from installation to login without reopening onboarding", () => {
    renderSetup({ claudeInstalled: true });

    expect(
      screen.getByRole("button", { name: "开始登录" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "一键安装 Claude Code" }),
    ).toBeNull();
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
