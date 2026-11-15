# Configuração do C-hub

Este guia monta o sistema completo. Só a parte 1 é obrigatória: o desktop funciona sozinho, com chats locais. As partes 2
e 3 acrescentam um servidor e o celular.

1. [Desktop](#1-desktop): o C-hub no seu PC Linux
2. [Servidor (VPS)](#2-servidor-vps): sessões rodando num servidor por SSH
3. [Celular](#3-celular): o app Android entrando nas sessões do PC e do servidor
4. [Problemas comuns](#4-problemas-comuns)
5. [Desfazer](#5-desfazer)

Nos exemplos, `minha-vps` é o nome do servidor no `~/.ssh/config`, `voce` é o seu usuário e `100.x.y.z` é um IP do
Tailscale. Troque pelos seus.

---

## 1. Desktop

### Requisitos

| O quê | Versão | Para quê |
|---|---|---|
| Rust | 1.85 ou mais nova (edition 2024) | compilar |
| GTK 4 | 4.18 ou mais nova | interface |
| VTE para GTK 4 | 0.78 ou mais nova | terminais |
| tmux | qualquer recente | os chats locais rodam nele |
| OpenSSH (cliente) | | sessões no servidor |
| `wl-clipboard` | opcional | copiar com `/copy` de dentro de um tmux remoto (Wayland) |
| Claude Code e/ou Codex | instalados e logados | os agentes |

Pacotes por distribuição. Só o Arch/Omarchy foi testado; nas outras, os nomes são os dos repositórios oficiais e as versões
exigidas existem a partir das versões indicadas.

```sh
# Arch / Omarchy
sudo pacman -S --needed rust gtk4 vte4 tmux openssh wl-clipboard

# Debian 13 / Ubuntu 25.04 ou mais novos (o Rust pelo rustup: https://rustup.rs)
sudo apt install build-essential pkg-config libgtk-4-dev libvte-2.91-gtk4-dev tmux openssh-client wl-clipboard

# Fedora 42 ou mais novo (o Rust pelo rustup ou: sudo dnf install rust cargo)
sudo dnf install gcc pkgconf-pkg-config gtk4-devel vte291-gtk4-devel tmux openssh-clients wl-clipboard
```

Os agentes (siga a documentação oficial de cada um):

```sh
curl -fsSL https://claude.ai/install.sh | bash   # Claude Code; depois rode "claude" uma vez e faça o login
npm install -g @openai/codex                      # Codex; depois rode "codex" uma vez e faça o login
```

O C-hub procura o `claude` no `PATH` e em `~/.local/bin`, e o `codex` no `PATH`, em `~/.local/bin` e nas versões do Node
instaladas pelo nvm.

### Instalar

```sh
git clone https://github.com/Pedro-AS7/c-hub.git
cd c-hub
./install.sh
```

O `install.sh` compila em release e instala só para o seu usuário, sem sudo:

- `~/.local/bin/chube` (o binário)
- `~/.local/share/applications/com.pedro.Chube*.desktop` (o atalho no launcher e as entradas de "Abrir com")
- `~/.local/share/icons/hicolor/scalable/apps/com.pedro.Chube.svg` (o ícone)

Para atualizar: `git pull && ./install.sh`. Para só testar sem instalar: `cargo run --release`.

### Primeiro uso

Abra **C-hub** no launcher (ou rode `chube`). Em **nova sessão** (Ctrl+Shift+N), escolha o agente (`claude` ou `codex`;
`omarchy` só funciona no Omarchy), o lugar (`local` ou um servidor) e a pasta. Pastas aceitam `~/…`, caminhos absolutos e
atalhos de pasta (`nome` ou `nome/sub`).

Sem o Omarchy, o C-hub usa o tema Tokyo Night. Os outros temas e o editor de cores ficam no botão de paleta da lista de chats.

Os chats locais rodam em `tmux -L chube`, um servidor de tmux separado do seu e sem a sua configuração. Para entrar num
deles de um terminal: `tmux -L chube ls` e `tmux -L chube attach -t chube-…`.

---

## 2. Servidor (VPS)

O C-hub abre as sessões do servidor com `ssh -t minha-vps`, cada uma num tmux de lá. Fechar o chat só desconecta.

### No servidor

1. Instale o `tmux` e o `bash`. Ex.: `sudo apt install tmux`.
2. Instale e faça o login do Claude Code e/ou do Codex **com o seu usuário de lá**, como no desktop. Para o Claude Code,
   rode `claude` uma vez pelo SSH e siga o login.
3. Confira se o agente é encontrado num login sem terminal (é assim que o C-hub roda):

   ```sh
   ssh minha-vps 'bash -lc "command -v claude; command -v codex"'
   ```

   O C-hub já acrescenta `~/.local/bin` ao `PATH`. Se o agente estiver em outro lugar (nvm, por exemplo), acerte o `PATH`
   no `~/.profile` ou no `~/.bash_profile` de lá.

### No PC

1. Crie uma chave SSH, se ainda não tiver, e autorize-a no servidor:

   ```sh
   ssh-keygen -t ed25519 -C "meu-pc"
   ssh-copy-id -i ~/.ssh/id_ed25519.pub voce@endereco-da-vps
   ```

2. Dê um nome ao servidor no `~/.ssh/config`. O C-hub lista os `Host` desse arquivo (os nomes exatos, sem `*` nem `?`):

   ```
   Host minha-vps
       HostName 100.x.y.z        # IP do Tailscale ou o endereço público
       User voce
       IdentityFile ~/.ssh/id_ed25519
   ```

3. Teste: `ssh minha-vps tmux -V`.

4. **Chave com senha:** além dos chats, o C-hub faz consultas sem terminal (`BatchMode=yes`) para a lista e o estado das
   sessões, e elas não têm onde pedir a senha. Deixe a chave no `ssh-agent` (`ssh-add ~/.ssh/id_ed25519`) ou use
   `AddKeysToAgent yes` no `~/.ssh/config`. Opcional: com `AddKeysToAgent confirm` e um `SSH_ASKPASS` gráfico, cada conexão
   pede a sua confirmação numa janela.

O C-hub não reaproveita conexões (`ControlMaster=no`, mesmo que o seu `~/.ssh/config` tenha um mestre): cada uma autentica
de novo.

### Usar

Em **nova sessão → onde**, escolha `minha-vps`. O painel de sessões (botão de histórico) tem uma aba por servidor, com os
tmux abertos lá e as sessões salvas do Claude Code de lá.

---

## 3. Celular

O app Android não roda agentes: ele entra por SSH nos tmux do PC (os chats do C-hub desktop) e dos servidores, e cria
sessões novas neles. Requer Android 12 ou mais novo e um bloqueio de tela (a chave SSH só é liberada pela biometria ou
pelo PIN).

### 3.1 Rede: Tailscale

O celular precisa alcançar o PC pela porta 22. O jeito mais simples e seguro é o [Tailscale](https://tailscale.com),
que cria uma rede privada entre os seus aparelhos sem abrir portas no roteador.

1. Instale o Tailscale no PC, no celular e, se quiser, no servidor, todos na mesma conta.
2. Anote o IP do Tailscale do PC: `tailscale ip -4` (um `100.x.y.z`).

Outra VPN ou a rede local também servem: o app só precisa de um endereço e uma porta.

### 3.2 PC: ligar o sshd

O script abaixo liga o servidor SSH do PC aceitando **só chave**, **só o seu usuário** e **sem root**. Se o `ufw`
estiver ativo, ele abre a porta 22 apenas na interface do Tailscale. No fim, mostra as impressões digitais do PC: guarde-as
para conferir no celular.

```sh
sudo bash scripts/sshd-tailscale.sh
```

Ele grava `/etc/ssh/sshd_config.d/50-chub.conf`. O `AllowUsers` dele vale para o sshd inteiro: se outra pessoa entra
nesse PC por SSH, acrescente o usuário dela no arquivo.

Se o **Tailscale SSH** estiver ligado no PC (`tailscale up --ssh`), ele atende a porta 22 de quem chega pela tailnet antes
do sshd, e o app não consegue entrar. Desligue-o:

```sh
sudo bash scripts/tailscale-sem-ssh.sh
```

### 3.3 Instalar o app

Não há APK publicado. Compile o seu:

1. Instale um JDK 21 e o Android SDK com `platforms;android-37.0` e `build-tools;37.0.0`, por exemplo com o
   [Android Studio](https://developer.android.com/studio) ou com o `sdkmanager` das command-line tools.
2. Aponte o SDK em `android/local.properties` (`sdk.dir=/caminho/do/Android/Sdk`) ou na variável `ANDROID_HOME`.
3. Compile:

   ```sh
   cd android
   ./gradlew :app:assembleDebug
   ```

4. No celular, ligue a **depuração USB** (Configurações → Sobre o telefone → toque 7 vezes em "Número da versão" →
   Opções do desenvolvedor). Conecte-o pelo USB e instale:

   ```sh
   adb install -r app/build/outputs/apk/debug/app-debug.apk
   ```

Quem tem acesso ao GitHub também acha o APK de depuração nos artefatos da última execução da CI (aba Actions).

### 3.4 Chave SSH do celular

O app usa uma chave sua, importada uma vez e guardada cifrada no Keystore do Android. O jeito mais simples é gerá-la no
[Termux](https://f-droid.org/packages/com.termux/):

```sh
pkg install openssh
ssh-keygen -t ed25519 -C "celular"
```

No app, **ajustes → chave ssh**, e então uma destas:

- **Colar:** no Termux, `cat ~/.ssh/id_ed25519`, copie tudo (de `-----BEGIN` a `-----END`) e cole no campo.
- **Arquivo:** no Termux, `termux-setup-storage` e `cp ~/.ssh/id_ed25519 ~/storage/downloads/`; no app, "escolher
  arquivo…". **Depois apague a cópia** dos Downloads.

Se a chave tiver senha, digite-a no campo de senha. Depois de importar, cada conexão pede a digital (ou o PIN).

Chaves ed25519, ECDSA e RSA no formato do OpenSSH funcionam. Também dá para gerar a chave no PC, mas aí a chave privada
passa pelo PC e pelo cabo: prefira gerar no celular.

### 3.5 Autorizar a chave no PC e no servidor

No app, **ajustes → copiar pública**. Depois, no PC:

```sh
scripts/autorizar-celular.sh "ssh-ed25519 AAAA… celular"
```

Com o celular no USB e o app de depuração instalado, `scripts/autorizar-celular.sh` sem argumentos lê a chave do app pelo
`adb`.

No servidor, acrescente a mesma linha ao `~/.ssh/authorized_keys` de lá:

```sh
ssh minha-vps 'cat >> ~/.ssh/authorized_keys' <<< "ssh-ed25519 AAAA… celular"
```

### 3.6 Cadastrar os hosts no app

Em **ajustes → hosts**:

- **+ PC com C-hub:** o PC do desktop. Endereço = o IP do Tailscale dele; usuário = o seu usuário no PC. O campo
  "servidor do tmux" já vem com `chube`, o tmux dos chats do desktop. Pasta padrão = onde um chat novo começa quando o
  campo de pasta fica vazio. Marque "tem o agente do Omarchy" se o PC for um Omarchy.
- **+ servidor:** cada VPS. Deixe o "servidor do tmux" vazio (o tmux padrão de lá, onde o desktop cria as sessões).

### 3.7 Primeira conexão

Na lista, toque em atualizar. Na primeira conexão a cada host, o app mostra a impressão digital da chave dele. Compare com
a do host antes de aceitar:

```sh
ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
```

Se uma chave mudar depois, o app avisa em vermelho. Recuse, a não ser que você saiba por que ela mudou (reinstalou o
servidor, por exemplo).

### Usar

- A lista mostra as sessões de cada host com o status. Toque numa para entrar; **nova sessão** cria uma.
- A barra acima do teclado tem as teclas que faltam no celular: Ctrl e Alt (valem para a próxima tecla), Esc, Shift+Tab
  (troca o modo do Claude), Tab, ^C, setas, Enter e colar.
- Um tmux tem um tamanho só. Por isso o celular só fica conectado com o chat na tela: ao voltar para a lista ou sair do
  app, ele solta o terminal e o desktop volta à largura dele.
- **ajustes → tema → usar o do desktop** copia o tema do C-hub desktop. Os atalhos de pasta de cada host são lidos do
  próprio host (os mesmos do desktop).

---

## 4. Problemas comuns

**O celular diz "Permission denied (publickey)".**
A chave pública do app não está no `~/.ssh/authorized_keys` desse host (3.5), o usuário no cadastro do host está errado, ou
o `AllowUsers` do `50-chub.conf` não tem esse usuário. Também acontece se o Tailscale SSH estiver ligado (3.2). No host,
`journalctl -u sshd -n 30` (ou `-u ssh`) mostra o motivo.

**"sem resposta" ou "conectando…" sem fim.**
O Tailscale está desligado no celular ou no host, ou o firewall bloqueia a porta 22. Teste do Termux:
`ssh voce@100.x.y.z`.

**O desktop diz "sem acesso a minha-vps: a chave não está no agente…".**
As consultas de fundo não pedem senha. Rode `ssh-add` com a chave do servidor ou abra um chat lá primeiro: o terminal do
chat pede a senha.

**O chat do servidor fecha logo ao abrir.**
O agente não foi encontrado no login de lá. Teste com `ssh minha-vps 'bash -lc "command -v claude"'` (parte 2).

**Os símbolos do Claude aparecem como `_` no tmux.**
O tmux de lá não está em UTF-8. O C-hub e o app já forçam o UTF-8 nas sessões que criam; para um tmux aberto à mão, use
`tmux -u`.

**A compilação do desktop reclama de `gtk4` ou de `vte`.**
A sua distribuição tem GTK abaixo de 4.18 ou VTE abaixo de 0.78. Veja os requisitos na parte 1.

---

## 5. Desfazer

- Desktop: `./install.sh --uninstall`. As configurações ficam em `~/.config/chube/` e a lista de chats em
  `~/.local/state/chube/`; apague-as se quiser.
- sshd do PC (3.2): `sudo rm /etc/ssh/sshd_config.d/50-chub.conf`, `sudo ufw delete allow in on tailscale0 to any port 22
  proto tcp` (se usou o ufw) e `sudo systemctl disable --now sshd` (ou `ssh`). O Tailscale SSH volta com
  `sudo tailscale set --ssh=true`.
- Chave do celular: apague a linha dela no `~/.ssh/authorized_keys` do PC e dos servidores, e "apagar" em ajustes no app.
- Sessões que ficaram abertas: `tmux -L chube kill-server` no PC; no servidor, `tmux ls` e `tmux kill-session -t chube-…`.
