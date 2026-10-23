#!/bin/sh
# Runs a Nuvora server binary as a macOS background service (a LaunchAgent):
# it starts when you log in, starts again if it stops, and keeps running
# after the terminal or app that started it closes.
#
#   scripts/macos-service.sh install --bin ~/.nuvora/bin/nuvora --data-dir ~/.nuvora/data
#   scripts/macos-service.sh status
#   scripts/macos-service.sh uninstall
#
# install also takes --bind (default 0.0.0.0:8640) and --log (default
# ~/Library/Logs/Nuvora/server.log). Running install again applies new
# settings.
set -eu

LABEL=app.nuvora.server
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
DOMAIN="gui/$(id -u)"

die() { echo "macos-service: $*" >&2; exit 1; }

# Paths go into XML.
xml() { printf '%s' "$1" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }

absolute() {
  case "$1" in
    "~"/*) printf '%s/%s' "$HOME" "${1#\~/}" ;;
    /*) printf '%s' "$1" ;;
    *) printf '%s/%s' "$PWD" "$1" ;;
  esac
}

install() {
  BIN="" DATA_DIR="" BIND=0.0.0.0:8640 LOG="$HOME/Library/Logs/Nuvora/server.log"
  while [ $# -gt 0 ]; do
    case "$1" in
      --bin) BIN=$(absolute "$2"); shift 2 ;;
      --data-dir) DATA_DIR=$(absolute "$2"); shift 2 ;;
      --bind) BIND=$2; shift 2 ;;
      --log) LOG=$(absolute "$2"); shift 2 ;;
      *) die "unknown option $1" ;;
    esac
  done
  [ -n "$BIN" ] && [ -x "$BIN" ] || die "--bin must name the nuvora binary"
  [ -n "$DATA_DIR" ] || die "--data-dir is required"
  # launchd starts programs with a bare PATH; Nuvora finds ffmpeg on PATH.
  FFMPEG=$(command -v ffmpeg) || die "ffmpeg is not on PATH; install it first (brew install ffmpeg)"
  SEARCH="$(dirname "$FFMPEG"):/usr/bin:/bin:/usr/sbin:/sbin"
  mkdir -p "$(dirname "$PLIST")" "$(dirname "$LOG")" "$DATA_DIR"

  cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$(xml "$BIN")</string>
    <string>--data-dir</string><string>$(xml "$DATA_DIR")</string>
    <string>--bind</string><string>$(xml "$BIND")</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key><string>$(xml "$SEARCH")</string>
    <key>RUST_LOG</key><string>warn</string>
  </dict>
  <key>WorkingDirectory</key><string>$(xml "$DATA_DIR")</string>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>StandardOutPath</key><string>$(xml "$LOG")</string>
  <key>StandardErrorPath</key><string>$(xml "$LOG")</string>
</dict>
</plist>
EOF
  plutil -lint -s "$PLIST" || die "wrote an invalid $PLIST"
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  launchctl enable "$DOMAIN/$LABEL"
  launchctl bootstrap "$DOMAIN" "$PLIST"
  echo "Nuvora runs in the background now and starts when you log in. Log: $LOG"
}

uninstall() {
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST"
  echo "Nuvora no longer runs in the background."
}

status() {
  [ -f "$PLIST" ] || { echo "not installed"; return; }
  launchctl print "$DOMAIN/$LABEL" 2>/dev/null | sed -n -e 's/^\t\(state = .*\)/\1/p' -e 's/^\t\(pid = .*\)/\1/p' -e 's/^\t\(last exit code = .*\)/\1/p' \
    || echo "installed but not loaded; run install again"
}

case "${1:-}" in
  install) shift; install "$@" ;;
  uninstall) uninstall ;;
  status) status ;;
  *) die "usage: $0 install --bin PATH --data-dir DIR [--bind ADDR] [--log FILE] | status | uninstall" ;;
esac
