import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AddModelDialog } from "./AddModelDialog";

describe("AddModelDialog", () => {
  it("prefills the DeepSeek Anthropic-compatible endpoint and model", async () => {
    render(<AddModelDialog onClose={vi.fn()} onSave={vi.fn()} />);
    await userEvent.selectOptions(screen.getByLabelText(/提供商/), "DeepSeek");
    expect(screen.getByLabelText(/API 地址/)).toHaveValue(
      "https://api.deepseek.com/anthropic",
    );
    expect(screen.getByLabelText(/模型 ID/)).toHaveValue("deepseek-v4-pro");
    expect(
      screen.getByText(/api\.deepseek\.com\/anthropic/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/勿填 platform\.deepseek\.com/),
    ).toBeInTheDocument();
  });

  it("prefills Claude Official defaults when Claude Official is selected", async () => {
    render(<AddModelDialog onClose={vi.fn()} onSave={vi.fn()} />);
    await userEvent.selectOptions(
      screen.getByLabelText(/提供商/),
      "Claude Official",
    );
    expect(screen.getByLabelText(/API 地址/)).toHaveValue(
      "https://api.anthropic.com",
    );
    expect(screen.getByLabelText(/模型 ID/)).toHaveValue("claude-opus-5");
  });

  it("keeps entered values when switching to 自定义", async () => {
    render(<AddModelDialog onClose={vi.fn()} onSave={vi.fn()} />);
    const url = screen.getByLabelText(/API 地址/);
    await userEvent.clear(url);
    await userEvent.type(url, "https://custom.example/anthropic");
    await userEvent.selectOptions(screen.getByLabelText(/提供商/), "自定义");
    expect(screen.getByLabelText(/API 地址/)).toHaveValue(
      "https://custom.example/anthropic",
    );
  });

  it("auto-converts the bare DeepSeek API origin to the anthropic endpoint on save", async () => {
    const onSave = vi.fn();
    render(<AddModelDialog onClose={vi.fn()} onSave={onSave} />);
    await userEvent.selectOptions(screen.getByLabelText(/提供商/), "DeepSeek");
    const url = screen.getByLabelText(/API 地址/);
    await userEvent.clear(url);
    await userEvent.type(url, "https://api.deepseek.com");
    await userEvent.click(screen.getByRole("button", { name: "保存配置" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        baseUrl: "https://api.deepseek.com/anthropic",
      }),
      true,
    );
    expect(screen.getByLabelText(/API 地址/)).toHaveValue(
      "https://api.deepseek.com/anthropic",
    );
  });

  it("selects the first saved model when the app has no current model", async () => {
    const onSave = vi.fn();
    render(
      <AddModelDialog selectByDefault onClose={vi.fn()} onSave={onSave} />,
    );
    await userEvent.click(screen.getByRole("button", { name: "保存配置" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ selected: true }),
      true,
    );
  });

  it("does not switch away from an existing model when adding another", async () => {
    const onSave = vi.fn();
    render(<AddModelDialog onClose={vi.fn()} onSave={onSave} />);
    await userEvent.click(screen.getByRole("button", { name: "保存配置" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ selected: false }),
      true,
    );
  });

  it("prefills Zhipu GLM (智谱) endpoint and model", async () => {
    render(<AddModelDialog onClose={vi.fn()} onSave={vi.fn()} />);
    await userEvent.selectOptions(screen.getByLabelText(/提供商/), "Zhipu GLM");
    expect(screen.getByLabelText(/API 地址/)).toHaveValue(
      "https://open.bigmodel.cn/api/anthropic",
    );
    expect(screen.getByLabelText(/模型 ID/)).toHaveValue("glm-5.1");
  });

  it("prefills Kimi (Moonshot) endpoint and model", async () => {
    render(<AddModelDialog onClose={vi.fn()} onSave={vi.fn()} />);
    await userEvent.selectOptions(screen.getByLabelText(/提供商/), "Kimi");
    expect(screen.getByLabelText(/API 地址/)).toHaveValue(
      "https://api.moonshot.cn/anthropic",
    );
    expect(screen.getByLabelText(/模型 ID/)).toHaveValue("kimi-k2.7-code");
  });

  it("prefills 胜算云 (Shengsuanyun) endpoint", async () => {
    render(<AddModelDialog onClose={vi.fn()} onSave={vi.fn()} />);
    await userEvent.selectOptions(
      screen.getByLabelText(/提供商/),
      "Shengsuanyun",
    );
    expect(screen.getByLabelText(/API 地址/)).toHaveValue(
      "https://router.shengsuanyun.com/api",
    );
  });

  it("auto-converts the GLM OpenAI path to the anthropic endpoint on save", async () => {
    const onSave = vi.fn();
    render(<AddModelDialog onClose={vi.fn()} onSave={onSave} />);
    await userEvent.selectOptions(
      screen.getByLabelText(/提供商/),
      "Zhipu GLM en",
    );
    const url = screen.getByLabelText(/API 地址/);
    await userEvent.clear(url);
    await userEvent.type(url, "https://api.z.ai/api/paas/v4");
    await userEvent.click(screen.getByRole("button", { name: "保存配置" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        baseUrl: "https://api.z.ai/api/anthropic",
      }),
      true,
    );
  });

  it("keeps the /anthropic endpoint unchanged", async () => {
    const onSave = vi.fn();
    render(<AddModelDialog onClose={vi.fn()} onSave={onSave} />);
    await userEvent.selectOptions(screen.getByLabelText(/提供商/), "DeepSeek");
    await userEvent.click(screen.getByRole("button", { name: "保存配置" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        baseUrl: "https://api.deepseek.com/anthropic",
      }),
      true,
    );
  });

  it("keeps existing profile values while editing (no prefill override)", async () => {
    render(
      <AddModelDialog
        profile={{
          id: "p1",
          providerName: "DeepSeek",
          note: null,
          websiteUrl: null,
          baseUrl: "https://api.deepseek.com/anthropic",
          modelId: "deepseek-v4-flash",
          selected: true,
          hasApiKey: true,
        }}
        onClose={vi.fn()}
        onSave={vi.fn()}
      />,
    );
    expect(screen.getByLabelText(/API 地址/)).toHaveValue(
      "https://api.deepseek.com/anthropic",
    );
    expect(screen.getByLabelText(/模型 ID/)).toHaveValue("deepseek-v4-flash");
  });
});
