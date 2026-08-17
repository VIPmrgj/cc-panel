import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModelControl } from "./ModelControl";

const model = {
  desiredUserModel: "opus",
  settingsRevision: "revision",
  candidates: [
    {
      source: "user-env" as const,
      label: "env.ANTHROPIC_MODEL",
      value: "custom-provider-model",
      enforced: false,
    },
  ],
  activeSessionObservable: false as const,
  warnings: [],
};

describe("ModelControl", () => {
  it("distinguishes desired value from detected candidates", () => {
    render(
      <ModelControl
        model={model}
        saving={false}
        onSave={vi.fn()}
        onClear={vi.fn()}
      />,
    );
    expect(screen.getByLabelText("期望的用户默认模型")).toHaveValue("opus");
    expect(screen.getByText("检测到的覆盖候选")).toBeInTheDocument();
    expect(screen.getByText("custom-provider-model")).toBeInTheDocument();
    expect(screen.getByText(/实际模型无法观察/)).toBeInTheDocument();
  });

  it("preserves a custom model ID", async () => {
    const onSave = vi.fn();
    render(
      <ModelControl
        model={model}
        saving={false}
        onSave={onSave}
        onClear={vi.fn()}
      />,
    );
    const input = screen.getByLabelText("期望的用户默认模型");
    await userEvent.clear(input);
    await userEvent.type(input, "deepseek-v4-pro[[1m]");
    await userEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(onSave).toHaveBeenCalledWith("deepseek-v4-pro[1m]");
  });
});
