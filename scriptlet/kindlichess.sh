#!/bin/sh
# Name: Kindli-Chess
# Author: mxyyz
# Icon: /mnt/us/kindlichess/assets/chess-king.png
#
# Dedicated scriptlet launcher (kindlemodding.org/kindle-dev/scriptlets.html):
# the jailbreak's SH_Integration auto-registers any .sh in /mnt/us/documents
# as a launchable library item — this file is deployed there by flash.sh /
# flash_usb.sh from the repo-root scriptlet/ directory. It only delegates to
# the real launcher; chess_app.sh itself stays the KUAL menu action
# (menu.json), so KUAL remains the fallback launch path.
exec /mnt/us/kindlichess/chess_app.sh --scriptlet
