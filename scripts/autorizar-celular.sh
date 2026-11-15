#!/bin/bash
# Põe a chave pública do C-hub Android no ~/.ssh/authorized_keys deste PC (sem sudo).
# Uso:
#   scripts/autorizar-celular.sh                 lê a chave do app pelo adb (build de depuração, celular no USB)
#   scripts/autorizar-celular.sh "ssh-ed25519 …" usa a chave colada (no app: ajustes → copiar pública)
# Rode no PC e em cada servidor que o celular vai acessar.
set -euo pipefail
if [[ $# -gt 0 ]]; then
    key="$*"
else
    command -v adb >/dev/null || { echo "sem adb: passe a chave pública como argumento"; exit 1; }
    key=$(adb shell run-as com.pedro.chub cat shared_prefs/chave.xml | sed -n 's|.*name="publica">\([^<]*\)<.*|\1|p')
fi
[[ $key == ssh-* || $key == ecdsa-* ]] || { echo "chave pública não encontrada: importe a chave no app primeiro"; exit 1; }
echo "$key" | ssh-keygen -lf - >/dev/null || { echo "isso não parece uma chave pública"; exit 1; }
mkdir -p ~/.ssh && chmod 700 ~/.ssh
touch ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys
if grep -qF "$(echo "$key" | awk '{print $2}')" ~/.ssh/authorized_keys; then
    echo "já autorizada"
else
    echo "$key" >> ~/.ssh/authorized_keys
    echo "autorizada: $(echo "$key" | ssh-keygen -lf -)"
fi
