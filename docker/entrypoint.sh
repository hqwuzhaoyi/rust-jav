#!/bin/sh
set -eu

uid="${RUST_JAV_UID:-568}"
gid="${RUST_JAV_GID:-568}"
case "$uid:$gid" in *[!0-9:]*|:*|*:) echo "RUST_JAV_UID and RUST_JAV_GID must be numeric" >&2; exit 64;; esac
if [ "$uid" -eq 0 ] || [ "$gid" -eq 0 ]; then
  echo "rust-jav refuses to run the Management Interface as root" >&2
  exit 77
fi

if [ "$(id -u)" -eq 0 ]; then
  groupmod --non-unique --gid "$gid" rust-jav
  usermod --non-unique --uid "$uid" --gid "$gid" rust-jav
  exec gosu "$uid:$gid" "$0" "$@"
fi
if [ "$(id -u)" -ne "$uid" ] || [ "$(id -g)" -ne "$gid" ]; then
  echo "container identity $(id -u):$(id -g) does not match configured $uid:$gid" >&2
  exit 77
fi

exec /usr/local/bin/rust-jav-bin "$@"
