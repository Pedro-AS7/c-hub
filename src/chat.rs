use std::path::PathBuf;
use std::sync::Mutex;

use cosmic::widget::text_editor;

use crate::agent::Agent;
use crate::claude::{Mode, Start};
use crate::commands::Command;
use crate::restore::OpenChat;
use crate::sessions::SavedSession;
use crate::term::{self, Terminal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Restaurado da última execução: o `claude` só abre quando o chat for selecionado.
    Suspended,
    Starting,
    Working,
    /// O Claude Code espera uma resposta (permissão, pergunta).
    Waiting,
    Ready,
    Exited,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Suspended => "em espera",
            Status::Starting => "iniciando",
            Status::Working => "trabalhando",
            Status::Waiting => "precisa de você",
            Status::Ready => "pronto",
            Status::Exited => "encerrado",
        }
    }

    /// Status gravado pelo Claude Code em `~/.claude/sessions/<pid>.json`.
    pub fn from_claude(status: &str) -> Option<Self> {
        match status {
            "busy" => Some(Status::Working),
            "waiting" => Some(Status::Waiting),
            "idle" => Some(Status::Ready),
            _ => None,
        }
    }
}

/// Um chat é um `claude` interativo rodando no seu próprio terminal.
pub struct Chat {
    pub agent: Agent,
    pub codex_thread_hint: Option<String>,
    pub context_window: Option<u32>,
    pub codex_limits: Vec<crate::usage::Limit>,
    /// Identifica o chat e os eventos do terminal dele (mesma numeração das abas do terminal).
    pub id: term::TabId,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub start: Start,
    /// Mensagem passada ao `claude` ao abrir (`--prompt`).
    pub prompt: Option<String>,
    /// Id da sessão no Claude Code. Num sub-chat, só é conhecido depois que o processo sobe;
    /// muda também com `/clear`.
    pub session_id: Option<String>,
    pub title: String,
    pub parent: Option<String>,
    pub status: Status,
    /// Modelo usado de fato, lido do histórico da sessão.
    pub active_model: Option<String>,
    pub context_tokens: u32,
    /// `None` até o canal de eventos do terminal existir ou se o `claude` não abriu.
    pub terminal: Option<Mutex<Terminal>>,
    /// Texto do campo de mensagem abaixo do terminal.
    pub composer: text_editor::Content<cosmic::Renderer>,
    /// Modo de permissão lido na tela; `None` até aparecer no rodapé da TUI.
    pub mode: Option<Mode>,
    pub mode_change: Option<ModeChange>,
    /// Comandos `/…` sugeridos no campo; carregados na primeira `/`.
    pub commands: Option<Vec<Command>>,
}

/// Troca de modo em andamento: um Shift+Tab por vez até a tela mostrar o modo pedido.
#[derive(Clone, Copy, Debug)]
pub struct ModeChange {
    pub target: Mode,
    /// Modo antes da troca: voltar a ele quer dizer que o pedido não está no ciclo
    /// (ex.: "Sem permissões" sem `--dangerously-skip-permissions`).
    pub start: Option<Mode>,
    pub steps: u8,
}

