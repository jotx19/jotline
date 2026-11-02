#!/usr/bin/env bash
# jotline installer.
#
#   curl -fsSL https://raw.githubusercontent.com/jotx19/jotline/main/install.sh | bash
#
# Installs the `jotline` agent binary from the latest GitHub release into ~/.local/bin, verifies
# its SHA-256 checksum, and installs the `jotline-tui` interactive UI with npm when Node.js 18.18+
# is available. Nothing is installed outside the install directory and npm's global prefix.
#
# Environment variables:
#   JOTLINE_VERSION       release tag to install, e.g. v2.1.0 (default: latest)
#   JOTLINE_INSTALL_DIR   where to put the binary (default: ~/.local/bin)
#   JOTLINE_NO_TUI=1      skip installing the interactive UI
#   JOTLINE_DOWNLOAD_BASE release download base URL (default: https://github.com/jotx19/jotline/releases)
#
# The whole script is wrapped in main() so a partially downloaded script never runs.

set -euo pipefail

REPO="jotx19/jotline"
TMP_DIR=""
TUI_INSTALLED=0

if [ -t 1 ]; then
  BOLD=$'\033[1m' DIM=$'\033[2m' RED=$'\033[31m' GREEN=$'\033[32m' YELLOW=$'\033[33m' ACCENT=$'\033[38;5;173m' RESET=$'\033[0m'
else
  BOLD='' DIM='' RED='' GREEN='' YELLOW='' ACCENT='' RESET=''
fi

say() { printf '%s\n' "$*"; }
step() { printf '%s●%s %s\n' "$ACCENT" "$RESET" "$*"; }
ok() { printf '%s✓%s %s\n' "$GREEN" "$RESET" "$*"; }
warn() { printf '%s!%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
die() {
  printf '%s✕ %s%s\n' "$RED" "$*" "$RESET" >&2
  exit 1
}

need() { command -v "$1" >/dev/null 2>&1 || die "this installer needs '$1'; install it and run again."; }

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin)
      # A shell running under Rosetta reports x86_64 on Apple Silicon; prefer the native build.
      if [ "$arch" = "x86_64" ] && [ "$(sysctl -n hw.optional.arm64 2>/dev/null || echo 0)" = "1" ]; then
        arch="arm64"
      fi
      case "$arch" in
        arm64) echo "aarch64-apple-darwin" ;;
        x86_64) echo "x86_64-apple-darwin" ;;
        *) die "unsupported Mac architecture: $arch" ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64 | amd64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64 | arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) die "unsupported Linux architecture: $arch" ;;
      esac
      ;;
    MINGW* | MSYS* | CYGWIN*) die "Windows is not supported natively. Run this inside WSL." ;;
    *) die "unsupported operating system: $os" ;;
  esac
}

resolve_version() {
  local base="$1"
  if [ -n "${JOTLINE_VERSION:-}" ] && [ "${JOTLINE_VERSION}" != "latest" ]; then
    echo "$JOTLINE_VERSION"
    return
  fi
  # /releases/latest redirects to /releases/tag/<tag>; read the tag from the final URL.
  local url
  url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$base/latest")" ||
    die "could not reach $base/latest. Check your connection, or set JOTLINE_VERSION."
  local tag="${url##*/}"
  case "$tag" in
    v[0-9]*) echo "$tag" ;;
    *) die "could not find a published jotline release at $base. Set JOTLINE_VERSION, or build from source: https://github.com/$REPO#install" ;;
  esac
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

download() {
  # download <url> <file>; returns non-zero (without exiting) when the file does not exist.
  curl -fsL --retry 3 -o "$2" "$1"
}

node_ok() {
  command -v node >/dev/null 2>&1 && command -v npm >/dev/null 2>&1 || return 1
  local version major minor
  version="$(node -p 'process.versions.node' 2>/dev/null)" || return 1
  major="${version%%.*}"
  minor="$(echo "$version" | cut -d. -f2)"
  [ "$major" -gt 18 ] || { [ "$major" -eq 18 ] && [ "$minor" -ge 18 ]; }
}

