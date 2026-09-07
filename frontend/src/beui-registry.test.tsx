import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { Button } from "./components/ui/button";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "./components/ui/accordion";
import { Badge } from "./components/ui/badge";
import { Checkbox } from "./components/ui/checkbox";
import { AlertDialog, Dialog, Sheet } from "./components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./components/ui/dropdown-menu";
import { Input } from "./components/ui/input";
import { Progress } from "./components/ui/progress";
import { ScrollArea } from "./components/ui/scroll-area";
import { Select } from "./components/ui/select";
import { Separator } from "./components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";
import { Textarea } from "./components/ui/textarea";
import { ToggleGroup, ToggleGroupItem } from "./components/ui/toggle-group";

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
      <Dialog open title="确认操作" onClose={close}>
        <Button>继续</Button>
      </Dialog>,
    );
    expect(screen.getByRole("dialog", { name: "确认操作" })).toContainElement(document.activeElement as HTMLElement);
    await userEvent.keyboard("{Escape}");
    expect(close).toHaveBeenCalledOnce();
  });

  it("owns Sheet and AlertDialog modal roles while AlertDialog makes Escape opt-in", async () => {
    const close = vi.fn();
    const { rerender } = render(<Sheet open title="筛选资产" onClose={close}><Button>应用</Button></Sheet>);
    expect(screen.getByRole("dialog", { name: "筛选资产" })).toHaveAttribute("aria-modal", "true");
    await userEvent.keyboard("{Escape}");
    expect(close).toHaveBeenCalledOnce();

    rerender(<AlertDialog open title="永久删除" onClose={close}><Button>取消</Button></AlertDialog>);
    expect(screen.getByRole("alertdialog", { name: "永久删除" })).toHaveAttribute("aria-modal", "true");
    await userEvent.keyboard("{Escape}");
    expect(close).toHaveBeenCalledOnce();
  });

  it("keeps a pointer-blocking backdrop behind non-dismissible AlertDialog content", () => {
    const close = vi.fn();
    render(<AlertDialog open title="永久删除" onClose={close}><Button>确认</Button></AlertDialog>);
    const backdrop = document.querySelector('[data-modal-backdrop="blocking"]');
    expect(backdrop).toBeInTheDocument();
    fireEvent.pointerDown(backdrop!);
    fireEvent.click(backdrop!);
    expect(close).not.toHaveBeenCalled();
  });

  it("keeps Textarea, Checkbox and Select as keyboard-native form controls", async () => {
    render(<><Textarea aria-label="备注" /><Checkbox aria-label="选择媒体资产" /><Select aria-label="状态" defaultValue="normal"><option value="normal">正常</option><option value="exception">异常</option></Select></>);
    await userEvent.type(screen.getByRole("textbox", { name: "备注" }), "说明");
    await userEvent.click(screen.getByRole("checkbox", { name: "选择媒体资产" }));
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "状态" }), "exception");
    expect(screen.getByRole("textbox", { name: "备注" })).toHaveValue("说明");
    expect(screen.getByRole("checkbox", { name: "选择媒体资产" })).toBeChecked();
    expect(screen.getByRole("combobox", { name: "状态" })).toHaveValue("exception");
  });

  it("implements Tabs and ToggleGroup arrow-key selection with ARIA state", async () => {
    const changed = vi.fn();
    render(<><Tabs defaultValue="overview"><TabsList label="详情"><TabsTrigger value="overview">概览</TabsTrigger><TabsTrigger value="nfo">NFO</TabsTrigger></TabsList><TabsContent value="overview">概览内容</TabsContent><TabsContent value="nfo">NFO 内容</TabsContent></Tabs><ToggleGroup aria-label="显示模式" defaultValue="grid" onValueChange={changed}><ToggleGroupItem value="grid">网格</ToggleGroupItem><ToggleGroupItem value="list">列表</ToggleGroupItem></ToggleGroup></>);
    screen.getByRole("tab", { name: "概览" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "NFO" })).toHaveAttribute("aria-selected", "true");
    screen.getByRole("button", { name: "网格" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "列表" })).toHaveAttribute("aria-pressed", "true");
    expect(changed).toHaveBeenLastCalledWith("list");
  });

  it("exposes Accordion disclosure, ScrollArea, Badge, and Separator semantics", async () => {
    render(<><Accordion defaultValue="identity"><AccordionItem value="identity"><AccordionTrigger>身份</AccordionTrigger><AccordionContent>媒体资产编号</AccordionContent></AccordionItem></Accordion><ScrollArea role="region" aria-label="完整路径">/media/very-long-path</ScrollArea><Badge>已完成</Badge><Separator /></>);
    const trigger = screen.getByRole("button", { name: "身份" });
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(trigger);
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("region", { name: "完整路径" })).toHaveAttribute("tabindex", "0");
    expect(screen.getByText("已完成")).toHaveClass("beui-badge");
    expect(screen.getByRole("separator")).toHaveAttribute("aria-orientation", "horizontal");
  });
});
