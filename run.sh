#!/usr/bin/env bash
# Install adguard-cli if missing
if ! command -v adguard-cli &>/dev/null; then
  echo "adguard-cli not found, installing via AUR..."
  if command -v paru &>/dev/null; then
    paru -S --noconfirm --needed adguard-cli-bin
  elif command -v yay &>/dev/null; then
    yay -S --noconfirm --needed adguard-cli-bin
  else
    echo "No AUR helper found. Please install adguard-cli-bin manually."
    exit 1
  fi
fi
exec /home/al/.local/bin/adguard-cli-gui "$@"
