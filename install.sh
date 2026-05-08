#!/usr/bin/env bash
# Install the latest cmdlog release into ~/.local/share/cmdlog and
# symlink the binary into ~/.local/bin.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/mel0us/cmdlog/main/install.sh | bash
#   ./install.sh                 # install latest release
#   ./install.sh v2.0.2          # install a specific tag

set -euo pipefail

REPO="mel0us/cmdlog"
PREFIX="$HOME/.local/share/cmdlog"
BIN_DIR="$HOME/.local/bin"
TAG="${1:-latest}"

# Linux glibc builds are produced on ubuntu-22.04 (glibc 2.35).
# Older systems get the musl build instead.
GLIBC_REQUIRED="2.35"

err() { printf 'install: %s\n' "$*" >&2; exit 1; }
log() { printf 'install: %s\n' "$*"; }

# Echo the system glibc version (e.g. "2.35"), or empty if not glibc.
detect_glibc() {
    local v
    v=$(ldd --version 2>/dev/null | head -n1 | awk '{print $NF}')
    echo "$v" | grep -qE '^[0-9]+\.[0-9]+$' && echo "$v"
}

# True (0) if the running glibc is >= GLIBC_REQUIRED.
glibc_meets_requirement() {
    local cur
    cur=$(detect_glibc)
    [ -n "$cur" ] || return 1
    [ "$(printf '%s\n%s\n' "$GLIBC_REQUIRED" "$cur" | sort -V | head -n1)" = "$GLIBC_REQUIRED" ]
}

detect_target() {
    local os arch
    os=$(uname -s)
    arch=$(uname -m)
    case "$os/$arch" in
        Darwin/arm64)              echo "aarch64-apple-darwin"; return ;;
        Linux/x86_64)              ;;
        Linux/aarch64|Linux/arm64) ;;
        *) err "unsupported platform: $os/$arch" ;;
    esac

    local libc="gnu"
    if ! glibc_meets_requirement; then
        libc="musl"
        log "system glibc < $GLIBC_REQUIRED (or non-glibc) — using musl build"
    fi

    case "$arch" in
        x86_64)         echo "x86_64-unknown-linux-${libc}" ;;
        aarch64|arm64)  echo "aarch64-unknown-linux-${libc}" ;;
    esac
}

resolve_tag() {
    if [ "$TAG" = "latest" ]; then
        curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
            | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' \
            | head -n1
    else
        echo "$TAG"
    fi
}

tmp=""
trap '[ -n "$tmp" ] && rm -rf "$tmp"' EXIT

main() {
    command -v curl >/dev/null || err "curl is required"
    command -v tar  >/dev/null || err "tar is required"

    local target tag asset url staged
    target=$(detect_target)
    tag=$(resolve_tag)
    [ -n "$tag" ] || err "could not resolve release tag"
    asset="cmdlog-${target}.tar.gz"
    url="https://github.com/$REPO/releases/download/$tag/$asset"

    log "platform: $target"
    log "release:  $tag"
    log "asset:    $asset"

    tmp=$(mktemp -d)

    log "downloading $url"
    curl -fsSL "$url" -o "$tmp/$asset" || err "download failed"

    log "extracting"
    tar -C "$tmp" -xzf "$tmp/$asset"
    staged="$tmp/cmdlog-${target}"
    [ -x "$staged/cmdlog" ] || err "extracted bundle missing cmdlog binary"

    log "installing to $PREFIX"
    mkdir -p "$PREFIX"
    rm -rf "$PREFIX/hook"
    cp "$staged/cmdlog"       "$PREFIX/cmdlog"
    cp "$staged/default.conf" "$PREFIX/default.conf"
    cp -R "$staged/hook"      "$PREFIX/hook"
    chmod +x "$PREFIX/cmdlog"

    mkdir -p "$BIN_DIR"
    ln -sf "$PREFIX/cmdlog" "$BIN_DIR/cmdlog"
    log "symlinked $BIN_DIR/cmdlog -> $PREFIX/cmdlog"

    log "done. run 'cmdlog install <bash|zsh|tcsh>' to wire up your shell."
}

main "$@"
