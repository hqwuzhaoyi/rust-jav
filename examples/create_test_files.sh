#!/bin/bash
# Create test files for TUI testing
# Usage: ./create_test_files.sh [directory]

DIR="${1:-.}"

echo "Creating test files in: $DIR"

# ========================================
# 1. 单独的视频文件（无目录）
# ========================================
touch "$DIR/ABP-123.mp4"                    # 标准格式
touch "$DIR/SNIS-456-C.mp4"                 # 中文字幕
touch "$DIR/IPX-789.mkv"                    # MKV 格式
touch "$DIR/[7sht.me]@MIDE-001.mp4"         # 带前缀
touch "$DIR/hhd800.com@SSNI-234.avi"        # 另一种前缀
touch "$DIR/CARIB-567-uncensored.mp4"       # 无码
touch "$DIR/FC2-PPV-1234567.mp4"            # FC2 格式
touch "$DIR/HEYZO-0890.mp4"                 # HEYZO 格式
touch "$DIR/1PONDO-010120_001.mp4"          # 一本道格式
touch "$DIR/MEYD-456-C-无码流出.mkv"         # 中文字幕+无码流出
touch "$DIR/UUSS-456-UC.mp4"                # 无码中文 (Uncensored Chinese)

# ========================================
# 2. 目录内含视频文件的场景
# ========================================

# 正常的已整理目录
mkdir -p "$DIR/STAR-123"
touch "$DIR/STAR-123/STAR-123.mp4"
touch "$DIR/STAR-123/STAR-123.nfo"
touch "$DIR/STAR-123/fanart.jpg"
touch "$DIR/STAR-123/poster.jpg"

# 目录内有多个视频
mkdir -p "$DIR/PRED-456"
touch "$DIR/PRED-456/PRED-456-cd1.mp4"
touch "$DIR/PRED-456/PRED-456-cd2.mp4"
touch "$DIR/PRED-456/PRED-456.nfo"

# 目录带中文字幕标记
mkdir -p "$DIR/JUL-789-C"
touch "$DIR/JUL-789-C/JUL-789-C.mkv"
touch "$DIR/JUL-789-C/JUL-789.srt"

# 嵌套目录结构
mkdir -p "$DIR/FSDSS-001/trailers"
touch "$DIR/FSDSS-001/FSDSS-001.mp4"
touch "$DIR/FSDSS-001/trailers/trailer.mp4"

# ========================================
# 3. 包含广告/垃圾文件的目录
# ========================================

# 目录内有广告文件
mkdir -p "$DIR/MIAA-222"
touch "$DIR/MIAA-222/MIAA-222.mp4"
touch "$DIR/MIAA-222/新 片 首 發 每 天 更 新 同 步 日 韓.txt"
touch "$DIR/MIAA-222/聚 合 全 網 H 直 播.html"
touch "$DIR/MIAA-222/最 新 位 址 獲 取.url"
touch "$DIR/MIAA-222/苍老师推荐.txt"
touch "$DIR/MIAA-222/美女荷官在线发牌.jpg"

# 另一个有广告的目录
mkdir -p "$DIR/SSNI-888"
touch "$DIR/SSNI-888/SSNI-888.mkv"
touch "$DIR/SSNI-888/uur93最新地址.txt"
touch "$DIR/SSNI-888/乐鱼体育投注.url"
touch "$DIR/SSNI-888/全 网 最 全 资 源.txt"
touch "$DIR/SSNI-888/扫码获取最新地址.jpg"
touch "$DIR/SSNI-888/N房间进入.html"

# 有广告的单独视频旁边
touch "$DIR/JUFE-333.mp4"
touch "$DIR/新片首发每天更新.txt"
touch "$DIR/有趣台妹小视频.url"
touch "$DIR/大平台真人荷官.html"

# ========================================
# 4. 边缘情况
# ========================================

# 只有 nfo 没有视频的目录（应被清理）
mkdir -p "$DIR/EMPTY-001"
touch "$DIR/EMPTY-001/EMPTY-001.nfo"
touch "$DIR/EMPTY-001/fanart.jpg"

# 空目录
mkdir -p "$DIR/EMPTY-DIR"

# 只有 trailers 的目录
mkdir -p "$DIR/TRAILER-ONLY/trailers"
touch "$DIR/TRAILER-ONLY/trailers/trailer.mp4"

echo ""
echo "=== Created test structure ==="
echo ""
echo "Single video files: 11"
echo "Directories with videos: 6"
echo "Directories with ads: 2"
echo "Edge cases: 3"
echo ""
echo "Directory structure:"
find "$DIR" -maxdepth 2 -type d | sort
echo ""
echo "Files with ads pattern:"
find "$DIR" -name "*新*" -o -name "*最*" -o -name "*荷官*" -o -name "*体育*" -o -name "*台妹*" -o -name "*uur*" -o -name "*房间*" -o -name "*扫码*" 2>/dev/null | head -20
