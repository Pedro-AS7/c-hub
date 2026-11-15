#!/bin/bash
# Liga o sshd deste PC para o C-hub Android entrar nos chats: login só por chave, só o seu usuário,
# sem root. Com o ufw, a porta 22 abre apenas na interface do Tailscale (tailscale0).
# Uso: sudo bash scripts/sshd-tailscale.sh
# Reverter: sudo rm /etc/ssh/sshd_config.d/50-chub.conf
#           sudo ufw delete allow in on tailscale0 to any port 22 proto tcp   (se usou o ufw)
#           sudo systemctl disable --now sshd   (ou "ssh", no Debian/Ubuntu)
set -euo pipefail
[[ $EUID -eq 0 ]] || { echo "rode com sudo: sudo bash $0"; exit 1; }
user=${SUDO_USER:-}
[[ -n $user && $user != root ]] || { echo "rode com sudo a partir do seu usuário (não como root direto)"; exit 1; }
command -v sshd >/dev/null || { echo "instale o OpenSSH (Arch: pacman -S openssh; Debian/Ubuntu: apt install openssh-server)"; exit 1; }

mkdir -p /etc/ssh/sshd_config.d
grep -qE '^\s*Include\s+/etc/ssh/sshd_config.d/' /etc/ssh/sshd_config ||
    echo "aviso: o /etc/ssh/sshd_config não inclui o sshd_config.d; acrescente 'Include /etc/ssh/sshd_config.d/*.conf' no começo dele"

cat > /etc/ssh/sshd_config.d/50-chub.conf <<CONF
# C-hub: só chave, só o $user, sem root.
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
AuthenticationMethods publickey
AllowUsers $user
X11Forwarding no
CONF

# Primeira vez do sshd nesta máquina: gera as chaves de host que faltam (não troca as existentes).
ssh-keygen -A
mkdir -p /run/sshd  # o Debian/Ubuntu exige para o teste abaixo
sshd -t

if command -v ufw >/dev/null && ufw status | grep -q 'Status: active'; then
    ufw allow in on tailscale0 to any port 22 proto tcp comment 'C-hub (ssh pelo Tailscale)'
else
    echo "aviso: ufw não está ativo; a porta 22 fica aberta em todas as redes (só com chave)."
    echo "       Para limitar ao Tailscale, use o firewall da sua distribuição na interface tailscale0."
fi

if systemctl cat sshd.service >/dev/null 2>&1; then service=sshd; else service=ssh; fi
systemctl enable --now "$service"
echo; systemctl is-active "$service"
echo; echo "Impressões digitais deste PC (confira no celular na primeira conexão):"
for f in /etc/ssh/ssh_host_*_key.pub; do ssh-keygen -lf "$f"; done
