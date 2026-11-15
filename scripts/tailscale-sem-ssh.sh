#!/bin/bash
# Desliga o Tailscale SSH neste PC. Com ele ligado, o Tailscale atende a porta 22 de quem chega pela
# tailnet antes do sshd e recusa a chave do C-hub Android. A rede do Tailscale continua igual.
# Uso: sudo bash scripts/tailscale-sem-ssh.sh    Reverter: sudo tailscale set --ssh=true
set -euo pipefail
[[ $EUID -eq 0 ]] || { echo "rode com sudo: sudo bash $0"; exit 1; }
tailscale set --ssh=false
tailscale debug prefs | grep RunSSH
