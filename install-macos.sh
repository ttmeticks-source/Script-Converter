#!/usr/bin/env bash
set -euo pipefail

REPO="https://github.com/ttmeticks-source/Script-Converter"
ARCHIVE="https://codeload.github.com/ttmeticks-source/Script-Converter/tar.gz/refs/heads/main"
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

for tool in curl tar sips iconutil ditto; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Required macOS tool not found: $tool"
    exit 1
  fi
done

TMP_DIR="$(mktemp -d -t realismcraft-install)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

echo "[1/6] Downloading the latest RealismCraft source..."
curl -fL --retry 3 --connect-timeout 20 "$ARCHIVE" -o "$TMP_DIR/source.tar.gz"
mkdir "$TMP_DIR/source"
tar -xzf "$TMP_DIR/source.tar.gz" -C "$TMP_DIR/source" --strip-components=1

echo "[2/6] Building an optimized native macOS binary..."
cd "$TMP_DIR/source"
cargo build --release

BIN="$TMP_DIR/source/target/release/realismcraft"
if [[ ! -x "$BIN" ]]; then
  echo "Build finished but the executable was not found: $BIN"
  exit 1
fi

echo "[3/6] Creating the macOS application bundle..."
APP_BUNDLE="$TMP_DIR/RealismCraft.app"
mkdir -p "$APP_BUNDLE/Contents/MacOS" "$APP_BUNDLE/Contents/Resources"
cp "$BIN" "$APP_BUNDLE/Contents/MacOS/RealismCraft"
chmod 755 "$APP_BUNDLE/Contents/MacOS/RealismCraft"

ICON_SOURCE="$TMP_DIR/source/assets/realismcraft-icon.png"
if [[ ! -f "$ICON_SOURCE" ]]; then
  echo "The bundled RealismCraft icon artwork is missing: $ICON_SOURCE"
  exit 1
fi

ICONSET="$APP_BUNDLE/Contents/Resources/RealismCraft.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  double_size=$((size * 2))
  sips -z "$size" "$size" "$ICON_SOURCE" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  sips -z "$double_size" "$double_size" "$ICON_SOURCE" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP_BUNDLE/Contents/Resources/RealismCraft.icns"

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
  <key>CFBundleIconFile</key><string>RealismCraft.icns</string>
  <key>CFBundleShortVersionString</key><string>0.2.0</string>
  <key>CFBundleVersion</key><string>0.2.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.games</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
printf 'APPL????' > "$APP_BUNDLE/Contents/PkgInfo"

echo "[4/6] Installing into /Applications (macOS may ask for your password)..."
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

echo "[5/6] Registering the updated app bundle..."
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$INSTALL_PATH" >/dev/null 2>&1 || true

echo "[6/6] Opening RealismCraft..."
open "$INSTALL_PATH"
echo
echo "Installed: $INSTALL_PATH"
echo "Custom icon: $INSTALL_PATH/Contents/Resources/RealismCraft.icns"
echo "Your local account and worlds remain in ~/Library/Application Support/RealismCraft."
echo "Source: $REPO"
