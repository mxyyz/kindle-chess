#!/bin/sh

cd crate

echo "Building Binaries"
RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release

echo "Copying Binaries to local KUAL-App"
yes | cp -r ./target/armv7-unknown-linux-musleabi/release/kindle-hello ../kindle_KUAL/kindlichess/bin/

cd ..

echo "Deleting Logs before copying.."
yes | rm ./kindle_KUAL/kindlichess/log/*

echo "Temporarly moving token before copying.."
mv ./kindle_KUAL/kindlichess/secrets/token.json ./

echo "Flashing local KUAL-App to Kindle"
# kindle_KUAL/* maps 1:1 onto /mnt/us — app root lands at /mnt/us/kindlichess,
# KUAL wiring at /mnt/us/extensions/hellokindle. New deploys only — an old
# /mnt/us/hellokindle install on the device is left untouched.
scp -r ./kindle_KUAL/* root@kindle:/mnt/us

echo "Deploying scriptlet to /mnt/us/documents (SH_Integration auto-registers it)"
scp ./scriptlet/kindlichess.sh root@kindle:/mnt/us/documents/

echo "Moving token back.."
mv ./token.json ./kindle_KUAL/kindlichess/secrets/
echo "Done flashing!"