install_tui() {
  local base="$1" tag="$2" tmp="$3"
  if [ "${JOTLINE_NO_TUI:-}" = "1" ]; then
    say "${DIM}  skipping the interactive UI (JOTLINE_NO_TUI=1)${RESET}"
    return
  fi
  if ! node_ok; then
    warn "Node.js 18.18+ not found, so the interactive UI (jotline-tui) was not installed."
    say "  The classic terminal works without it. To add the UI later: install Node.js, then run this installer again."
    return
  fi
  local tarball="$tmp/jotline-tui-$tag.tgz"
  step "Installing the interactive UI (jotline-tui) with npm"
  if ! download "$base/download/$tag/jotline-tui-$tag.tgz" "$tarball"; then
    warn "release $tag has no jotline-tui package; skipping the interactive UI."
    return
  fi
  if npm install --global --no-fund --no-audit --no-update-notifier --loglevel=error "$tarball" >/dev/null; then
    TUI_INSTALLED=1
    local npm_bin
    npm_bin="$(npm prefix --global)/bin"
    ok "jotline-tui installed to $npm_bin/jotline-tui"
    path_hint "$npm_bin"
  else
    warn "npm could not install globally (often a permissions issue with npm's global folder)."
    say "  Fix npm permissions (https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally),"
    say "  or run: npm install --global $base/download/$tag/jotline-tui-$tag.tgz"
  fi
}

path_hint() {
  local dir="$1"
  case ":$PATH:" in
    *":$dir:"*) return ;;
  esac
  local rc="$HOME/.profile"
  case "${SHELL:-}" in
    */zsh) rc="$HOME/.zshrc" ;;
    */bash) rc="$HOME/.bashrc" ;;
    */fish) rc="$HOME/.config/fish/config.fish" ;;
  esac
  say ""
  warn "$dir is not on your PATH yet. Add it, then open a new terminal:"
  if [ "$rc" = "$HOME/.config/fish/config.fish" ]; then
    say "  echo 'fish_add_path $dir' >> $rc"
  else
    say "  echo 'export PATH=\"$dir:\$PATH\"' >> $rc"
  fi
}

main() {
  need curl
  need tar
  need uname
  command -v sha256sum >/dev/null 2>&1 || need shasum

  local base="${JOTLINE_DOWNLOAD_BASE:-https://github.com/$REPO/releases}"
  local dir="${JOTLINE_INSTALL_DIR:-$HOME/.local/bin}"
  local target tag name tmp

  say "${BOLD}${ACCENT}✻ jotline${RESET}${BOLD} installer${RESET}"
  target="$(detect_target)"
  tag="$(resolve_version "$base")"
  name="jotline-$tag-$target"
  step "Installing jotline $tag for $target"

  TMP_DIR="$(mktemp -d)"
  tmp="$TMP_DIR"
  trap 'rm -rf "$TMP_DIR"' EXIT

  download "$base/download/$tag/$name.tar.gz" "$tmp/$name.tar.gz" ||
    die "release $tag has no build for $target. Check https://github.com/$REPO/releases or build from source."
  download "$base/download/$tag/$name.tar.gz.sha256" "$tmp/$name.tar.gz.sha256" ||
    die "release $tag is missing the checksum for $name.tar.gz; refusing to install an unverified file."

  local expected actual
  expected="$(awk '{print $1}' "$tmp/$name.tar.gz.sha256")"
  actual="$(sha256_of "$tmp/$name.tar.gz")"
  [ -n "$expected" ] && [ "$expected" = "$actual" ] ||
    die "checksum mismatch for $name.tar.gz (expected $expected, got $actual). Not installing."
  ok "checksum verified"

  tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
  [ -f "$tmp/$name/jotline" ] || die "the downloaded archive does not contain a jotline binary."
  mkdir -p "$dir"
  install -m 0755 "$tmp/$name/jotline" "$dir/jotline"
  # macOS marks downloaded files as quarantined; the binary is verified above, so clear the flag.
  if [ "$(uname -s)" = "Darwin" ] && command -v xattr >/dev/null 2>&1; then
    xattr -d com.apple.quarantine "$dir/jotline" 2>/dev/null || true
  fi
  ok "jotline installed to $dir/jotline ($("$dir/jotline" --version 2>/dev/null || echo "$tag"))"

  install_tui "$base" "$tag" "$tmp"
  path_hint "$dir"

  say ""
  if command -v ollama >/dev/null 2>&1; then
    ok "Ollama found"
  else
    warn "Ollama not found. Install it from https://ollama.com (or: brew install ollama)."
  fi
  say ""
  say "${BOLD}Next steps${RESET}"
  say "  ollama pull qwen2.5-coder:7b"
  say "  cd your-project"
  if [ "$TUI_INSTALLED" = "1" ]; then
    say "  jotline-tui --model qwen2.5-coder:7b     ${DIM}# interactive UI${RESET}"
  fi
  say "  jotline --model qwen2.5-coder:7b         ${DIM}# classic terminal${RESET}"
  say ""
  say "${DIM}Docs: https://github.com/$REPO · Uninstall: rm $dir/jotline && npm uninstall -g jotline-tui${RESET}"
}

main "$@"
