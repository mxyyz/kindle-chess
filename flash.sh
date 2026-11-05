#!/bin/sh

cd kindle_chess

echo "Building Binaries"
RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release

echo "Copying Binaries to local KUAL-App"
yes | cp -r ./target/armv7-unknown-linux-musleabi/release/kindle-hello ../kindle_KUAL/hellokindle/bin/

cd ..

echo "Deleting Logs before copying.."
yes | rm ./kindle_KUAL/hellokindle/log/*

echo "Temporarly moving token before copying.."
mv ./kindle_KUAL/hellokindle/secrets/token.json ./

echo "Flashing local KUAL-App to Kindle"
# App root deploys as /mnt/us/kindlichess (repo dir stays kindle_KUAL/hellokindle);
# KUAL wiring stays at /mnt/us/extensions/hellokindle. New deploys only — an old
# /mnt/us/hellokindle install on the device is left untouched.
scp -r ./kindle_KUAL/extensions root@kindle:/mnt/us/
scp -r ./kindle_KUAL/hellokindle root@kindle:/mnt/us/kindlichess

echo "Deploying scriptlet to /mnt/us/documents (SH_Integration auto-registers it)"
scp ./scriptlet/kindle-chess.sh root@kindle:/mnt/us/documents/

echo "Moving token back.."
mv ./token.json ./kindle_KUAL/hellokindle/secrets/
echo "Done flashing!"
