#!/bin/sh
# ixa_setup.sh: Download latest ixa code to current directory without git

apt-get update && apt-get install -y curl unzip ca-certificates

echo "Downloading latest ixa code..."
curl -L "https://github.com/CDCgov/ixa/archive/refs/heads/main.zip" -o ixa.zip

echo "Unzipping..."
unzip -o ixa.zip
rm ixa.zip

mv ixa-main ixa
cd ixa

curl https://raw.githubusercontent.com/CDCgov/ocio-certificates/refs/heads/main/data/min-cdc-bundle-ca.crt | tee /usr/local/share/ca-certificates/min-cdc-bundle-ca.crt >/dev/null
update-ca-certificates

cargo bench -p ixa-bench