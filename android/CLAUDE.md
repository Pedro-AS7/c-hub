# C-hub Android — v0.1.0

Versão para celular do C-hub (o desktop está na raiz do repositório). **Sem sessão local**: só entra nas sessões do tmux
do PC com o C-hub desktop e dos servidores por SSH (de preferência pelo Tailscale) e cria sessões novas neles. Kotlin +
Jetpack Compose (nativo), terminal do Termux. Interface, comentários e logs em português; identificadores em inglês.
Criado em 2026-11-14 como repositório à parte; está em `android/` desde 2026-11-15. Configuração para quem instala:
`docs/configuracao.md` (parte 3).

**Regra (2026-11-15):** toda mudança do C-hub desktop que fizer sentido no celular é trazida para cá na mesma tarefa, sem
precisar pedir (detalhes no `CLAUDE.md` da raiz). O inverso também: o que mudar aqui e tocar no formato
compartilhado (scripts do tmux, `config.json`) tem que continuar compatível com o desktop.

## Como funciona

- Cada chat é uma sessão do tmux no host, com os **mesmos nomes e opções do desktop** (`chube-<8 hex>`, sem barra, mouse,
  título do agente repassado, sem prefixo). O celular abre um canal SSH com pty que roda `tmux attach` (ou `new-session -A`
  rodando o agente). Fechar o terminal (×) só desconecta; o botão de parar mata o tmux.
- **PC com o C-hub desktop**: os chats locais do desktop rodam desde 2026-11-14 num servidor tmux próprio,
  `tmux -L chube -f /dev/null`. O host do PC no app usa o mesmo `-L` (`Host.tmuxSocket` = `Host.DESKTOP_TMUX`).
- **Hosts**: cadastrados no app (ajustes → hosts, prefs `lista`); não há host de fábrica. "+ PC com C-hub" já põe o `-L chube`.
- **Estado**: uma conexão SSH por host roda um laço (`Scripts.poll`) que manda a cada 3 s uma linha por sessão do tmux
  (campos separados por `\x1f`: nome, conectado, pasta, título, atividade, `sessionId`/`status` do
  `~/.claude/sessions/<pid>.json`, modelo e `usage` do `.jsonl`) e um `E`. "terminou" = pronto depois de trabalhando, fora da tela.
- Todos os canais de um host (estado, terminais, comandos) passam por **uma conexão** = uma biometria.
- `LANG` é forçado a `C.UTF-8` e o tmux roda com `-u`: a sessão do sshd não tem locale e o tmux trocaria os símbolos por `_`.

- **Tamanho**: um tmux só tem um tamanho, o do cliente que mexeu por último (`window-size latest`, padrão; testado: digitar
  ou rolar troca). Por isso o celular **só fica conectado com o chat na tela**: voltar à lista, abrir outro chat ou ir para
  segundo plano solta o terminal (`MainViewModel.release`/`releaseAll`, `onStop`) e o desktop volta à largura dele; a tela do
  chat reconecta ao voltar (o tmux redesenha; o histórico é o do tmux, pela rolagem). Com os dois na sessão, a sobra do maior
  fica em branco (`fill-character ' '`, com `-q` para tmux antigo).
- **Tailscale SSH desligado** no PC (`scripts/tailscale-sem-ssh.sh`): ele atende a porta 22 de quem vem pelo Tailscale
  antes do sshd e recusa a chave do app.

- **Tema** (2026-11-15): os mesmos temas do desktop (`ui/Palettes.kt`, ids iguais), o personalizado no mesmo JSON do
  `custom_theme` e o editor de cores (5 da interface + 16 do terminal; a opacidade do desktop não vale no celular). "usar o
  do desktop" lê o `config.json` do PC com o desktop (`theme`, `custom_theme`) ou, se ele segue o Omarchy, o `colors.toml` de lá.
  Escolha gravada no celular (prefs `tema`, `tema-personalizado`). `C` lê o tema de um estado do Compose: trocar redesenha.
- **Atalhos de pasta** (2026-11-15): os de cada host ficam **no host**, em `~/.config/chube/config.json` (`shortcuts`,
  caminho absoluto), o mesmo arquivo do desktop: os do PC são os do C-hub desktop, nos dois sentidos. `~/.shortcuts`
  (fshort) também tem editar e excluir (muda o arquivo no lugar, regra de nome do fshort; vale para o `go`). Gravar relê o arquivo e mantém as outras chaves (`Shortcuts.withShortcuts`, `.tmp` + `mv`). O campo
  de pasta aceita `nome` e `nome/sub`; pasta inexistente é recusada antes de criar (`cd` de teste), como no desktop.

## Chave SSH

- Importada do Termux (colar ou escolher o arquivo), conferida pelo sshj e gravada em `files/chave-ssh.bin` cifrada com
  AES-GCM por uma chave do Keystore (StrongBox se houver) que exige biometria/PIN **a cada uso** (`timeout 0`).
  Cada conexão pede a digital. Sem backup (`allowBackup=false`).
- Chaves de host: perguntadas na primeira conexão (impressão digital SHA256 como a do `ssh-keygen -l`) e gravadas
  (prefs `conhecidas-<id do host>`). Mudou = aviso em vermelho.

## Estrutura

```
terminal/   Emulador e view do Termux (v0.118.3, Apache 2.0, TERMUX-LICENSE.md), com o processo local (JNI) trocado por
            TerminalTransport (TerminalSession reescrito nessa parte)
app/src/main/java/com/pedro/chub/
  MainActivity.kt     Activity, BouncyCastle completo (ChubApp) e a biometria (BiometricPrompt + CryptoObject)
  MainViewModel.kt     Estado: hosts, conexões, laço do estado, terminais abertos, telas
  ChatTerminal.kt  TerminalSession de um chat (cliente do emulador)
  ssh/Scripts.kt  Scripts do shell (criar, entrar, matar, estado, sessões salvas) — iguais ao src/remote.rs do desktop
  ssh/Model.kt    Host, Status, RemoteChat, parsers
  ssh/Connection.kt  sshj: conexão, exec, stream e o transporte com pty
  ssh/KeyVault.kt    Chave no Keystore
  ui/             Compose (visual do desktop: Tokyo Night, JetBrainsMono Nerd Font, cantos retos) e o terminal com barra de teclas
```

## Comandos

- JDK 21 e o Android SDK (plataforma 37, build-tools 37.0.0), apontado em `local.properties` (`sdk.dir=…`, fora do git)
  ou em `ANDROID_HOME`.
- `./gradlew :app:assembleDebug` e `adb install -r app/build/outputs/apk/debug/app-debug.apk`
- `./gradlew :app:testDebugUnitTest` — scripts, aspas, parsers do estado e das sessões salvas.
- `minSdk` 31 (Android 12). Testado num Galaxy S23+ com Android 17, por adb no USB.

## No PC

- `scripts/sshd-tailscale.sh` (sudo): sshd só com chave, só o usuário de quem chamou, sem root; com o ufw, porta 22 só no
  `tailscale0`.
- `scripts/autorizar-celular.sh` (sem sudo, depois de importar a chave): põe a pública do app no `authorized_keys` (colada
  como argumento ou lida pelo adb do build de depuração).
- `scripts/tailscale-sem-ssh.sh` (sudo): desliga o Tailscale SSH.

## Testado

- Unidade: scripts, aspas, parsers do estado e das sessões salvas, atalhos de pasta, temas.
- No celular: importar a chave com biometria, conectar ao PC e a uma VPS, terminal e criar sessões.
- **Não testado no aparelho**: "usar o do desktop" (tema), editor de cores, salvar/editar/excluir atalhos de pasta.
