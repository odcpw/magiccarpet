#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)

# Unity's X11 startup can wait indefinitely for a window event under KWin
# Wayland. Use the native backend in a Wayland session; leave X11 sessions
# unchanged. MC_WINDOW_BACKEND=default retains Unity's own backend selection.
case "${MC_WINDOW_BACKEND:-auto}" in
  auto)
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then
      set -- -force-wayland "$@"
    fi
    ;;
  wayland) set -- -force-wayland "$@" ;;
  default) ;;
  *)
    echo "MC_WINDOW_BACKEND must be auto, wayland, or default" >&2
    exit 2
    ;;
esac

exec "$script_dir/build-player.sh" --launch "$@"
