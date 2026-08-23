import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AttachmentList } from "./AttachmentList";

const attachments = ["a", "b"].map((handle) => ({
  handle,
  name: `${handle}.txt`,
  kind: "text" as const,
  mime: "text/plain",
  rawBytes: 1,
  extractedBytes: 1,
  sha256: handle,
  warnings: [],
}));

describe("AttachmentList", () => {
  it("opens an explicit preview action for each attachment", async () => {
    const onPreview = vi.fn();
    render(
      <AttachmentList
        attachments={attachments}
        onRemove={vi.fn()}
        onMove={vi.fn()}
        onPreview={onPreview}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "查看 a.txt" }));
    expect(onPreview).toHaveBeenCalledWith("a");
  });
  it("provides keyboard-operable reorder controls", async () => {
    const onMove = vi.fn();
    render(
      <AttachmentList
        attachments={attachments}
        onRemove={vi.fn()}
        onMove={onMove}
        onPreview={vi.fn()}
      />,
    );
    const down = screen.getByRole("button", { name: "下移 a.txt" });
    down.focus();
    await userEvent.keyboard("{Enter}");
    expect(onMove).toHaveBeenCalledWith("a", 1);
    expect(screen.getByRole("button", { name: "上移 a.txt" })).toBeDisabled();
  });
});
