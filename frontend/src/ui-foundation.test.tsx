import React, { useState, type ComponentType, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import {
  BeUITab,
  BeUITabPanel,
  BeUITabs,
  BeUITabsList,
} from "./beui-tabs";
import { productionStyle, productionValue } from "./test-css";

afterEach(cleanup);

describe("生产 motion Tabs", () => {
  function renderTabs() {
    render(
      <BeUITabs defaultValue="overview">
        <BeUITabsList label="Asset sections">
          <BeUITab value="overview">Overview</BeUITab>
          <BeUITab value="metadata">Metadata</BeUITab>
        </BeUITabsList>
        <BeUITabPanel value="overview">Overview panel</BeUITabPanel>
        <BeUITabPanel value="metadata">Metadata panel</BeUITabPanel>
      </BeUITabs>,
    );
  }

  it("当键盘移动标签时，应同步选择并聚焦对应标签", async () => {
    renderTabs();
    const user = userEvent.setup();
    const overview = screen.getByRole("tab", { name: "Overview" });
    const metadata = screen.getByRole("tab", { name: "Metadata" });

    overview.focus();
    await user.keyboard("{ArrowRight}");

    expect(metadata).toHaveFocus();
    expect(metadata).toHaveAttribute("aria-selected", "true");
    expect(overview).toHaveAttribute("tabindex", "-1");
  });

  it("当标签控制面板时，应通过 ARIA 双向关联标签与面板", () => {
    renderTabs();
    const tablist = screen.getByRole("tablist", { name: "Asset sections" });
    const overview = within(tablist).getByRole("tab", { name: "Overview" });
    const panel = screen.getByRole("tabpanel", { name: "Overview" });

    expect(overview).toHaveAttribute("aria-controls", panel.id);
    expect(panel).toHaveAttribute("aria-labelledby", overview.id);
  });
});

type Toast = {
  id: string;
  title: ReactNode;
  dismissible?: boolean;
  duration?: number;
  action?: {
    label: ReactNode;
    onClick: (toast: Toast) => void;
  };
};

type ToastStackProps = {
  toasts: Toast[];
  onDismiss: (id: string) => void;
};

describe("生产 toast foundation", () => {
  it("当通知出现并被关闭时，应通过 polite live region 宣告并移除通知", async () => {
    const modulePath = "./components/motion/animated-toast-stack";
    const { AnimatedToastStack } = (await import(modulePath)) as {
      AnimatedToastStack: ComponentType<ToastStackProps>;
    };

    function ToastHarness() {
      const [toasts, setToasts] = useState<Toast[]>([
        { id: "saved", title: "Settings saved" },
      ]);
      return (
        <AnimatedToastStack
          toasts={toasts}
          onDismiss={(id) =>
            setToasts((current) => current.filter((toast) => toast.id !== id))
          }
        />
      );
    }

    const { container } = render(<ToastHarness />);
    const liveRegion = container.querySelector('[aria-live="polite"]');
    expect(liveRegion).toHaveAttribute("aria-atomic", "false");
    expect(screen.getByText("Settings saved")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Dismiss toast" }));

    expect(screen.queryByText("Settings saved")).not.toBeInTheDocument();
  });

  it("应让 action 保持 28px 紧凑视觉，同时让 action 与 dismiss 都有 44px 可访问触控目标", async () => {
    const modulePath = "./components/motion/animated-toast-stack";
    const { AnimatedToastStack } = (await import(modulePath)) as {
      AnimatedToastStack: ComponentType<ToastStackProps>;
    };
    const action = vi.fn();
    const dismiss = vi.fn();

    render(
      <AnimatedToastStack
        toasts={[
          {
            id: "undoable",
            title: "Actor removed",
            duration: 0,
            action: { label: "Undo", onClick: action },
          },
        ]}
        onDismiss={dismiss}
      />,
    );

    const actionButton = screen.getByRole("button", { name: "Undo" });
    const actionVisual = actionButton.querySelector(".ui-compact-surface");
    const close = screen.getByRole("button", { name: "Dismiss toast" });
    expect(actionButton.classList.contains("ui-compact-touch-target")).toBe(true);
    expect(actionVisual).not.toBeNull();
    expect([
      productionValue(actionButton, "min-width"),
      productionValue(actionButton, "min-height"),
    ]).toEqual(["44px", "44px"]);
    const actionVisualStyle = productionStyle(actionVisual!);
    expect([actionVisualStyle.height, actionVisualStyle.borderRadius]).toEqual([
      "28px",
      "999px",
    ]);
    expect(close.classList.contains("ui-compact-icon-button")).toBe(true);
    const closeStyle = productionStyle(close);
    expect([
      closeStyle.width,
      closeStyle.height,
      closeStyle.minWidth,
      closeStyle.minHeight,
      closeStyle.borderRadius,
      closeStyle.flexShrink,
    ]).toEqual([
      "44px",
      "44px",
      "44px",
      "44px",
      "50%",
      "0",
    ]);
    expect([
      productionValue(close, "min-width"),
      productionValue(close, "min-height"),
    ]).toEqual(["44px", "44px"]);
    expect(productionStyle(close.querySelector("svg")!).flexShrink).toBe("0");

    actionButton.focus();
    await userEvent.keyboard("{Enter}");
    expect(action).toHaveBeenCalledTimes(1);
    close.focus();
    await userEvent.keyboard("{Enter}");
    expect(dismiss).toHaveBeenCalledWith("undoable");
  });
});

type MorphingModalProps = {
  viewId: string | null;
  onClose: () => void;
  children: ReactNode;
};

describe("生产 morphing modal foundation", () => {
  it("当模态框打开后，应把焦点移入对话框", async () => {
    const modulePath = "./components/motion/morphing-modal";
    const { MorphingModal } = (await import(modulePath)) as {
      MorphingModal: ComponentType<MorphingModalProps>;
    };

    function ModalHarness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>Open preferences</button>
          <MorphingModal viewId={open ? "preferences" : null} onClose={() => setOpen(false)}>
            <section role="dialog" aria-modal="true" aria-labelledby="preferences-title">
              <h2 id="preferences-title">Preferences</h2>
              <button>Save preferences</button>
            </section>
          </MorphingModal>
        </>
      );
    }

    render(<ModalHarness />);
    await userEvent.click(screen.getByRole("button", { name: "Open preferences" }));

    const dialog = screen.getByRole("dialog", { name: "Preferences" });
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
  });

  it("当用户按 Escape 关闭模态框时，应移除对话框并把焦点归还触发按钮", async () => {
    const modulePath = "./components/motion/morphing-modal";
    const { MorphingModal } = (await import(modulePath)) as {
      MorphingModal: ComponentType<MorphingModalProps>;
    };

    function ModalHarness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>Open preferences</button>
          <MorphingModal viewId={open ? "preferences" : null} onClose={() => setOpen(false)}>
            <section role="dialog" aria-modal="true" aria-label="Preferences">
              <button>Save preferences</button>
            </section>
          </MorphingModal>
        </>
      );
    }

    render(<ModalHarness />);
    const opener = screen.getByRole("button", { name: "Open preferences" });
    await userEvent.click(opener);
    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("dialog", { name: "Preferences" })).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it("当用户在模态框边界按 Tab 时，应把焦点圈定在对话框内", async () => {
    const modulePath = "./components/motion/morphing-modal";
    const { MorphingModal } = (await import(modulePath)) as {
      MorphingModal: ComponentType<MorphingModalProps>;
    };

    render(
      <MorphingModal viewId="preferences" onClose={() => undefined}>
        <section role="dialog" aria-label="Preferences">
          <button>First action</button>
          <button>Last action</button>
        </section>
      </MorphingModal>,
    );

    const first = screen.getByRole("button", { name: "First action" });
    const last = screen.getByRole("button", { name: "Last action" });
    last.focus();
    await userEvent.keyboard("{Tab}");
    expect(first).toHaveFocus();
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    expect(last).toHaveFocus();
  });
});
