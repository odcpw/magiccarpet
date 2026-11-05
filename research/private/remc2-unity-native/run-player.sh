#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)

# Unity's X11 startup can wait indefinitely for a window event under KWin
# Wayland. Use the native backend in a Wayland session; leave X11 sessions
# unchanged. MC_WINDOW_BACKEND=default retains Unity's own backend selection.
mc_wayland_requested=0
case "${MC_WINDOW_BACKEND:-auto}" in
  auto)
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then
      set -- -force-wayland "$@"
      mc_wayland_requested=1
    fi
    ;;
  wayland) set -- -force-wayland "$@"; mc_wayland_requested=1 ;;
  default) ;;
  *)
    echo "MC_WINDOW_BACKEND must be auto, wayland, or default" >&2
    exit 2
    ;;
esac

# Unity and the NVIDIA Wayland driver can both create an explicit-sync
# object for the same surface, causing a fatal "surface already exists".
# The driver supports implicit synchronization for this process. Preserve an
# explicit operator override; other desktop applications keep their settings.
if [ "$mc_wayland_requested" = 1 ] && [ -r /proc/driver/nvidia/version ] &&
   [ -z "${__NV_DISABLE_EXPLICIT_SYNC+x}" ]; then
    export __NV_DISABLE_EXPLICIT_SYNC=1
fi

exec "$script_dir/build-player.sh" --launch "$@"
