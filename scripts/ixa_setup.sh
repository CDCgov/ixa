#!/bin/sh
# ixa_setup.sh: Download latest ixa code to current directory without git

ZIP_URL="https://github.com/CDCgov/ixa/archive/refs/heads/main.zip"
TARGET_DIR="ixa"

echo "Downloading latest ixa code..."
curl -L "$ZIP_URL" -o ixa.zip

echo "Unzipping..."
unzip -o ixa.zip

# Move extracted source from ixa-main to ixa
EXTRACTED_DIR="ixa-main"
if [ -d "$EXTRACTED_DIR" ]; then
  rm -rf "$TARGET_DIR"
  mv "$EXTRACTED_DIR" "$TARGET_DIR"
fi

rm ixa.zip

echo "Latest ixa code downloaded to ./$TARGET_DIR"
