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

for tool in curl tar swift sips iconutil ditto; do
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

cat > "$TMP_DIR/make-icon.swift" <<'SWIFT'
import AppKit
import Foundation

func polygon(_ points: [(CGFloat, CGFloat)], color: NSColor) {
    let path = NSBezierPath()
    guard let first = points.first else { return }
    path.move(to: NSPoint(x: first.0, y: first.1))
    for point in points.dropFirst() {
        path.line(to: NSPoint(x: point.0, y: point.1))
    }
    path.close()
    color.setFill()
    path.fill()
}

let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: 1024,
    pixelsHigh: 1024,
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
)!
let graphics = NSGraphicsContext(bitmapImageRep: bitmap)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = graphics
graphics.imageInterpolation = .high
graphics.cgContext.setShouldAntialias(true)

let background = NSBezierPath(roundedRect: NSRect(x: 30, y: 30, width: 964, height: 964), xRadius: 218, yRadius: 218)
NSGradient(colors: [
    NSColor(calibratedRed: 0.025, green: 0.075, blue: 0.23, alpha: 1),
    NSColor(calibratedRed: 0.07, green: 0.24, blue: 0.39, alpha: 1),
    NSColor(calibratedRed: 0.10, green: 0.36, blue: 0.36, alpha: 1)
])!.draw(in: background, angle: 90)

let glow = NSBezierPath(ovalIn: NSRect(x: 650, y: 635, width: 190, height: 190))
NSGradient(colors: [
    NSColor(calibratedRed: 1.0, green: 0.88, blue: 0.52, alpha: 0.95),
    NSColor(calibratedRed: 0.94, green: 0.48, blue: 0.25, alpha: 0.08)
])!.draw(in: glow, angle: 45)

for (x, y, radius) in [(170.0, 820.0, 5.0), (315.0, 875.0, 3.0), (820.0, 870.0, 4.0), (880.0, 720.0, 3.0)] {
    NSColor(calibratedWhite: 1.0, alpha: 0.72).setFill()
    NSBezierPath(ovalIn: NSRect(x: x, y: y, width: radius * 2, height: radius * 2)).fill()
}

// Isometric grass-and-soil block: a distinctive, original game icon.
polygon([(248, 590), (512, 736), (776, 590), (512, 444)],
        color: NSColor(calibratedRed: 0.30, green: 0.72, blue: 0.28, alpha: 1))
polygon([(248, 590), (512, 444), (512, 176), (248, 322)],
        color: NSColor(calibratedRed: 0.45, green: 0.29, blue: 0.17, alpha: 1))
polygon([(512, 444), (776, 590), (776, 322), (512, 176)],
        color: NSColor(calibratedRed: 0.31, green: 0.19, blue: 0.13, alpha: 1))

// Grass seams and soil strata add detail while staying crisp at small sizes.
polygon([(248, 590), (512, 736), (512, 700), (248, 554)],
        color: NSColor(calibratedRed: 0.17, green: 0.53, blue: 0.20, alpha: 1))
polygon([(512, 736), (776, 590), (776, 554), (512, 700)],
        color: NSColor(calibratedRed: 0.12, green: 0.43, blue: 0.19, alpha: 1))
polygon([(274, 500), (512, 368), (512, 350), (274, 482)],
        color: NSColor(calibratedRed: 0.57, green: 0.39, blue: 0.23, alpha: 0.65))
polygon([(536, 430), (750, 548), (750, 531), (536, 412)],
        color: NSColor(calibratedRed: 0.48, green: 0.29, blue: 0.17, alpha: 0.75))

let edge = NSBezierPath()
edge.lineWidth = 9
edge.lineJoinStyle = .round
edge.move(to: NSPoint(x: 248, y: 590))
edge.line(to: NSPoint(x: 512, y: 736))
edge.line(to: NSPoint(x: 776, y: 590))
NSColor(calibratedRed: 0.70, green: 0.93, blue: 0.58, alpha: 0.9).setStroke()
edge.stroke()

let border = NSBezierPath(roundedRect: NSRect(x: 30, y: 30, width: 964, height: 964), xRadius: 218, yRadius: 218)
border.lineWidth = 5
NSColor(calibratedWhite: 1.0, alpha: 0.13).setStroke()
border.stroke()

NSGraphicsContext.restoreGraphicsState()
guard let png = bitmap.representation(using: .png, properties: [:]) else {
    fatalError("Could not encode the RealismCraft icon.")
}
try png.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
SWIFT

swift "$TMP_DIR/make-icon.swift" "$TMP_DIR/RealismCraft.png"
ICONSET="$APP_BUNDLE/Contents/Resources/RealismCraft.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  double_size=$((size * 2))
  sips -z "$size" "$size" "$TMP_DIR/RealismCraft.png" --out "$ICONSET/icon_\${size}x\${size}.png" >/dev/null
  sips -z "$double_size" "$double_size" "$TMP_DIR/RealismCraft.png" --out "$ICONSET/icon_\${size}x\${size}@2x.png" >/dev/null
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
