#!/usr/bin/env bash
# Build a Debian package for server-guard.
# Usage:
#   ./scripts/build-deb.sh                 # native arch
#   ./scripts/build-deb.sh amd64           # Debian arch name
#   ./scripts/build-deb.sh arm64
#   TARGET=x86_64-unknown-linux-gnu ./scripts/build-deb.sh amd64
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
DEB_ARCH="${1:-}"
TARGET="${TARGET:-}"

detect_native_deb_arch() {
  case "$(uname -m)" in
    x86_64|amd64) echo "amd64" ;;
    aarch64|arm64) echo "arm64" ;;
    *)
      echo "unsupported architecture: $(uname -m)" >&2
      exit 1
      ;;
  esac
}

rust_target_for_deb_arch() {
  case "$1" in
    amd64) echo "x86_64-unknown-linux-gnu" ;;
    arm64) echo "aarch64-unknown-linux-gnu" ;;
    *)
      echo "unsupported Debian architecture: $1" >&2
      exit 1
      ;;
  esac
}

if [[ -z "$DEB_ARCH" ]]; then
  DEB_ARCH="$(detect_native_deb_arch)"
fi

if [[ -z "$TARGET" ]]; then
  TARGET="$(rust_target_for_deb_arch "$DEB_ARCH")"
fi

echo "==> Building server-guard ${VERSION} for ${DEB_ARCH} (${TARGET})"

if command -v rustup >/dev/null 2>&1; then
  rustup target add "$TARGET" >/dev/null
fi

# Prefer cargo-zigbuild for cross-compilation when available; fall back to cargo.
if [[ "$(uname -s)" != "Linux" ]] || [[ "$(rust_target_for_deb_arch "$(detect_native_deb_arch)" 2>/dev/null || true)" != "$TARGET" ]]; then
  if command -v cargo-zigbuild >/dev/null 2>&1; then
    cargo zigbuild --release --target "$TARGET"
  else
    echo "warning: cross-compiling without cargo-zigbuild; ensure a linker for ${TARGET} is installed" >&2
    cargo build --release --target "$TARGET"
  fi
else
  cargo build --release --target "$TARGET"
fi

BIN_PATH="target/${TARGET}/release/server-guard"
if [[ ! -x "$BIN_PATH" ]]; then
  # native build without explicit --target may land in target/release
  if [[ -x "target/release/server-guard" ]]; then
    BIN_PATH="target/release/server-guard"
  else
    echo "binary not found at ${BIN_PATH}" >&2
    exit 1
  fi
fi

DIST_DIR="${ROOT_DIR}/dist"
PKG_ROOT="${DIST_DIR}/pkg-root-${DEB_ARCH}"
rm -rf "$PKG_ROOT"
mkdir -p \
  "${PKG_ROOT}/DEBIAN" \
  "${PKG_ROOT}/usr/bin" \
  "${PKG_ROOT}/etc/server-guard" \
  "${PKG_ROOT}/lib/systemd/system" \
  "${PKG_ROOT}/var/lib/server-guard"

install -m 0755 "$BIN_PATH" "${PKG_ROOT}/usr/bin/server-guard"
install -m 0640 config/config.example.toml "${PKG_ROOT}/etc/server-guard/config.toml"
install -m 0640 config/settings.example.toml "${PKG_ROOT}/etc/server-guard/settings.toml"
install -m 0644 packaging/systemd/server-guard.service \
  "${PKG_ROOT}/lib/systemd/system/server-guard.service"

# Maintainer scripts
install -m 0755 packaging/debian/postinst "${PKG_ROOT}/DEBIAN/postinst"
install -m 0755 packaging/debian/prerm "${PKG_ROOT}/DEBIAN/prerm"
install -m 0755 packaging/debian/postrm "${PKG_ROOT}/DEBIAN/postrm"
install -m 0644 packaging/debian/conffiles "${PKG_ROOT}/DEBIAN/conffiles"

# Generate control with substituted fields
INSTALLED_SIZE="$(du -sk "${PKG_ROOT}/usr" "${PKG_ROOT}/etc" "${PKG_ROOT}/lib" "${PKG_ROOT}/var" | awk '{s+=$1} END {print s}')"
cat >"${PKG_ROOT}/DEBIAN/control" <<EOF
Package: server-guard
Version: ${VERSION}
Section: admin
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: server-guard contributors <security@example.com>
Installed-Size: ${INSTALLED_SIZE}
Depends: adduser
Homepage: https://github.com/example/server-guard
Description: Lightweight Linux security monitoring agent
 server-guard monitors SSH authentication events on Ubuntu servers
 and reports security events to a central API over HTTPS.
EOF

OUT_DEB="${DIST_DIR}/server-guard_${VERSION}_${DEB_ARCH}.deb"
mkdir -p "$DIST_DIR"

if command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb --root-owner-group --build "$PKG_ROOT" "$OUT_DEB"
else
  # Fallback for macOS / environments without dpkg-deb: produce an ar archive
  # compatible with dpkg when transferred to Ubuntu.
  echo "dpkg-deb not found; building .deb with ar/tar fallback" >&2
  STAGE="${DIST_DIR}/deb-stage-${DEB_ARCH}"
  rm -rf "$STAGE"
  mkdir -p "$STAGE"
  (
    cd "$PKG_ROOT"
    tar --format=gnu -czf "$STAGE/data.tar.gz" usr etc lib var
  )
  (
    cd "${PKG_ROOT}/DEBIAN"
    tar --format=gnu -czf "$STAGE/control.tar.gz" .
  )
  echo "2.0" >"$STAGE/debian-binary"
  (
    cd "$STAGE"
    ar r "$OUT_DEB" debian-binary control.tar.gz data.tar.gz
  )
fi

(
  cd "$DIST_DIR"
  shasum -a 256 "$(basename "$OUT_DEB")" >"$(basename "$OUT_DEB").sha256"
)

echo "==> Built ${OUT_DEB}"
echo "==> Checksum $(cat "${OUT_DEB}.sha256")"
