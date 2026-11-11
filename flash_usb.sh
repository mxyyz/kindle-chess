#!/bin/sh
sudo mount /dev/sdb1 /mnt/tmp
cd crate

echo "Building Binaries"
RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release

echo "Copying Binaries to local KUAL-App"
yes | sudo cp -r ./target/armv7-unknown-linux-musleabi/release/kindle-hello ../kindlichess/bin/

cd ..

echo "Deleting Logs before copying.."
yes | sudo rm ./kindlichess/log/*

echo "Temporarly moving token before copying.."
mv ./secrets/token.json ./

echo "Flashing local KUAL-App to Kindle"
# kindlichess/* maps 1:1 onto the mounted Kindle — app root lands at
# /mnt/us/kindlichess, scriptlet/kindlichess.sh at /mnt/us/documents/.
yes | sudo cp -r ./kindlichess /mnt/tmp/

echo "Deploying scriptlet to /mnt/us/documents (SH_Integration auto-registers it)"
yes | sudo cp ./scriptlet/kindlichess.sh /mnt/tmp/documents/

echo "Moving token back.."
mv ./token.json ./kindlichess/secrets/

sudo umount /mnt/tmp

echo "Done flashing!"
