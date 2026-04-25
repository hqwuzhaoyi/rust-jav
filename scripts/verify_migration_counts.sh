#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/verify_migration_counts.sh snapshot --dir <source_dir> --output <manifest_file>
  scripts/verify_migration_counts.sh compare --before <manifest_file> --after-dir <target_dir>

Commands:
  snapshot   Record the pre-migration file list as relative paths.
  compare    Compare a snapshot against a post-migration directory by:
             - total file count
             - per-extension file count

Notes:
  - This script is for omission checks during move/rename migrations.
  - Operations that intentionally delete files will produce count deltas.
  - actor-links creates hard links under a separate root; compare that tree separately.
EOF
}

die() {
  echo "error: $*" >&2
  exit 2
}

normalize_dir() {
  local dir="$1"
  if [ ! -d "$dir" ]; then
    die "directory does not exist: $dir"
  fi
  (
    cd "$dir"
    pwd
  )
}

stream_relative_files() {
  local dir="$1"
  find "$dir" -type f -print0 | while IFS= read -r -d '' path; do
    if [ "$dir" = "/" ]; then
      printf '%s\n' "${path#/}"
    else
      printf '%s\n' "${path#"$dir"/}"
    fi
  done | LC_ALL=C sort
}

count_extensions_from_manifest() {
  local manifest="$1"
  awk '
    function extension_for(path,   part_count, parts, base) {
      part_count = split(path, parts, "/");
      base = parts[part_count];
      if (base ~ /^\.[^.]+$/ || base !~ /\./) {
        return "[no_ext]";
      }
      sub(/^.*\./, "", base);
      return tolower(base);
    }

    NF {
      counts[extension_for($0)]++;
    }

    END {
      for (ext in counts) {
        printf "%s\t%d\n", ext, counts[ext];
      }
    }
  ' "$manifest" | LC_ALL=C sort
}

write_extension_delta_table() {
  local before_counts="$1"
  local after_counts="$2"
  local output_file="$3"

  awk -F '\t' '
    NR == FNR {
      before[$1] = $2;
      keys[$1] = 1;
      next;
    }

    {
      after[$1] = $2;
      keys[$1] = 1;
    }

    END {
      mismatch = 0;
      for (ext in keys) {
        before_count = (ext in before) ? before[ext] : 0;
        after_count = (ext in after) ? after[ext] : 0;
        delta = after_count - before_count;
        if (delta != 0) {
          mismatch = 1;
        }
        printf "%s\t%d\t%d\t%+d\n", ext, before_count, after_count, delta;
      }
      exit mismatch;
    }
  ' "$before_counts" "$after_counts" > "$output_file"
}

snapshot_command() {
  local dir=""
  local output=""

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --dir)
        [ "$#" -ge 2 ] || die "missing value for --dir"
        dir="$2"
        shift 2
        ;;
      --output)
        [ "$#" -ge 2 ] || die "missing value for --output"
        output="$2"
        shift 2
        ;;
      *)
        die "unknown snapshot argument: $1"
        ;;
    esac
  done

  [ -n "$dir" ] || die "snapshot requires --dir"
  [ -n "$output" ] || die "snapshot requires --output"

  dir="$(normalize_dir "$dir")"
  mkdir -p "$(dirname "$output")"
  stream_relative_files "$dir" > "$output"

  local total
  total="$(awk 'END { print NR + 0 }' "$output")"
  printf 'snapshot=%s\n' "$output"
  printf 'source_dir=%s\n' "$dir"
  printf 'total_files=%s\n' "$total"
}

compare_command() {
  local before=""
  local after_dir=""
  local after_manifest=""
  local before_counts=""
  local after_counts=""
  local delta_table=""

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --before)
        [ "$#" -ge 2 ] || die "missing value for --before"
        before="$2"
        shift 2
        ;;
      --after-dir)
        [ "$#" -ge 2 ] || die "missing value for --after-dir"
        after_dir="$2"
        shift 2
        ;;
      *)
        die "unknown compare argument: $1"
        ;;
    esac
  done

  [ -n "$before" ] || die "compare requires --before"
  [ -f "$before" ] || die "snapshot file does not exist: $before"
  [ -n "$after_dir" ] || die "compare requires --after-dir"

  after_dir="$(normalize_dir "$after_dir")"
  after_manifest="$(mktemp "${TMPDIR:-/tmp}/rust-jav-migration-after.XXXXXX")"
  before_counts="$(mktemp "${TMPDIR:-/tmp}/rust-jav-migration-before-counts.XXXXXX")"
  after_counts="$(mktemp "${TMPDIR:-/tmp}/rust-jav-migration-after-counts.XXXXXX")"
  delta_table="$(mktemp "${TMPDIR:-/tmp}/rust-jav-migration-delta.XXXXXX")"
  trap "rm -f '$after_manifest' '$before_counts' '$after_counts' '$delta_table'" EXIT

  stream_relative_files "$after_dir" > "$after_manifest"
  count_extensions_from_manifest "$before" > "$before_counts"
  count_extensions_from_manifest "$after_manifest" > "$after_counts"

  local before_total
  local after_total
  local total_delta
  local ext_status=0
  local status="ok"

  before_total="$(awk 'END { print NR + 0 }' "$before")"
  after_total="$(awk 'END { print NR + 0 }' "$after_manifest")"
  total_delta=$((after_total - before_total))

  if ! write_extension_delta_table "$before_counts" "$after_counts" "$delta_table"; then
    ext_status=1
  fi

  if [ "$total_delta" -ne 0 ] || [ "$ext_status" -ne 0 ]; then
    status="mismatch"
  fi

  printf 'before_manifest=%s\n' "$before"
  printf 'after_dir=%s\n' "$after_dir"
  printf 'before_total=%s\n' "$before_total"
  printf 'after_total=%s\n' "$after_total"
  printf 'total_delta=%+d\n' "$total_delta"
  printf 'extension\tbefore\tafter\tdelta\n'
  LC_ALL=C sort "$delta_table"
  printf 'status=%s\n' "$status"

  if [ "$status" != "ok" ]; then
    exit 1
  fi
}

main() {
  [ "$#" -gt 0 ] || {
    usage
    exit 2
  }

  local command="$1"
  shift

  case "$command" in
    snapshot)
      snapshot_command "$@"
      ;;
    compare)
      compare_command "$@"
      ;;
    -h|--help|help)
      usage
      ;;
    *)
      die "unknown command: $command"
      ;;
  esac
}

main "$@"
