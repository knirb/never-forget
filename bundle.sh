#!/bin/bash
set -e

APP_NAME="NeverForget"
BUNDLE_DIR="target/${APP_NAME}.app"

echo "Building release binary..."
cargo build --release

echo "Creating app bundle..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

cp target/release/neverforget "$BUNDLE_DIR/Contents/MacOS/neverforget"
cp Info.plist "$BUNDLE_DIR/Contents/Info.plist"

echo "Bundle created at: $BUNDLE_DIR"
echo ""
echo "To run:  open $BUNDLE_DIR"
echo "To quit: click the orange dot in the menu bar → Quit"
