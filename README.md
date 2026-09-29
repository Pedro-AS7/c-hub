# Chube

Gerenciador de sessões do [Claude Code](https://claude.com/claude-code) para o desktop COSMIC (Pop!_OS 24.04, Wayland),
feito em Rust com o [libcosmic](https://github.com/pop-os/libcosmic).

![Rascunho da interface](chub_racunho.png)

- Lista de chats ativos, cada um com o `claude` interativo num terminal integrado
- Sessões salvas para retomar, e sub-chats (um chat novo com o contexto do chat em foco)
- Painel de consumo do plano (limites de 5 h e da semana) e do contexto usado no chat em foco
- Terminal integrado com abas, ao lado do chat
- Temas: sistema, fósforo, neon e gruvbox

Usa a conta já logada no `claude`, sem API key.

> Projeto pessoal, sem ligação com a Anthropic.

## Requisitos

- Rust (edition 2024) e as dependências de compilação do libcosmic
- [Claude Code](https://docs.claude.com/en/docs/claude-code) instalado e logado (`claude` no `PATH` ou em `~/.local/bin`)
- Sessão Wayland. Feito para o COSMIC; também roda no GNOME, seguindo o tema do COSMIC

## Instalar

```sh
./install.sh             # compila em release e instala para o usuário (sem sudo)
./install.sh --uninstall # remove
```

Instala o binário em `~/.local/bin/chube`, o ícone e o atalho `.desktop` no launcher.
Para só rodar: `cargo run --release`. Opção: `chube --model <modelo>` define o modelo dos chats novos.

## Licença

[GPL-3.0-only](LICENSE). O terminal em `src/term/` é adaptado do [cosmic-term](https://github.com/pop-os/cosmic-term)
(© System76, GPL-3.0-only).
