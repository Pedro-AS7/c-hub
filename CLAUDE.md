# Chube — v0.1.0

Chube é um app desktop para o COSMIC (Pop!_OS 24.04, Wayland) que **gerencia sessões do Claude Code**: lista de chats ativos, cada um com o `claude` interativo num terminal integrado, sessões salvas para retomar, sub-chats (fork de sessão), painel de consumo do plano e **terminal integrado com abas**. Usa o plano Pro da conta logada no `claude` — nada de API key.

Rascunho da interface: `chub_racunho.png` (na raiz).

Projeto em português (Brasil). Comentários e UI em PT-BR; identificadores de código em inglês.

## Stack

- **Rust** (edition 2024) + **libcosmic** (git, fixado no commit `03d7dcb` de 2026-09-24 em `Cargo.toml`). O libcosmic usa um fork próprio do iced (`pop-os/iced`).
- Features do libcosmic: `winit`, `tokio`, `wayland`, `wgpu`, `multi-window`, `dbus-config`, `xdg-portal`.
  **Não** ligue `a11y`: o cosmic-term avisa que ela derruba o seletor de arquivos (usado em "Escolher pasta…").
- **Claude Code** (CLI `claude`, testado na 2.1.283): cada chat é o `claude` interativo (a TUI) rodando num PTY.
- **Terminal**: `alacritty_terminal` 0.25 + `cosmic-text` 0.19, código portado do **cosmic-term**. `cosmic-text` precisa ser a
  mesma versão que o libcosmic usa (o terminal escreve no `font_system()` dele).
- **Licença GPL-3.0-only**, por causa do código do cosmic-term em `src/term/`.

## Comandos

- `cargo run` — roda em modo debug (a primeira compilação baixa e compila o libcosmic: alguns minutos).
- `cargo build --release` — binário otimizado em `target/release/chube`.
- `cargo test` — testes do leitor do `/usage` (`src/usage.rs`), do comando, do título e do modo em `src/claude.rs`,
  das sugestões de `/comandos` (`src/commands.rs`) e os que vieram do cosmic-term em `src/term/terminal_box.rs`.
- Opções: `chube [--model <modelo>] [--prompt <mensagem>]`. `--model` vale para os chats novos; `--prompt` é passado ao `claude`
  do primeiro chat, que já começa respondendo (útil para testar sem clicar).

> Testes que falam com o modelo gastam o limite do plano Pro: use `--model haiku` e prompts curtos.

## Estrutura

```
src/
  main.rs       Argumentos (--model, --prompt) e Settings da janela
                (também limpa as variáveis de sessão herdadas de um Claude Code pai)
  app.rs        App (cosmic::Application): layout em pane_grid (painel | chat | terminal), update/view,
                diálogo de nova sessão, painel lateral de sessões salvas, painel "Uso", leitura periódica
                do status dos chats (Message::Poll)
  chat.rs       Chat (terminal com o `claude` + campo de mensagem, status/título/contexto/modo), Status;
                new / resume / fork / reopen
  claude.rs     Comando do `claude` interativo, caminho do binário, título vindo do terminal, Mode
  commands.rs   Comandos `/…` sugeridos no campo de mensagem (do Claude Code, do usuário e do projeto)
  sessions.rs   Sessões salvas (~/.claude/projects), status de um `claude` pelo pid (~/.claude/sessions)
                e contexto/modelo pelo fim do .jsonl
  dirs.rs       Resolve o diretório digitado: ".", "~", caminhos, atalhos do fshort (~/.shortcuts)
  usage.rs      Limites do plano via `/usage`
  term/         Terminal portado do cosmic-term (mod.rs substitui config, menu e atalhos de lá)
```

## Integração com o Claude Code (`src/claude.rs`)

Cada chat é um terminal (o mesmo widget do painel da direita) rodando:

```
env -u ANTHROPIC_API_KEY claude (--session-id <uuid> | --resume <id> | --resume <id> --fork-session)
       [--model <m>] [-- <prompt>]
```

- A conversa, as permissões, os comandos `/…` etc. são da própria TUI do Claude Code; o app só escreve no PTY,
  como um teclado (campo de mensagem, seletor de modo).
- Todos os chats ficam vivos ao mesmo tempo; a lista "Chats ativos" só troca qual terminal aparece na seção central.
- `ANTHROPIC_API_KEY` sai do ambiente pelo `env -u`: com ela, o Claude Code cobraria pela API em vez do plano.
  O `env` faz `exec`, então o pid do PTY (`Terminal::pid`) é o do `claude`.
