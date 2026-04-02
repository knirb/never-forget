#!/bin/bash
set -e

APP_NAME="NeverForget"
BUNDLE_DIR="target/${APP_NAME}.app"

# Get version from latest git tag, stripping the 'v' prefix
VERSION=$(git describe --tags --abbrev=0 2>/dev/null | sed 's/^v//')
if [ -z "$VERSION" ]; then
    echo "Error: No git tag found. Create one with: git tag v0.1.0"
    exit 1
fi
echo "Version: $VERSION (from git tag v$VERSION)"

echo "Building release binary..."
cargo build --release

echo "Creating app bundle..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

cp target/release/neverforget "$BUNDLE_DIR/Contents/MacOS/neverforget"

# Inject version into Info.plist
sed "s/0\.1\.0/$VERSION/g" Info.plist > "$BUNDLE_DIR/Contents/Info.plist"

echo "Bundle created at: $BUNDLE_DIR"
echo ""
echo "To run:  open $BUNDLE_DIR"
echo "To quit: click the orange dot in the menu bar → Quit"
