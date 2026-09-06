import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { Button } from "./components/ui/button";
import { Dialog } from "./components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./components/ui/dropdown-menu";
import { Input } from "./components/ui/input";
import { Progress } from "./components/ui/progress";

afterEach(cleanup);

describe("beUI registry primitives", () => {
  it("keeps the standard Button at a 44px touch target while native checkbox inputs keep their semantics", async () => {
    const onClick = vi.fn();
    render(
      <>
        <Button onClick={onClick}>保存</Button>
        <Input type="checkbox" aria-label="选择资产" />
      </>,
    );

    const button = screen.getByRole("button", { name: "保存" });
    expect(button).toHaveClass("beui-button", "ui-touch-target");
    button.focus();
    await userEvent.keyboard("{Enter}");
    expect(onClick).toHaveBeenCalledOnce();
    expect(screen.getByRole("checkbox", { name: "选择资产" })).toHaveClass("beui-input");
  });

  it("exposes determinate Progress state to assistive technology", () => {
    render(<Progress value={75} aria-label="媒体存储已使用 75%" />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "75");
  });

  it("moves focus into a Dropdown Menu and lets Escape close it", async () => {
    render(
      <DropdownMenu>
        <DropdownMenuTrigger aria-label="更多操作">•••</DropdownMenuTrigger>
        <DropdownMenuContent aria-label="资产操作">
          <DropdownMenuItem>移除演员目录…</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>,
    );
    await userEvent.click(screen.getByRole("button", { name: "更多操作" }));
    const menu = screen.getByRole("menu", { name: "资产操作" });
    expect(within(menu).getByRole("menuitem")).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menu", { name: "资产操作" })).not.toBeInTheDocument();
  });

  it("adapts the vendored beUI modal with focus and Escape behavior", async () => {
    const close = vi.fn();
    render(
      <Dialog open onClose={close}>
        <section role="dialog" aria-label="确认操作">
          <Button>继续</Button>
        </section>
      </Dialog>,
    );
    expect(screen.getByRole("button", { name: "继续" })).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(close).toHaveBeenCalledOnce();
  });
});
