import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ModelProfile } from "../../api/dto";
import { DeepSeekApiWizard } from "./DeepSeekApiWizard";

const savedProfile: ModelProfile = {
  id: "deepseek-1",
  providerName: "DeepSeek",
  note: null,
  websiteUrl: "https://platform.deepseek.com/",
  baseUrl: "https://api.deepseek.com/anthropic",
  modelId: "deepseek-v4-pro",
  selected: true,
  hasApiKey: true,
};

function makeProps(
  overrides: Partial<Parameters<typeof DeepSeekApiWizard>[0]> = {},
) {
  return {
    open: true,
    saving: false,
    testing: false,
    savedProfile: null,
    testResult: null,
    onSave: vi.fn(),
    onTest: vi.fn(),
    onOpenAdvanced: vi.fn(),
    onClose: vi.fn(),
    ...overrides,
  };
}

describe("DeepSeekApiWizard", () => {
  it("explains the API Key flow before asking for credentials", () => {
    render(<DeepSeekApiWizard {...makeProps()} />);
    expect(
      screen.getByRole("heading", { name: "先用一个例子学会接入模型" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/API Key 可以理解成模型服务的密码/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "我已有其他 API Key" }),
    ).toBeInTheDocument();
  });

  it("requires confirmation before advancing from the provider page", async () => {
    const user = userEvent.setup();
    render(<DeepSeekApiWizard {...makeProps()} />);
    await user.click(screen.getByRole("button", { name: "开始示例" }));

    expect(
      screen.getByRole("link", { name: /打开 DeepSeek 开放平台/ }),
    ).toHaveAttribute("href", "https://platform.deepseek.com/");
    const next = screen.getByRole("button", { name: "下一步" });
    expect(next).toBeDisabled();
    await user.click(
      screen.getByRole("checkbox", { name: "我已经创建并复制了 API Key" }),
    );
    expect(next).toBeEnabled();
    await user.click(next);
    expect(
      screen.getByRole("heading", { name: "保存密钥并使用推荐配置" }),
    ).toBeInTheDocument();
  });

  it("saves only the preset profile and lets the native dialog collect the key", async () => {
    const user = userEvent.setup();
    const props = makeProps();
    render(<DeepSeekApiWizard {...props} />);
    await user.click(screen.getByRole("button", { name: "开始示例" }));
    await user.click(
      screen.getByRole("checkbox", { name: "我已经创建并复制了 API Key" }),
    );
    await user.click(screen.getByRole("button", { name: "下一步" }));
    await user.click(
      screen.getByRole("button", { name: "保存并输入 API Key" }),
    );

    expect(props.onSave).toHaveBeenCalledWith({
      providerName: "DeepSeek",
      note: "通过 DeepSeek 新手引导配置",
      websiteUrl: "https://platform.deepseek.com/",
      baseUrl: "https://api.deepseek.com/anthropic",
      modelId: "deepseek-v4-pro",
      selected: true,
    });
  });

  it("does not test until the user acknowledges possible cost", async () => {
    const user = userEvent.setup();
    const props = makeProps({ savedProfile });
    render(<DeepSeekApiWizard {...props} />);
    const testButton = screen.getByRole("button", { name: "测试连接" });
    expect(testButton).toBeDisabled();
    await user.click(
      screen.getByRole("checkbox", { name: "我知道测试可能产生少量 API 费用" }),
    );
    expect(testButton).toBeEnabled();
    await user.click(testButton);
    expect(props.onTest).toHaveBeenCalledWith("deepseek-1");
  });

  it("shows a verifiable success result without faking model output", () => {
    render(
      <DeepSeekApiWizard
        {...makeProps({
          savedProfile,
          testResult: {
            ok: true,
            code: "MODEL_TEST_OK",
            message: "DeepSeek 已连接，模型可以使用。",
            providerName: "DeepSeek",
            modelId: "deepseek-v4-pro",
          },
        })}
      />,
    );
    expect(
      screen.getByRole("heading", { name: "连接成功" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/现在可以返回新手引导/)).toBeInTheDocument();
  });
});
