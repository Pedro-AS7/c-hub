# C-hub

Session manager for [Claude Code](https://claude.com/claude-code) and [Codex](https://github.com/openai/codex) on the Linux
desktop, with an Android app to follow and start sessions from your phone.

Each chat is an interactive `claude` or `codex` running inside **tmux**, either on your PC or on a server (VPS) over SSH.
Closing the app only disconnects: the agent keeps working, and you can join the same session from the desktop, the phone or
a terminal.

> Personal project, not affiliated with Anthropic or OpenAI. The UI and the rest of the documentation are in Brazilian
> Portuguese.

## Features

**Desktop** (Rust + GTK4 + VTE; tested on [Omarchy](https://omarchy.org), Arch + Hyprland)

- List of active chats grouped by location and folder, with each one's status: working, needs you, ready, finished
- Local chats in a dedicated tmux server (`tmux -L chube`): they survive closing the app, and the phone can join them
- **Sessions on a VPS** over SSH, each in its own tmux there; the sessions panel shows the open tmux sessions and the saved
  sessions on that server
- Saved sessions to resume, and sub-chats (a new chat carrying the context of the focused chat)
- Claude permission mode selector, plan usage panel and chat context usage
- Built-in terminal with tabs, next to the chat
- Themes: follows the Omarchy theme (colors, font, glass mode) or uses one of its own (Tokyo Night, Catppuccin, Gruvbox,
  Nord…), with a color editor
- Folder shortcuts in the new session panel (it also reads and edits `fshort`'s `~/.shortcuts`)

**Android** (Kotlin + Jetpack Compose, in [`android/`](android/))

- No local sessions: connects over SSH to the tmux sessions on the PC and servers, using the
  [Termux](https://github.com/termux/termux-app) terminal
- List with each chat's status; start new sessions, resume saved ones, end them
- SSH key stored in the Android Keystore, unlocked by biometrics on every connection
- The same themes and folder shortcuts as the desktop (read from the PC)

## Installation

The complete step-by-step guide, including the server and the phone, is in **[docs/configuracao.md](docs/configuracao.md)**
(in Portuguese).

Desktop quick start on Arch/Omarchy:

```sh
sudo pacman -S --needed rust gtk4 vte4 tmux openssh
git clone https://github.com/Pedro-AS7/c-hub.git && cd c-hub
./install.sh             # builds in release mode and installs for the current user (no sudo)
./install.sh --uninstall # removes it
```

This installs the binary to `~/.local/bin/chube`, plus the icon and the `.desktop` launcher entry. It requires `claude`
and/or `codex` to be installed and logged in. Other distros and requirements:
[docs/configuracao.md](docs/configuracao.md#1-desktop).

Options: `chube [--agent omarchy|claude|codex] [--model <model>] [--open-directory <folder>] [-- <folders>…]`. `--agent`
picks the agent for folders opened from the command line or the file manager; `--model` sets the model for that agent's
new chats.

## How it works

```
  PC running C-hub desktop               VPS
  tmux -L chube                          tmux
    chube-1a2b3c4d  (claude)               chube-9f8e7d6c  (claude)
    chube-5e6f7a8b  (codex)                chube-0a1b2c3d  (codex)
          ^                                      ^
          |  ssh (sshd, key-only, via Tailscale)  |  ssh
          +-------- phone (C-hub Android) --------+
```

- The desktop opens VPS chats with `ssh -t <host>` and local ones directly in the local tmux.
- The session name (`chube-<8 hex>`), the tmux options and the scripts are the same on the desktop (`src/remote.rs`) and
  on Android (`android/app/src/main/java/com/pedro/chub/ssh/Scripts.kt`): both see and create the same sessions.
- Status comes from Claude Code itself (`~/.claude/sessions/<pid>.json`) and from the terminal title; context and model
  come from the session history. Nothing is written on the server beyond what the agent and tmux already write.
- It uses each CLI's existing login and strips `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` from the chats' environment: usage
  counts against your plan, not the API.

## Contributing

Issues and pull requests are welcome. **[CONTRIBUTING.md](CONTRIBUTING.md)** explains how to set up the environment, run
the tests and what to check before opening a PR.

## License

[GPL-3.0-only](LICENSE). The terminal emulator and view in `android/terminal/` come from
[Termux](https://github.com/termux/termux-app) (Apache 2.0, see `android/terminal/TERMUX-LICENSE.md`). The JetBrains Mono
Nerd Font bundled with the Android app is distributed under the SIL Open Font License (`android/licenses/`).
