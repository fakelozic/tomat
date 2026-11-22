#!/bin/sh
# Installs the built tomat for this user only: both programs, the menu entry and the icon. No sudo, no autostart.
# Usage: scripts/install.sh [--uninstall]
# Build and check first (`mise run build`). A running tomat keeps its old version until it is stopped.
set -eu
cd "$(dirname "$0")/.."

data=${XDG_DATA_HOME:-$HOME/.local/share}
home=$HOME/.local/lib/tomat
link=$HOME/.local/bin/tomat
entry=$data/applications/tomat.desktop
icons=$data/icons/hicolor
icon=$icons/scalable/apps/tomat.svg

refresh() {
  # An existing GTK cache hides new icons until it is rebuilt.
  if [ -f "$icons/icon-theme.cache" ] && command -v gtk-update-icon-cache > /dev/null; then
    gtk-update-icon-cache --force --ignore-theme-index --quiet "$icons"
  fi
  if command -v kbuildsycoca6 > /dev/null; then
    kbuildsycoca6 > /dev/null 2>&1 || echo "Plasma's menu didn't refresh. It will at the next login." >&2
  fi
}

case ${1-} in
--uninstall)
  rm -f "$home/tomat" "$home/tomat-window" "$link" "$entry" "$icon"
  rmdir "$home" 2> /dev/null || true # Not empty: something of the user's is in it.
  refresh
  echo "Removed tomat. Your slots and logs are untouched."
  exit 0 ;;
"") ;;
*) echo "Usage: $0 [--uninstall]" >&2; exit 2 ;;
esac

app=target/x86_64-unknown-linux-musl/release/tomat
window=target/release/tomat-window
for built in "$app" "$window"; do
  [ -x "$built" ] || { echo "Not built yet: run mise run build" >&2; exit 1; }
done
if [ -n "$(find src assets Cargo.toml Cargo.lock -type f -newer "$app" -print -quit)" ] ||
   [ -n "$(find src assets window Cargo.lock -type f -newer "$window" -print -quit)" ]; then
  echo "The build is older than the source: run mise run build" >&2
  exit 1
fi
"$app" --version > /dev/null

# The entry needs the full path, because ~/.local/bin isn't on Plasma's PATH.
case $home in
*[!A-Za-z0-9/._\ -]*) echo "Can't write a menu entry for this path: $home" >&2; exit 1 ;;
esac
if [ -e "$link" ] && [ ! -L "$link" ]; then
  echo "Something else is at $link: move it away first" >&2
  exit 1
fi

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
sed "s|@BIN@|$home/tomat|g" packaging/tomat.desktop > "$stage/tomat.desktop"
if command -v desktop-file-validate > /dev/null; then
  desktop-file-validate "$stage/tomat.desktop"
fi

# Each file appears whole or not at all, and a running tomat keeps the file it started from.
put() {
  mkdir -p "$(dirname "$3")"
  install -m "$1" "$2" "$3.new"
  mv -f "$3.new" "$3"
}
put 755 "$window" "$home/tomat-window"
put 755 "$app" "$home/tomat"
put 644 packaging/tomat.svg "$icon"
put 644 "$stage/tomat.desktop" "$entry"
mkdir -p "$(dirname "$link")"
ln -sfn "$home/tomat" "$link"
refresh

echo "Installed tomat:"
printf '  %s\n' "$home/tomat" "$home/tomat-window" "$link" "$entry" "$icon"