impl Chat {
    pub fn new(id: term::TabId, agent: Agent, cwd: PathBuf, model: Option<String>, prompt: Option<String>) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        Self::with_start(
            id,
            agent,
            cwd,
            model,
            Start::New {
                session_id: session_id.clone(),
            },
            (agent == Agent::Claude).then_some(session_id),
            "Novo chat".into(),
        )
        .with_prompt(prompt)
    }

    /// Retoma uma sessão salva; o histórico aparece no próprio Claude Code.
    pub fn resume(id: term::TabId, saved: &SavedSession, model: Option<String>) -> Self {
        Self::with_start(
            id,
            saved.agent,
            saved.cwd.clone(),
            model,
            Start::Resume {
                session_id: saved.id.clone(),
            },
            Some(saved.id.clone()),
            saved.title.clone(),
        )
    }

    /// Chat aberto na última execução, em espera até ser selecionado. Sem o `.jsonl` (sessão
    /// sem nenhuma mensagem, ou apagada pelo Claude Code), o `--resume` falharia: volta como
    /// chat novo na mesma pasta.
    pub fn restore(id: term::TabId, open: &OpenChat, saved: bool) -> Self {
        let mut chat = if saved {
            let mut chat = Self::with_start(
                id,
                open.agent,
                open.cwd.clone(),
                open.model.clone(),
                Start::Resume {
                    session_id: open.session_id.clone(),
                },
                Some(open.session_id.clone()),
                open.title.clone(),
            );
            chat.parent = open.parent.clone();
            chat
        } else {
            Self::new(id, open.agent, open.cwd.clone(), open.model.clone(), None)
        };
        chat.status = Status::Suspended;
        chat
    }

    /// Sub-chat: nova sessão que começa com todo o contexto desta.
    pub fn fork(&self, id: term::TabId) -> Option<Self> {
        let parent_session_id = self.session_id.clone()?;
        let mut chat = Self::with_start(
            id,
            self.agent,
            self.cwd.clone(),
            self.model.clone(),
            Start::Fork { parent_session_id },
            None,
            format!("{} (sub)", self.title),
        );
        chat.parent = Some(self.title.clone());
        chat.active_model = self.active_model.clone();
        chat.context_tokens = self.context_tokens;
        Some(chat)
    }

    /// Abre de novo uma sessão encerrada, no mesmo lugar da lista.
    pub fn reopen(&self, id: term::TabId) -> Option<Self> {
        let Some(session_id) = self.session_id.clone() else {
            return Some(Self::new(id, self.agent, self.cwd.clone(), self.model.clone(), None));
        };
        let mut chat = Self::with_start(
            id,
            self.agent,
            self.cwd.clone(),
            self.model.clone(),
            Start::Resume {
                session_id: session_id.clone(),
            },
            Some(session_id),
            self.title.clone(),
        );
        chat.parent = self.parent.clone();
        chat.active_model = self.active_model.clone();
        chat.context_tokens = self.context_tokens;
        Some(chat)
    }

    fn with_start(
        id: term::TabId,
        agent: Agent,
        cwd: PathBuf,
        model: Option<String>,
        start: Start,
        session_id: Option<String>,
        title: String,
    ) -> Self {
        Self {
            agent,
            codex_thread_hint: None,
            context_window: None,
            codex_limits: Vec::new(),
            id,
            cwd,
            model,
            start,
            prompt: None,
            session_id,
            title,
            parent: None,
            status: Status::Starting,
            active_model: None,
            context_tokens: 0,
            terminal: None,
            composer: text_editor::Content::new(),
            mode: None,
            mode_change: None,
            commands: None,
        }
    }

    fn with_prompt(mut self, prompt: Option<String>) -> Self {
        self.prompt = prompt;
        self
    }

    pub fn is_alive(&self) -> bool {
        self.status != Status::Exited
    }

    /// Manda bytes ao `claude`, como teclas digitadas no terminal.
    pub fn send(&self, bytes: impl Into<Vec<u8>>) {
        if let Some(Ok(terminal)) = self.terminal.as_ref().map(Mutex::lock) {
            terminal.input_scroll(bytes.into());
        }
    }

    /// Cola o texto no `claude` (bracketed paste: quebras de linha não enviam a mensagem).
    pub fn paste(&self, text: String) {
        if let Some(Ok(terminal)) = self.terminal.as_ref().map(Mutex::lock) {
            terminal.paste(text);
        }
    }

    /// Relê o modo no rodapé da TUI; mantém o anterior se o rodapé não está na tela.
    pub fn read_mode(&mut self) {
        if self.agent != Agent::Claude { return; }
        let Some(Ok(terminal)) = self.terminal.as_ref().map(Mutex::lock) else {
            return;
        };
        if let Some(mode) = Mode::from_screen(&term::screen_tail(&terminal, 4)) {
            self.mode = Some(mode);
        }
    }

    /// Pid do `claude` deste chat.
    pub fn pid(&self) -> Option<u32> {
        self.terminal.as_ref()?.lock().ok()?.pid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_session_keeps_its_agent_when_restored_forked_and_reopened() {
        let open = OpenChat {
            agent: Agent::Codex, session_id: "sessao-codex".into(), cwd: "/tmp".into(),
            title: "Teste".into(), model: Some("modelo-codex".into()), parent: None,
        };
        let restored = Chat::restore(1, &open, true);
        assert_eq!(restored.agent, Agent::Codex);
        assert_eq!(restored.status, Status::Suspended);
        assert!(matches!(restored.start, Start::Resume { .. }));
        for chat in [restored.fork(2).unwrap(), restored.reopen(3).unwrap()] {
            assert_eq!(chat.agent, Agent::Codex);
            assert_eq!(chat.model.as_deref(), Some("modelo-codex"));
        }
        assert_eq!(restored.fork(2).unwrap().session_id, None);
    }

    #[test]
    fn missing_codex_history_starts_a_new_codex_chat() {
        let open = OpenChat {
            agent: Agent::Codex, session_id: "draft:1".into(), cwd: "/tmp".into(),
            title: "Novo chat".into(), model: None, parent: None,
        };
        let chat = Chat::restore(1, &open, false);
        assert_eq!(chat.agent, Agent::Codex);
        assert_eq!(chat.session_id, None);
        assert!(matches!(chat.start, Start::New { .. }));
        assert!(chat.fork(2).is_none());
        assert_eq!(chat.reopen(3).unwrap().agent, Agent::Codex);
        let claude = Chat::new(2, Agent::Claude, "/tmp".into(), None, None);
        assert!(claude.session_id.is_some());
    }
}
