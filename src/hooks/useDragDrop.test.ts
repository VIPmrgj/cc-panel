import { renderHook } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useDragDrop } from "./useDragDrop";

const eventMocks = vi.hoisted(() => ({
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: eventMocks.listen,
}));

type DropEvent = { payload: { grant: unknown } };

describe("useDragDrop", () => {
  beforeEach(() => {
    eventMocks.listen.mockReset();
  });

  it("uses the latest handler without resubscribing", async () => {
    let listener: ((event: DropEvent) => void) | undefined;
    const dispose = vi.fn();
    eventMocks.listen.mockImplementation((_name, handler) => {
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
    act(() => listener?.({ payload: { grant: "grant-1" } }));

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith({ grant: "grant-1" });
    expect(eventMocks.listen).toHaveBeenCalledWith(
      "cc-panel://attachment-drop",
      expect.any(Function),
    );
    expect(eventMocks.listen).toHaveBeenCalledTimes(1);
    unmount();
    expect(dispose).toHaveBeenCalledTimes(1);
  });

  it("disposes a listener that resolves after unmount", async () => {
    let resolveListener: ((dispose: () => void) => void) | undefined;
    const dispose = vi.fn();
    eventMocks.listen.mockReturnValue(
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

  it("ignores malformed native drop events", async () => {
    let listener: ((event: DropEvent) => void) | undefined;
    eventMocks.listen.mockImplementation((_name, handler) => {
      listener = handler;
      return Promise.resolve(vi.fn());
    });
    const onDrop = vi.fn();
    renderHook(() => useDragDrop(onDrop));
    await act(async () => Promise.resolve());

    act(() => {
      listener?.({ payload: { grant: "" } });
      listener?.({ payload: { grant: "   " } });
      listener?.({ payload: { grant: null } });
    });

    expect(onDrop).not.toHaveBeenCalled();
  });
});
