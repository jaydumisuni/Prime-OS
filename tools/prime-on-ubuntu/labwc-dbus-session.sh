#!/bin/sh
# Isolated GUI session bus: Ubuntu's XFCE/Thunar daemon stays on the host bus.
set -eu
: "$PRIME_UBUNTU_TOOLS"
exec /usr/bin/dbus-run-session -- "$PRIME_UBUNTU_TOOLS/labwc-session.sh"