- `main.rs` apaga as variáveis que um Claude Code põe nos filhos (`CLAUDECODE`, `CLAUDE_CODE_CHILD_SESSION`…).
  Herdadas (Chube aberto de dentro de uma sessão, ex.: `cargo run` pelo Claude), o `claude` dos chats se acha
  aninhado e **não grava a sessão** ("Transcript saving is off"): sem `.jsonl`, sem `~/.claude/sessions/<pid>.json`.
- O `claude` é procurado no PATH e, se não houver, em `~/.local/bin/claude` (o launcher não põe `~/.local/bin` no PATH).
- Fechar o app fecha os PTYs, e o `claude` recebe SIGHUP.
- Quando o `claude` sai (`/exit`), o chat fica "encerrado" com a última saída na tela e o botão "Reabrir sessão"
  (`--resume` do mesmo id, no mesmo lugar da lista).
- O × do cabeçalho e o × de cada item da lista "Chats ativos" fecham o chat (descarta o `Terminal`: SIGHUP no `claude`). Sem nenhum chat, a seção central mostra
  "Nova sessão" / "Sessões salvas"; o código usa `self.chats.get(self.focused)`, nunca indexação direta.

### Campo de mensagem (abaixo do terminal do chat)
- `text_editor` do libcosmic que cresce com o texto (até `COMPOSER_MAX_HEIGHT`, depois rola). É ele que recebe o foco
  ao trocar de chat; clicar no terminal do chat continua permitindo digitar direto na TUI.
- Enter envia: o texto vai por bracketed paste (`Terminal::paste`) e o `\r` segue `SUBMIT_DELAY` depois — junto da
  colagem, a TUI trataria o Enter como texto. Assim `/model`, `/btw` etc. rodam como comandos (testado na 2.1.283).
- Teclas repassadas à TUI: Esc (interromper), Shift+Tab (modo), e, com o campo vazio, Enter e ↑/↓ (escolher opção
  num pedido de permissão). Shift+Enter quebra a linha; Tab completa o `/comando` sugerido.
- Sugestões: com o texto `/prefixo`, `commands::suggestions`; a lista (`commands::load`) é lida na primeira `/`.

### Modo de permissão
- Lido na tela (`term::screen_tail`, últimas 4 linhas): `manual mode on`, `accept edits on`, `plan mode on`,
  `auto mode on`, `bypass permissions on` (`Mode::from_screen`). Sem o rodapé na tela (pedido de permissão aberto),
  mantém o último lido. Relido a cada `Message::Poll`.
- Trocar no seletor manda um Shift+Tab (`\x1b[Z`) por vez e relê a tela (`MODE_STEP_DELAY`) até chegar ao pedido;
  desiste ao voltar ao modo inicial (o pedido não está no ciclo, ex.: "Sem permissões") ou após `MODE_MAX_STEPS`.

### Status, título e contexto
- A cada 1 s (`Message::Poll`, em `spawn_blocking`), para cada chat vivo: `~/.claude/sessions/<pid>.json` →
  `sessionId` e `status` (`busy` → trabalhando, `waiting` → precisa de você, `idle` → pronto). O `sessionId` muda
  com `/clear` e é assim que um sub-chat descobre o id novo.
- Só do chat em foco: os últimos 512 KB do `.jsonl` da sessão → tokens de contexto e modelo da última resposta.
  O tamanho da janela não é gravado: acima de 200 mil tokens, o painel assume 1 milhão.
- Título: o Claude Code define o título do terminal como `✳ Assunto` (spinner no lugar do `✳` enquanto trabalha);
  `claude::title_from_terminal` tira o símbolo e ignora `Claude Code`.

### Consumo do plano
- `claude -p --no-session-persistence --output-format json "/usage"` roda o `/usage` **localmente**: 0 tokens, ~2,5 s,
  não cria sessão salva. O texto (`Current session: 34% used · resets Sep 27, 12:50am (…)`) é lido em `usage.rs`.
  O formato não é oficial: se mudar, o painel mostra "Erro ao atualizar" — ajuste `parse_line` e os testes.

### Arquivos do Claude Code lidos pelo app
- `~/.claude/projects/<dir-codificado>/<session-id>.jsonl` — **todas** as sessões ficam aqui, de qualquer diretório.
  Linhas usadas: `cwd`, `custom-title` > `ai-title` > `last-prompt` (título na lista de sessões salvas) e a última
  `assistant` fora de `isSidechain` (`message.usage` e `message.model`). O Claude Code apaga sessões com mais de 30 dias (`cleanupPeriodDays`).
