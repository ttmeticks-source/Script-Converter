#!/usr/bin/env bash
set -euo pipefail

REPO="https://github.com/ttmeticks-source/Script-Converter"
ARCHIVE="https://codeload.github.com/ttmeticks-source/Script-Converter/tar.gz/refs/heads/main"
APP_NAME="RealismCraft"
INSTALL_PATH="/Applications/RealismCraft.app"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This installer is for macOS only."
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust is required. Install the stable Rust toolchain from https://rustup.rs/ and run this command again."
  exit 1
fi

if ! xcode-select -p >/dev/null 2>&1; then
  echo "Apple Command Line Tools are required. Run: xcode-select --install"
  exit 1
fi

TMP_DIR="$(mktemp -d -t realismcraft-install)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

echo "[1/5] Downloading the latest RealismCraft source..."
curl -fL --retry 3 --connect-timeout 20 "$ARCHIVE" -o "$TMP_DIR/source.tar.gz"
mkdir "$TMP_DIR/source"
tar -xzf "$TMP_DIR/source.tar.gz" -C "$TMP_DIR/source" --strip-components=1

echo "[2/5] Building an optimized native macOS binary..."
cd "$TMP_DIR/source"
cargo build --release

BIN="$TMP_DIR/source/target/release/realismcraft"
if [[ ! -x "$BIN" ]]; then
  echo "Build finished but the executable was not found: $BIN"
  exit 1
fi

echo "[3/5] Creating the macOS application bundle..."
APP_BUNDLE="$TMP_DIR/RealismCraft.app"
mkdir -p "$APP_BUNDLE/Contents/MacOS" "$APP_BUNDLE/Contents/Resources"
cp "$BIN" "$APP_BUNDLE/Contents/MacOS/RealismCraft"
chmod 755 "$APP_BUNDLE/Contents/MacOS/RealismCraft"
cat > "$APP_BUNDLE/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>RealismCraft</string>
  <key>CFBundleIdentifier</key><string>com.timeticks.realismcraft</string>
  <key>CFBundleName</key><string>RealismCraft</string>
  <key>CFBundleDisplayName</key><string>RealismCraft</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.games</string>
  <key>NSHighResolutionCapable</key><false/>
</dict>
</plist>
PLIST
printf 'APPL????' > "$APP_BUNDLE/Contents/PkgInfo"

echo "[4/5] Installing into /Applications (macOS may ask for your password)..."
sudo -v
STAGE="/Applications/.RealismCraft.app.new.$$"
BACKUP="/Applications/.RealismCraft.app.old.$$"
sudo rm -rf "$STAGE" "$BACKUP"
sudo ditto "$APP_BUNDLE" "$STAGE"

if [[ -e "$INSTALL_PATH" ]]; then
  sudo mv "$INSTALL_PATH" "$BACKUP"
fi

if sudo mv "$STAGE" "$INSTALL_PATH"; then
  sudo rm -rf "$BACKUP"
else
  if [[ -e "$BACKUP" ]]; then sudo mv "$BACKUP" "$INSTALL_PATH"; fi
  echo "Could not install the app bundle."
  exit 1
fi

echo "[5/5] Opening RealismCraft..."
open "$INSTALL_PATH"
echo
echo "Installed: $INSTALL_PATH"
echo "Your local account and worlds remain in ~/Library/Application Support/RealismCraft."
echo "Source: $REPO"
