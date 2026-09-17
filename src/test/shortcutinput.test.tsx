import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { ShortcutInput } from "../components/settings/ShortcutInput";

vi.mock("../app/ipc", () => ({ ipc: { shortcutsCapture: vi.fn(async () => undefined) } }));

describe("ShortcutInput", () => {
  it("records a key combination", () => {
    const onChange = vi.fn();
    render(<ShortcutInput label="Open" value="CommandOrControl+Space" defaultValue="CommandOrControl+Space" onChange={onChange} />);
    const btn = screen.getByLabelText(/Open:/);
    fireEvent.click(btn);
    fireEvent.keyDown(window, { code: "KeyJ", ctrlKey: true, altKey: true });
    expect(onChange).toHaveBeenCalledWith("CommandOrControl+Alt+J");
  });
});
