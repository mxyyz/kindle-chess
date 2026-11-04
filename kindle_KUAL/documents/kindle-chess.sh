#!/bin/sh
# Name: Kindli-Chess
# Author: mxyyz
# Icon: /mnt/us/hellokindle/assets/chess-king.png
#
# Scriptlet entry point. The Winterbreak Universal Hotfix (SH_Integration)
# auto-registers any .sh in /mnt/us/documents as a launchable library item —
# no appreg.db editing needed (those com.notmarek.shell_integration rows are
# the hotfix's own; appreg.db is Amazon's native registry). This file just
# delegates to the real launcher. KUAL menu entry stays as a fallback.
exec /mnt/us/hellokindle/chess_app.sh --scriptlet
