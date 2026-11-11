#!/bin/sh

cd crate

echo "Building Binaries"
RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release

echo "Copying Binaries to local app bundle"
yes | cp -r ./target/armv7-unknown-linux-musleabi/release/kindle-hello ../kindlichess/bin/

cd ..

echo "Deleting Logs before copying.."
yes | rm ./kindlichess/log/*

echo "Temporarly moving token before copying.."
mv ./kindlichess/secrets/token.json ./

echo "Flashing local app bundle to Kindle"
# kindlichess/* maps 1:1 onto /mnt/us — app root lands at /mnt/us/kindlichess,
# Scriptlet at /mnt/us/documents/kindlichess.sh is the launch entry point.
scp -r ./kindlichess/* root@kindle:/mnt/us

echo "Deploying scriptlet to /mnt/us/documents (SH_Integration auto-registers it)"
scp ./scriptlet/kindlichess.sh root@kindle:/mnt/us/documents/

echo "Moving token back.."
mv ./token.json ./kindlichess/secrets/
echo "Done flashing!"
