import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PendingSensitiveAttachment } from "../../api/dto";
import { SensitiveImportDialog } from "./SensitiveImportDialog";

const attachment: PendingSensitiveAttachment = {
  confirmationToken: "token",
  name: ".env",
  reason: "文件名可能包含密钥。",
  rawBytes: 12,
};

describe("SensitiveImportDialog", () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div class="app-frame">
        <main><button id="trigger">导入</button></main>
        <div id="portal"></div>
      </div>
    `;
    document.querySelector<HTMLButtonElement>("#trigger")?.focus();
  });

  it("focuses Cancel, traps Tab, applies inert, and restores focus", async () => {
    const trigger = document.querySelector<HTMLButtonElement>("#trigger")!;
    const portal = document.querySelector<HTMLElement>("#portal")!;
    const { unmount } = render(
      <SensitiveImportDialog
        attachment={attachment}
        busy={false}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
      { container: portal },
    );
    const cancel = screen.getByRole("button", { name: "取消" });
    const confirm = screen.getByRole("button", { name: "仍然导入" });

    expect(cancel).toHaveFocus();
    expect(document.querySelector("main")).toHaveProperty("inert", true);
    confirm.focus();
    await userEvent.keyboard("{Tab}");
    expect(cancel).toHaveFocus();
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    expect(confirm).toHaveFocus();

    unmount();
    expect(document.querySelector("main")).toHaveProperty("inert", false);
    expect(trigger).toHaveFocus();
  });

  it("cancels with Escape only when it is not busy", async () => {
    const onCancel = vi.fn();
    const portal = document.querySelector<HTMLElement>("#portal")!;
    const { rerender } = render(
      <SensitiveImportDialog
        attachment={attachment}
        busy={false}
        onCancel={onCancel}
        onConfirm={vi.fn()}
      />,
      { container: portal },
    );

    await userEvent.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
    rerender(
      <SensitiveImportDialog
        attachment={attachment}
        busy
        onCancel={onCancel}
        onConfirm={vi.fn()}
      />,
    );
    await userEvent.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
