import { renderHook } from "@testing-library/react";
import { act } from "react";
import { describe, expect, it, vi } from "vitest";
import { useCopyShortcut } from "./useCopyShortcut";

describe("useCopyShortcut", () => {
  it("runs the copy path once for Ctrl+Enter", () => {
    const onCopy = vi.fn();
    renderHook(() => useCopyShortcut({ onCopy, disabled: false }));

    const event = new KeyboardEvent("keydown", {
      key: "Enter",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    act(() => window.dispatchEvent(event));

    expect(onCopy).toHaveBeenCalledTimes(1);
    expect(event.defaultPrevented).toBe(true);
  });

  it("ignores disabled, IME, and non-Ctrl shortcuts", () => {
    const onCopy = vi.fn();
    const { rerender } = renderHook(
      ({ disabled }) => useCopyShortcut({ onCopy, disabled }),
      { initialProps: { disabled: false } },
    );

    act(() => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
      );
      const composing = new KeyboardEvent("keydown", {
        key: "Enter",
        ctrlKey: true,
        bubbles: true,
      });
      Object.defineProperty(composing, "isComposing", { value: true });
      window.dispatchEvent(composing);
    });
    rerender({ disabled: true });
    act(() => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          ctrlKey: true,
          bubbles: true,
        }),
      );
    });

    expect(onCopy).not.toHaveBeenCalled();
  });
});
