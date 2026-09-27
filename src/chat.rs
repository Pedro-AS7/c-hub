use std::path::PathBuf;
use std::sync::Mutex;

use crate::claude::Start;
use crate::sessions::SavedSession;
use crate::term::{self, Terminal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
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
}

impl Chat {
    pub fn new(id: term::TabId, cwd: PathBuf, model: Option<String>, prompt: Option<String>) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        Self::with_start(
            id,
            cwd,
            model,
            Start::New {
                session_id: session_id.clone(),
            },
            Some(session_id),
            "Novo chat".into(),
        )
        .with_prompt(prompt)
    }

    /// Retoma uma sessão salva; o histórico aparece no próprio Claude Code.
    pub fn resume(id: term::TabId, saved: &SavedSession, model: Option<String>) -> Self {
        Self::with_start(
            id,
            saved.cwd.clone(),
            model,
            Start::Resume {
                session_id: saved.id.clone(),
            },
            Some(saved.id.clone()),
            saved.title.clone(),
        )
    }

    /// Sub-chat: nova sessão que começa com todo o contexto desta.
    pub fn fork(&self, id: term::TabId) -> Option<Self> {
        let parent_session_id = self.session_id.clone()?;
        let mut chat = Self::with_start(
            id,
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
        let session_id = self.session_id.clone()?;
        let mut chat = Self::with_start(
            id,
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
        cwd: PathBuf,
        model: Option<String>,
        start: Start,
        session_id: Option<String>,
        title: String,
    ) -> Self {
        Self {
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
        }
    }

    fn with_prompt(mut self, prompt: Option<String>) -> Self {
        self.prompt = prompt;
        self
    }

    pub fn is_alive(&self) -> bool {
        self.status != Status::Exited
    }

    /// Pid do `claude` deste chat.
    pub fn pid(&self) -> Option<u32> {
        self.terminal.as_ref()?.lock().ok()?.pid()
    }
}
