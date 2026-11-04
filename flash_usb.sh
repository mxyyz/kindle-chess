#!/bin/sh
sudo mount /dev/sdb1 /mnt/tmp
cd kindle_chess

echo "Building Binaries"
RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release

echo "Copying Binaries to local KUAL-App"
yes | sudo cp -r ./target/armv7-unknown-linux-musleabi/release/kindle-hello ../kindle_KUAL/hellokindle/bin/

cd ..

echo "Deleting Logs before copying.."
yes | sudo rm ./kindle_KUAL/hellokindle/log/*

echo "Temporarly moving token before copying.."
mv ./kindle_KUAL/hellokindle/secrets/token.json ./

echo "Flashing local KUAL-App to Kindle"
# kindle_KUAL/documents/kindle-chess.sh lands in /mnt/us/documents — the
# jailbreak's SH_Integration auto-registers it as a launchable scriptlet.
yes | sudo cp -r ./kindle_KUAL/* /mnt/tmp/

echo "Moving token back.."
mv ./token.json ./kindle_KUAL/hellokindle/secrets/

sudo umount /mnt/tmp

echo "Done flashing!"
