#!/bin/bash
set -euo pipefail

# Create scenario-based fixtures for the current CLI surface.
# Usage: ./examples/create_test_files.sh [directory]

DIR="${1:-./examples/test}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SCENARIOS=(
  "delete-ad-files"
  "standardize-names"
  "extract-codes"
  "categorize-files"
  "move-origin"
  "organize-by-code"
  "clean-empty-dirs"
  "actor-links"
)

reset_dir() {
  local path="$1"
  rm -rf "$path"
  mkdir -p "$path"
}

touch_file() {
  local path="$1"
  mkdir -p "$(dirname "$path")"
  : >"$path"
}

mkdir -p "$DIR"
for scenario in "${SCENARIOS[@]}"; do
  reset_dir "$DIR/$scenario"
done

# ========================================
# 0. delete-ad-files
# ========================================
touch_file "$DIR/delete-ad-files/新片首发每天更新.txt"
touch_file "$DIR/delete-ad-files/大平台真人荷官.html"
touch_file "$DIR/delete-ad-files/乐鱼体育投注.url"
touch_file "$DIR/delete-ad-files/扫码获取最新地址.jpg"
touch_file "$DIR/delete-ad-files/SSNI-888/新片首发每天更新.txt"
touch_file "$DIR/delete-ad-files/SSNI-888/SSNI-888.mkv"
# video file whose name matches an ad pattern — spec allows deletion
touch_file "$DIR/delete-ad-files/新片首发每天更新.mp4"
# regular video — must NOT be matched
touch_file "$DIR/delete-ad-files/STAR-123.mp4"

# ========================================
# 1. standardize-names
# ========================================
touch_file "$DIR/standardize-names/[7sht.me]@MIDE-001.mp4"
touch_file "$DIR/standardize-names/hhd800.com@SSNI-234.avi"

# ========================================
# 2. extract-codes
# ========================================
touch_file "$DIR/extract-codes/sample__abp123-C.mp4"
touch_file "$DIR/extract-codes/clip__ipx789.mkv"

# ========================================
# 3. categorize-files
# ========================================
touch_file "$DIR/categorize-files/SNIS-456-C.mp4"
touch_file "$DIR/categorize-files/UUSS-456-UC.mp4"
touch_file "$DIR/categorize-files/MEYD-456-C-无码流出.mkv"

# ========================================
# 4. move-origin
# ========================================
touch_file "$DIR/move-origin/ABP-123.mp4"
touch_file "$DIR/move-origin/IPX-789.mkv"
touch_file "$DIR/move-origin/JUFE-333.mp4"

# ========================================
# 5. organize-by-code
# ========================================
touch_file "$DIR/organize-by-code/ABP-123.mp4"
touch_file "$DIR/organize-by-code/HEYZO-0890.mp4"
touch_file "$DIR/organize-by-code/PRED-456-cd1.mp4"
touch_file "$DIR/organize-by-code/PRED-456-cd2.mp4"

# ========================================
# 6. clean-empty-dirs
# ========================================
mkdir -p "$DIR/clean-empty-dirs/EMPTY-DIR"
mkdir -p "$DIR/clean-empty-dirs/nested/inner-empty"
touch_file "$DIR/clean-empty-dirs/KEEP/video.mp4"

# ========================================
# 7. actor-links
# ========================================
cp "$REPO_ROOT/REBD-615.nfo" "$DIR/actor-links/REBD-615.nfo"
touch_file "$DIR/actor-links/REBD-615.mp4"
touch_file "$DIR/actor-links/REBD-615-poster.jpg"
touch_file "$DIR/actor-links/REBD-615-backdrop.jpg"

echo ""
echo "=== Created scenario fixtures ==="
echo "Output: $DIR"
echo ""
for scenario in "${SCENARIOS[@]}"; do
  file_count=$(find "$DIR/$scenario" -type f | wc -l | tr -d ' ')
  echo "- $scenario ($file_count files)"
done
echo ""
echo "Suggested smoke commands:"
echo "  cargo run -- ops --dir $DIR/delete-ad-files --op delete-ad-files --json"
echo "  cargo run -- ops --dir $DIR/delete-ad-files --op delete-ad-files --apply --json"
echo "  cargo run -- ops --dir $DIR/standardize-names --op standardize-names --json"
echo "  cargo run -- ops --dir $DIR/extract-codes --op extract-codes --apply --json"
echo "  cargo run -- ops --dir $DIR/categorize-files --op categorize-files --apply --json"
echo "  cargo run -- ops --dir $DIR/move-origin --op move-origin --apply --json"
echo "  cargo run -- ops --dir $DIR/organize-by-code --op organize-by-code --apply --json"
echo "  cargo run -- ops --dir $DIR/clean-empty-dirs --op clean-empty-dirs --apply --json"
echo "  cargo run -- actor-links --source $DIR/actor-links --actors-root /tmp/rust-jav-actors --apply --json"
