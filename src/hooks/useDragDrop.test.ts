import { renderHook } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useDragDrop } from "./useDragDrop";

const webviewMocks = vi.hoisted(() => ({
  onDragDropEvent: vi.fn(),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => webviewMocks,
}));

describe("useDragDrop", () => {
  beforeEach(() => {
    webviewMocks.onDragDropEvent.mockReset();
  });

  it("uses the latest handler without resubscribing", async () => {
    let listener:
      | ((event: { payload: { type: "drop"; paths: string[] } }) => void)
      | undefined;
    const dispose = vi.fn();
    webviewMocks.onDragDropEvent.mockImplementation((handler) => {
      listener = handler;
      return Promise.resolve(dispose);
    });
    const first = vi.fn();
    const second = vi.fn();
    const { rerender, unmount } = renderHook(
      ({ handler }) => useDragDrop(handler),
      { initialProps: { handler: first } },
    );

    await act(async () => Promise.resolve());
    rerender({ handler: second });
    act(() => listener?.({ payload: { type: "drop", paths: ["a.txt"] } }));

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith(["a.txt"]);
    expect(webviewMocks.onDragDropEvent).toHaveBeenCalledTimes(1);
    unmount();
    expect(dispose).toHaveBeenCalledTimes(1);
  });

  it("disposes a listener that resolves after unmount", async () => {
    let resolveListener: ((dispose: () => void) => void) | undefined;
    const dispose = vi.fn();
    webviewMocks.onDragDropEvent.mockReturnValue(
      new Promise<() => void>((resolve) => {
        resolveListener = resolve;
      }),
    );
    const { unmount } = renderHook(() => useDragDrop(vi.fn()));

    unmount();
    await act(async () => {
      resolveListener?.(dispose);
      await Promise.resolve();
    });

    expect(dispose).toHaveBeenCalledTimes(1);
  });
});
