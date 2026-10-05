#!/bin/bash
# Builds build/Embers.app without Xcode: swiftc per architecture, lipo, hand-written
# Info.plist, ad-hoc codesign. Approach follows CodexIsland (MIT) — see THIRD_PARTY_NOTICES.md.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

APP_NAME="Embers"
MIN_MACOS="14.0"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: workspace version must be X.Y.Z (got '$VERSION')" >&2
  exit 1
fi

BUILD_DIR="$ROOT/build"
APP_DIR="$BUILD_DIR/$APP_NAME.app"
CONTENTS="$APP_DIR/Contents"

rm -rf "$APP_DIR"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"

SOURCES=()
while IFS= read -r -d '' f; do SOURCES+=("$f"); done \
  < <(find apps/macos/Sources -name '*.swift' -print0 | sort -z)

# swiftc can't emit a multi-arch binary directly: build each slice, then lipo.
SLICES=()
for arch in arm64 x86_64; do
  out="$BUILD_DIR/$APP_NAME-$arch"
  swiftc \
    -target "$arch-apple-macos$MIN_MACOS" \
    -O \
    -parse-as-library \
    -framework AppKit \
    -o "$out" \
    "${SOURCES[@]}"
  SLICES+=("$out")
done
lipo -create "${SLICES[@]}" -output "$CONTENTS/MacOS/$APP_NAME"
rm -f "${SLICES[@]}"

sed -e "s/__VERSION__/$VERSION/g" -e "s/__MIN_MACOS__/$MIN_MACOS/g" \
  apps/macos/Resources/Info.plist.template > "$CONTENTS/Info.plist"
plutil -lint "$CONTENTS/Info.plist" >/dev/null

codesign --force --sign - --timestamp=none "$APP_DIR"

echo "built $APP_DIR ($VERSION)"
