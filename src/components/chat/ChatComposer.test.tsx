import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatComposer } from "./ChatComposer";

const baseProps = {
  value: "检查项目",
  busy: false,
  queuedCount: 0,
  queuedItems: [],
  onRemoveQueued: vi.fn(),
  sessionActive: true,
  attachments: [],
  selectedSkills: [],
  ollamaAvailable: false,
  ollamaSelectedModel: null,
  enhancedPrompt: null,
  useEnhanced: false,
  showFinal: false,
  finalText: null,
  enhancing: false,
  onChange: vi.fn(),
  onSend: vi.fn(),
  onStop: vi.fn(),
  onAddFiles: vi.fn(),
  onEnhance: vi.fn(),
  onUseEnhanced: vi.fn(),
  onToggleFinal: vi.fn(),
};

describe("ChatComposer", () => {
  it("sends on Enter", () => {
    const onSend = vi.fn();
    render(<ChatComposer {...baseProps} onSend={onSend} />);

    fireEvent.keyDown(screen.getByLabelText("发送给 Claude Code"), {
      key: "Enter",
    });

    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("inserts a newline on Ctrl+Enter instead of sending", () => {
    const onSend = vi.fn();
    const onChange = vi.fn();
    render(<ChatComposer {...baseProps} onSend={onSend} onChange={onChange} />);

    fireEvent.keyDown(screen.getByLabelText("发送给 Claude Code"), {
      key: "Enter",
      ctrlKey: true,
    });

    expect(onSend).not.toHaveBeenCalled();
    expect(onChange).toHaveBeenCalled();
  });

  it("keeps the input usable and queues on Enter while busy", () => {
    const onSend = vi.fn();
    render(
      <ChatComposer {...baseProps} busy queuedCount={2} onSend={onSend} />,
    );

    const input = screen.getByLabelText("发送给 Claude Code");
    expect(input).not.toBeDisabled();
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(screen.getByText("已排队 2 条")).toBeInTheDocument();
  });
  it("does not send while composing text", () => {
    const onSend = vi.fn();
    render(<ChatComposer {...baseProps} onSend={onSend} />);

    fireEvent.keyDown(screen.getByLabelText("发送给 Claude Code"), {
      key: "Enter",
      isComposing: true,
    });

    expect(onSend).not.toHaveBeenCalled();
  });
});
