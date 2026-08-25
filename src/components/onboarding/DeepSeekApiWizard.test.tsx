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
      screen.getByRole("heading", {
        name: "为什么需要 API Key？——就像给汽车加油",
      }),
    ).toBeInTheDocument();
    expect(screen.getByText(/本软件就像一辆好用的汽车/)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "我已有其他 API Key" }),
    ).toBeInTheDocument();
  });

  it("shows the three-step guide and saves a named default DeepSeek profile", async () => {
    const user = userEvent.setup();
    const props = makeProps();
    render(<DeepSeekApiWizard {...props} />);
    await user.click(screen.getByRole("button", { name: "开始配置 DeepSeek" }));

    expect(
      screen.getByRole("heading", { name: "三步拿到“加油卡”" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: /打开 DeepSeek 平台/ }),
    ).toHaveAttribute("href", "https://platform.deepseek.com/usage");
    expect(
      screen.getByRole("img", { name: "DeepSeek 登录界面示意图" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "复制 DeepSeek API Key" }),
    ).toBeInTheDocument();

    const save = screen.getByRole("button", { name: "保存并配置默认模型" });
    expect(save).toBeDisabled();
    await user.type(
      screen.getByLabelText("在这里粘贴 DeepSeek API Key"),
      "sk-test-key",
    );
    expect(save).toBeEnabled();
    await user.click(save);

    expect(props.onSave).toHaveBeenCalledWith(
      {
        providerName: "默认模型",
        note: "DeepSeek",
        websiteUrl: "https://platform.deepseek.com/",
        baseUrl: "https://api.deepseek.com/anthropic",
        modelId: "deepseek-v4-pro",
        selected: true,
      },
      "sk-test-key",
    );
  });

  it("rejects a key that is not copied completely", async () => {
    const user = userEvent.setup();
    render(<DeepSeekApiWizard {...makeProps()} />);
    await user.click(screen.getByRole("button", { name: "开始配置 DeepSeek" }));
    await user.type(
      screen.getByLabelText("在这里粘贴 DeepSeek API Key"),
      "not-a-key",
    );
    await user.click(
      screen.getByRole("button", { name: "保存并配置默认模型" }),
    );
    expect(
      screen.getByText("API Key 通常以 sk- 开头，请检查是否复制完整。"),
    ).toBeInTheDocument();
  });

  it("opens a tutorial image in the accessible preview", async () => {
    const user = userEvent.setup();
    render(<DeepSeekApiWizard {...makeProps()} />);
    await user.click(screen.getByRole("button", { name: "开始配置 DeepSeek" }));
    await user.click(
      screen.getByRole("button", { name: "放大查看：登录界面" }),
    );
    expect(screen.getByRole("dialog", { name: "放大查看：登录界面" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "关闭图片预览" })).toBeInTheDocument();
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