- `~/.claude/sessions/<pid>.json` — sessões abertas agora (`sessionId`, `status`, `cwd`); conferir `/proc/<pid>`.
  **Não leia os `.key`** do mesmo diretório (têm tokens).

## Terminal (`src/term/`)

`terminal.rs`, `terminal_box.rs` e `mouse_reporter.rs` vêm do cosmic-term (commit `9129277`, 2026-09-23) com poucas
mudanças, marcadas no topo de cada arquivo:
- eventos identificados por `TabId` em vez de `(pane, aba)`; `TerminalPaneGrid` removido; `Terminal::pid()`;
- copiar/colar (Ctrl+Shift+C/V) tratado dentro do widget (no cosmic-term é atalho do app);
- foco: o clique dá foco também no modo de mouse (a TUI do `claude` liga esse modo), clicar fora tira o foco
  (antes, o terminal do chat e o do painel ficavam os dois com foco e recebiam o mesmo texto) e o foco volta
  quando a janela recupera o dela (antes era preciso trocar de chat);
- `terminal_theme.rs` só com as paletas COSMIC Dark/Light.

`term/mod.rs` substitui o que esses arquivos usavam do cosmic-term (`Config` com os valores padrão de lá, `MenuState`,
`Action` sem variantes) e tem `spawn()`. Os eventos do PTY chegam por um canal único (`Message::TermEventTx` /
`TermEvent`), tratados em `App::handle_term_event` como no `Message::TermEvent` do cosmic-term.
Para trazer novidades do upstream, compare com o cosmic-term e reaplique só essas mudanças.

## Convenções

- Comentários em PT-BR, curtos, explicando o porquê.
- Widgets com `widget::Column::new()` / `widget::Row::new()` (os helpers `widget::column()`/`row()` do iced exigem argumento),
  espaçamentos de `theme::spacing()`, seções dentro de `card()` (`theme::Container::Card`).
- Funções async na UI via `cosmic::task::future(async { Message::… })`; trabalho de disco em `tokio::task::spawn_blocking`.
- `Message` precisa ser `Clone + Debug`: o `Terminal` fica no `Chat` (dentro de `Mutex`), nunca numa mensagem.

## Verificar a UI

O Claude **não consegue clicar nem digitar na janela** (o app não aparece no AT-SPI). Para validar:
- captura de tela: `cosmic-screenshot --interactive=false --modal=false --notify=false -s <pasta>`;
- fluxos de chat: abrir com `--model haiku --prompt "…"` e capturar a tela; status e sessão do `claude` do chat em
  `~/.claude/sessions/<pid>.json` (`pgrep -af "claude --session-id"`);
- para fechar o app: `pkill -x chube` (**não** `pkill -f …chube`, que casa com o próprio shell do comando).
  Se o Claude estiver rodando **dentro** de um Chube, abra uma segunda instância e feche só ela pelo pid;
- lógica fora da UI (parsers, resolução de diretório): testes em Rust;
- cliques e digitação (troca de chat, painel de sessões, diálogo de nova sessão, sub-chat, "Reabrir sessão",
  terminais, campo de mensagem, seletor de modo): pedir para conferir manualmente;
- bytes que a TUI entende: script Python com `pty.fork()` rodando o `claude` (comandos locais como `/help` não gastam tokens).

## Status do projeto

Versão 0.1.0. O roteiro inicial de 5 passos está completo (2026-09-27): esqueleto, chat real, sessões salvas/retomar/
sub-chat/permissões, painel de consumo e terminal integrado. Em seguida (2026-09-27), o chat em stream-json com
markdown e cartão de permissão foi trocado pelo `claude` interativo num terminal por chat. Repositório git criado (branch `main`), **ainda sem commits**.
Não está instalado (sem binário em `~/.local/bin` nem `.desktop`).

Validado por captura de tela: o `claude` do chat abre com `--prompt`, responde, e status, modelo e contexto aparecem.
Ainda não testado com clique: troca entre chats, painel de sessões, diálogo de nova sessão, sub-chat, "Reabrir sessão",
digitação nos terminais, fechar chat.

Limitações conhecidas:
- o status só vem de `~/.claude/sessions`, um arquivo interno do Claude Code (formato pode mudar);
- o terminal não tem menu de contexto, zoom, busca nem divisão em painéis (o cosmic-term tem);
- a leitura do `/usage` depende do texto do Claude Code.
