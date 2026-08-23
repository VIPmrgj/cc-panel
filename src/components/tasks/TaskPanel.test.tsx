import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TaskPanel } from "./TaskPanel";

describe("TaskPanel", () => {
  it("lists the confirmed starter tasks and runs the selected template", async () => {
    const onRun = vi.fn();
    render(<TaskPanel onRun={onRun} />);
    expect(screen.getByRole("heading", { name: "任务" })).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", { name: "开始这个任务" }),
    ).toHaveLength(5);

    await userEvent.click(
      screen.getAllByRole("button", { name: "开始这个任务" })[0],
    );
    expect(onRun).toHaveBeenCalledWith(
      expect.objectContaining({ id: "analyze-project" }),
    );
  });
});
