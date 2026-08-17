import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Drawer } from "./Drawer";

describe("Drawer", () => {
  it("traps focus, closes on Escape, and restores focus", async () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();
    const onClose = vi.fn();
    const { rerender } = render(
      <Drawer open title="测试抽屉" onClose={onClose}>
        <button>第一项</button>
        <button>最后一项</button>
      </Drawer>,
    );
    const panel = screen.getByRole("dialog", { name: "测试抽屉" });
    const close = screen.getByRole("button", { name: "关闭测试抽屉" });
    const first = screen.getByRole("button", { name: "第一项" });
    const last = screen.getByRole("button", { name: "最后一项" });

    expect(panel).toHaveFocus();
    await userEvent.keyboard("{Tab}");
    expect(close).toHaveFocus();
    last.focus();
    await userEvent.keyboard("{Tab}");
    expect(close).toHaveFocus();
    close.focus();
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    expect(last).toHaveFocus();
    first.focus();
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);

    rerender(
      <Drawer open={false} title="测试抽屉" onClose={onClose}>
        内容
      </Drawer>,
    );
    expect(trigger).toHaveFocus();
    trigger.remove();
  });
});
