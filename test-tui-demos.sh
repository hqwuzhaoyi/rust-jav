#!/bin/bash

# TUI 演示测试脚本
# 用于快速测试三个 TUI 方案的演示程序

set -e

echo "🎬 Rust JAV TUI 演示测试脚本"
echo "════════════════════════════════════════════════════════"
echo ""

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

function show_menu() {
    echo -e "${CYAN}请选择要测试的方案：${NC}"
    echo ""
    echo "1) Ratatui - 全屏 TUI，实时进度，完全控制 (推荐 ⭐⭐⭐⭐⭐)"
    echo "2) Cursive - 高层次抽象，类 GUI 开发"
    echo "3) Inquire - 现代交互式 CLI，最简单"
    echo "4) 安装所有依赖"
    echo "5) 查看依赖需求"
    echo "q) 退出"
    echo ""
    read -p "选择 (1-5/q): " choice
}

function install_ratatui() {
    echo -e "${YELLOW}安装 Ratatui 依赖...${NC}"
    cargo add ratatui@0.28
    cargo add crossterm@0.28
    echo -e "${GREEN}✓ Ratatui 依赖安装完成${NC}"
}

function install_cursive() {
    echo -e "${YELLOW}安装 Cursive 依赖...${NC}"
    cargo add cursive@0.21
    echo -e "${GREEN}✓ Cursive 依赖安装完成${NC}"
}

function install_inquire() {
    echo -e "${YELLOW}安装 Inquire 依赖...${NC}"
    cargo add inquire@0.7
    # indicatif 已经在项目中
    echo -e "${GREEN}✓ Inquire 依赖安装完成${NC}"
}

function run_ratatui_demo() {
    echo -e "${YELLOW}运行 Ratatui 演示...${NC}"
    echo -e "${CYAN}操作提示：${NC}"
    echo "  - ↑/↓ 或 j/k: 上下移动"
    echo "  - 空格: 切换选项"
    echo "  - e: 编辑输出目录"
    echo "  - Enter: 开始处理"
    echo "  - q: 退出"
    echo ""
    read -p "按 Enter 继续..."

    cargo run --example tui_ratatui_demo 2>&1 || {
        echo -e "${YELLOW}提示：如果出现编译错误，请先运行选项 4 安装依赖${NC}"
        return 1
    }
}

function run_cursive_demo() {
    echo -e "${YELLOW}运行 Cursive 演示...${NC}"
    echo -e "${CYAN}操作提示：${NC}"
    echo "  - 鼠标或 Tab: 切换组件"
    echo "  - 空格: 切换复选框"
    echo "  - Enter: 确认按钮"
    echo ""
    read -p "按 Enter 继续..."

    cargo run --example tui_cursive_demo 2>&1 || {
        echo -e "${YELLOW}提示：如果出现编译错误，请先运行选项 4 安装依赖${NC}"
        return 1
    }
}

function run_inquire_demo() {
    echo -e "${YELLOW}运行 Inquire 演示...${NC}"
    echo -e "${CYAN}操作提示：${NC}"
    echo "  - ↑/↓: 上下移动"
    echo "  - 空格: 选择/取消选择"
    echo "  - Enter: 确认"
    echo "  - Esc: 取消"
    echo ""
    read -p "按 Enter 继续..."

    cargo run --example tui_inquire_demo 2>&1 || {
        echo -e "${YELLOW}提示：如果出现编译错误，请先运行选项 4 安装依赖${NC}"
        return 1
    }
}

function install_all() {
    echo -e "${YELLOW}安装所有 TUI 依赖...${NC}"
    echo ""
    install_ratatui
    echo ""
    install_cursive
    echo ""
    install_inquire
    echo ""
    echo -e "${GREEN}✓ 所有依赖安装完成！${NC}"
}

function show_dependencies() {
    echo -e "${CYAN}依赖需求：${NC}"
    echo ""
    echo "方案一 - Ratatui:"
    echo "  ratatui = \"0.28\""
    echo "  crossterm = \"0.28\""
    echo ""
    echo "方案二 - Cursive:"
    echo "  cursive = \"0.21\""
    echo ""
    echo "方案三 - Inquire:"
    echo "  inquire = \"0.7\""
    echo "  indicatif = \"0.17.8\" (已存在)"
    echo ""
    echo "您也可以手动在 Cargo.toml 的 [dependencies] 中添加以上依赖"
    echo ""
}

# 主循环
while true; do
    show_menu

    case $choice in
        1)
            echo ""
            run_ratatui_demo
            echo ""
            ;;
        2)
            echo ""
            run_cursive_demo
            echo ""
            ;;
        3)
            echo ""
            run_inquire_demo
            echo ""
            ;;
        4)
            echo ""
            install_all
            echo ""
            ;;
        5)
            echo ""
            show_dependencies
            echo ""
            ;;
        q|Q)
            echo ""
            echo -e "${GREEN}感谢使用！${NC}"
            exit 0
            ;;
        *)
            echo ""
            echo -e "${YELLOW}无效选择，请重试${NC}"
            echo ""
            ;;
    esac

    read -p "按 Enter 返回菜单..."
    clear
done
