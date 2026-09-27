use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::iced::{Alignment, Length, Subscription};
use cosmic::widget::{self, pane_grid, segmented_button, tooltip};
use cosmic::{Application, ApplicationExt, Element, theme};

use crate::chat::{Chat, Status};
use crate::claude;
use crate::dirs;
use crate::sessions::{self, Running, SavedSession, Stats};
use crate::term::{self, Terminal};
use crate::usage::{self, Limit, Window};

/// Intervalo de atualização automática do painel de uso.
const REFRESH_EVERY: Duration = Duration::from_secs(180);
/// Intervalo da leitura do status das sessões (`~/.claude/sessions`).
const POLL_EVERY: Duration = Duration::from_secs(1);
const TITLE_MAX_CHARS: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Sidebar,
    Chat,
    Terminal,
}

#[derive(Clone, Debug)]
pub enum Message {
    SelectChat(usize),
    NewChat,
    NewChatInput(String),
    NewChatConfirm,
    NewChatCancel,
    NewChatBrowse,
    NewChatPicked(Option<PathBuf>),
    ToggleDirGroup(PathBuf),
    ShowSessions,
    CloseSessions,
    SessionsLoaded(Vec<SavedSession>),
    ResumeSession(usize),
    SubChat,
    ReopenChat,
    Poll,
    Polled(Vec<Polled>),
    OpenLink(String),
    RefreshInfo,
    UsageFetched(Result<Vec<Limit>, String>),
    PaneResized(pane_grid::ResizeEvent),
    HideSection(Section),
    ShowSection(Section),
    NewTerminalTab,
    TerminalTabActivated(segmented_button::Entity),
    TerminalTabClosed(segmented_button::Entity),
    TermEventTx(term::EventSender),
    TermEvent(term::TabId, term::Event),
}

/// Estado de um chat lido do disco pelo `Message::Poll`.
#[derive(Clone, Debug)]
pub struct Polled {
    chat: term::TabId,
    running: Option<Running>,
    /// Só do chat em foco (é o único mostrado no painel "Uso").
    stats: Option<Stats>,
}

/// Opções da linha de comando.
#[derive(Clone, Debug, Default)]
pub struct Flags {
    /// Modelo usado nos chats novos (padrão: o do Claude Code).
    pub model: Option<String>,
    /// Primeira mensagem, passada ao `claude` do primeiro chat.
    pub prompt: Option<String>,
}

pub struct App {
    core: Core,
    chats: Vec<Chat>,
    focused: usize,
    default_dir: PathBuf,
    model: Option<String>,
    /// Id do widget do terminal do chat, para dar foco ao trocar de chat.
    chat_term_id: widget::Id,
    polling: bool,
    panes: pane_grid::State<Section>,
    sidebar_visible: bool,
    chat_visible: bool,
    terminal_visible: bool,
    /// Proporções das divisões, guardadas para refazer o layout ao esconder/mostrar seções.
    sidebar_ratio: f32,
    chat_ratio: f32,
    /// Texto do diálogo de nova sessão; `None` com o diálogo fechado.
    new_chat_input: Option<String>,
    new_chat_input_id: widget::Id,
    /// Abas do terminal; cada uma guarda seu `TabId` e o `Mutex<Terminal>`.
    terminal_tabs: segmented_button::SingleSelectModel,
    /// Numeração única dos terminais: abas e chats.
    next_terminal_id: term::TabId,
    /// Canal dos eventos do PTY; chega pela subscription logo depois de abrir o app.
    term_events: Option<term::EventSender>,
    /// Limites do plano, do `/usage` e atualizados pelo `rate_limit_event` das respostas.
    limits: Vec<Limit>,
    usage_updated: Option<String>,
    usage_error: Option<String>,
    fetching_usage: bool,
    /// Sessões salvas mostradas no painel lateral; `None` enquanto carrega.
    saved_sessions: Option<Vec<SavedSession>>,
    /// Diretórios abertos na lista de sessões salvas.
    expanded_dirs: HashSet<PathBuf>,
}

impl Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = "com.pedro.Chube";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Flags) -> (Self, Task<Message>) {
        // Aberto pelo launcher, o diretório atual é `/`: nesse caso, usa a home.
        let default_dir = std::env::current_dir()
            .ok()
            .filter(|dir| dir.parent().is_some())
            .unwrap_or_else(claude::home_dir);

        let mut app = App {
            core,
            chats: Vec::new(),
            focused: 0,
            default_dir,
            model: flags.model,
            chat_term_id: widget::Id::unique(),
            polling: false,
            panes: pane_grid::State::new(Section::Chat).0,
            sidebar_visible: true,
            chat_visible: true,
            terminal_visible: true,
            sidebar_ratio: 0.22,
            chat_ratio: 0.64,
            new_chat_input: None,
            new_chat_input_id: widget::Id::unique(),
            terminal_tabs: segmented_button::SingleSelectModel::default(),
            next_terminal_id: 0,
            term_events: None,
            limits: Vec::new(),
            usage_updated: None,
            usage_error: None,
            fetching_usage: false,
            saved_sessions: None,
            expanded_dirs: HashSet::new(),
        };
        app.rebuild_panes();
        // O terminal do chat abre quando o canal de eventos chegar (`Message::TermEventTx`).
        let id = app.take_terminal_id();
        let chat = Chat::new(id, app.default_dir.clone(), app.model.clone(), flags.prompt);
        app.chats.push(chat);
        app.set_header_title("Chube".into());

        let title = match app.core.main_window_id() {
            Some(id) => app.set_window_title("Chube".into(), id),
            None => Task::none(),
        };
        let usage = app.refresh_usage();
        (app, Task::batch([title, usage]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectChat(index) => {
                if index < self.chats.len() {
                    self.focused = index;
                    return self.focus_chat_terminal();
                }
            }
            Message::NewChat => {
                self.new_chat_input = Some(String::new());
                return widget::text_input::focus(self.new_chat_input_id.clone());
            }
            Message::NewChatInput(text) => self.new_chat_input = Some(text),
            Message::NewChatCancel => self.new_chat_input = None,
            Message::NewChatConfirm => {
                let Some(input) = &self.new_chat_input else {
                    return Task::none();
                };
                if let Ok(folder) = dirs::resolve(input, &self.focused_dir()) {
                    self.new_chat_input = None;
                    let id = self.take_terminal_id();
                    return self.open_chat(self.chats.len(), Chat::new(id, folder, self.model.clone(), None));
                }
            }
            Message::NewChatBrowse => {
                return cosmic::task::future(async {
                    let folder = cosmic::dialog::file_chooser::open::Dialog::new()
                        .title("Pasta do novo chat")
                        .open_folder()
                        .await
                        .ok()
                        .and_then(|response| response.url().to_file_path().ok());
                    Message::NewChatPicked(folder)
                });
            }
            // A pasta escolhida vai para o campo, para ser conferida antes de criar.
            Message::NewChatPicked(Some(folder)) => {
                self.new_chat_input = Some(folder.display().to_string());
            }
            Message::NewChatPicked(None) => {}
            Message::ToggleDirGroup(dir) => {
                if !self.expanded_dirs.remove(&dir) {
                    self.expanded_dirs.insert(dir);
                }
            }
            Message::ShowSessions => {
                self.saved_sessions = None;
                self.expanded_dirs = HashSet::from([self.focused_dir()]);
                self.set_show_context(true);
                return cosmic::task::future(async {
                    let list = tokio::task::spawn_blocking(sessions::list)
                        .await
                        .unwrap_or_default();
                    Message::SessionsLoaded(list)
                });
            }
            Message::CloseSessions => self.set_show_context(false),
            Message::SessionsLoaded(list) => self.saved_sessions = Some(list),
            Message::ResumeSession(index) => {
                let Some(saved) = self
                    .saved_sessions
                    .as_ref()
                    .and_then(|list| list.get(index))
                    .cloned()
                else {
                    return Task::none();
                };
                self.set_show_context(false);
                if let Some(open) = self.chat_index_for_session(&saved.id) {
                    self.focused = open;
                    return self.focus_chat_terminal();
                }
                let id = self.take_terminal_id();
                return self.open_chat(self.chats.len(), Chat::resume(id, &saved, self.model.clone()));
            }
            Message::SubChat => {
                let id = self.take_terminal_id();
                if let Some(chat) = self.chats[self.focused].fork(id) {
                    return self.open_chat(self.focused + 1, chat);
                }
            }
            Message::ReopenChat => {
                let id = self.take_terminal_id();
                if let Some(chat) = self.chats[self.focused].reopen(id) {
                    // Descartar o chat antigo encerra o terminal dele.
                    self.chats.remove(self.focused);
                    return self.open_chat(self.focused, chat);
                }
            }
            Message::Poll => return self.poll(),
            Message::Polled(list) => {
                self.polling = false;
                self.apply_poll(list);
            }
            Message::OpenLink(url) => {
                let _ = std::process::Command::new("xdg-open").arg(url).spawn();
            }
            Message::RefreshInfo => return self.refresh_usage(),
            Message::UsageFetched(result) => {
                self.fetching_usage = false;
                match result {
                    Ok(limits) => {
                        self.limits = limits;
                        self.usage_error = None;
                        self.usage_updated = Some(now_hms());
                    }
                    Err(error) => self.usage_error = Some(error),
                }
            }
            Message::PaneResized(event) => {
                // Com o painel da esquerda visível, a divisão da raiz é a dele.
                let root = match self.panes.layout() {
                    pane_grid::Node::Split { id, .. } => Some(*id),
                    pane_grid::Node::Pane(_) => None,
                };
                if self.sidebar_visible && root == Some(event.split) {
                    self.sidebar_ratio = event.ratio;
                } else {
                    self.chat_ratio = event.ratio;
                }
                self.panes.resize(event.split, event.ratio);
            }
            Message::HideSection(section) => {
                // Chat e terminal não somem os dois ao mesmo tempo.
                match section {
                    Section::Sidebar => self.sidebar_visible = false,
                    Section::Chat if self.terminal_visible => self.chat_visible = false,
                    Section::Terminal if self.chat_visible => self.terminal_visible = false,
                    _ => {}
                }
                self.rebuild_panes();
            }
            Message::ShowSection(section) => {
                match section {
                    Section::Sidebar => self.sidebar_visible = true,
                    Section::Chat => self.chat_visible = true,
                    Section::Terminal => self.terminal_visible = true,
                }
                self.rebuild_panes();
            }
            Message::NewTerminalTab => self.add_terminal_tab(),
            Message::TerminalTabActivated(id) => self.terminal_tabs.activate(id),
            // Remover a aba descarta o `Terminal`, que encerra o shell.
            Message::TerminalTabClosed(id) => self.terminal_tabs.remove(id),
            Message::TermEventTx(sender) => {
                let first = self.term_events.is_none();
                self.term_events = Some(sender);
                if first {
                    if self.terminal_tabs.len() == 0 {
                        self.add_terminal_tab();
                    }
                    for index in 0..self.chats.len() {
                        self.spawn_chat_terminal(index);
                    }
                    return self.focus_chat_terminal();
                }
            }
            Message::TermEvent(id, event) => return self.handle_term_event(id, event),
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            cosmic::iced::time::every(REFRESH_EVERY).map(|_| Message::RefreshInfo),
            Subscription::run(term_events),
        ];
        if self.chats.iter().any(Chat::is_alive) {
            subscriptions.push(cosmic::iced::time::every(POLL_EVERY).map(|_| Message::Poll));
        }
        Subscription::batch(subscriptions)
    }

    fn context_drawer(&self) -> Option<cosmic::app::ContextDrawer<'_, Message>> {
        if !self.core.window.show_context {
            return None;
        }
        Some(
            cosmic::app::context_drawer(self.sessions_list(), Message::CloseSessions)
                .title("Sessões salvas"),
        )
    }

    fn dialog(&self) -> Option<Element<'_, Message>> {
        let input = self.new_chat_input.as_ref()?;
        let space = theme::spacing();
        let resolved = dirs::resolve(input, &self.focused_dir());

        let preview = match &resolved {
            Ok(path) => format!("→ {}", path.display()),
            Err(error) => error.clone(),
        };
        let names: Vec<String> = dirs::shortcuts().into_iter().map(|(name, _)| name).collect();
        let mut control = widget::Column::new()
            .push(
                widget::text_input("Diretório, atalho do fshort ou .", input)
                    .id(self.new_chat_input_id.clone())
                    .on_input(Message::NewChatInput)
                    .on_submit(|_| Message::NewChatConfirm),
            )
            .push(widget::text::caption(preview))
            .spacing(space.space_xs);
        if !names.is_empty() {
            control = control.push(widget::text::caption(format!(
                "Atalhos: {}",
                names.join(", ")
            )));
        }

        Some(
            widget::dialog()
                .title("Nova sessão")
                .body(format!(
                    "\".\" usa a pasta do chat em foco ({}).",
                    claude::display_path(&self.focused_dir())
                ))
                .control(control)
                .primary_action(
                    widget::button::suggested("Criar")
                        .on_press_maybe(resolved.is_ok().then_some(Message::NewChatConfirm)),
                )
                .secondary_action(widget::button::standard("Cancelar").on_press(Message::NewChatCancel))
                .tertiary_action(
                    widget::button::text("Escolher pasta…").on_press(Message::NewChatBrowse),
                )
                .into(),
        )
    }

    fn view(&self) -> Element<'_, Message> {
        let space = theme::spacing();

        widget::container(
            pane_grid::PaneGrid::new(&self.panes, |_pane, section, _maximized| {
                pane_grid::Content::new(self.section_view(*section))
            })
            .spacing(space.space_s)
            .min_size(180)
            .on_resize(10, Message::PaneResized),
        )
        .padding([0, space.space_s, space.space_s, space.space_s])
        .into()
    }
}

impl App {
    fn take_terminal_id(&mut self) -> term::TabId {
        self.next_terminal_id += 1;
        self.next_terminal_id
    }

    /// Busca os limites com o `/usage`, se já não houver uma busca em andamento.
    fn refresh_usage(&mut self) -> Task<Message> {
        if self.fetching_usage {
            return Task::none();
        }
        self.fetching_usage = true;
        cosmic::task::future(async { Message::UsageFetched(usage::fetch().await) })
    }

    /// Refaz o layout com as seções visíveis, mantendo as proporções.
    fn rebuild_panes(&mut self) {
        use pane_grid::{Axis, Configuration};

        let split = |ratio, a, b| Configuration::Split {
            axis: Axis::Vertical,
            ratio,
            a: Box::new(a),
            b: Box::new(b),
        };
        let main = match (self.chat_visible, self.terminal_visible) {
            (true, true) => split(
                self.chat_ratio,
                Configuration::Pane(Section::Chat),
                Configuration::Pane(Section::Terminal),
            ),
            (false, true) => Configuration::Pane(Section::Terminal),
            _ => Configuration::Pane(Section::Chat),
        };
        let layout = if self.sidebar_visible {
            split(self.sidebar_ratio, Configuration::Pane(Section::Sidebar), main)
        } else {
            main
        };
        self.panes = pane_grid::State::with_configuration(layout);
    }

    fn focused_dir(&self) -> PathBuf {
        self.chats
            .get(self.focused)
            .map_or_else(|| self.default_dir.clone(), |chat| chat.cwd.clone())
    }

    /// Põe o chat na lista, em `index`, abre o `claude` dele e o deixa em foco.
    fn open_chat(&mut self, index: usize, chat: Chat) -> Task<Message> {
        self.chats.insert(index, chat);
        self.focused = index;
        self.spawn_chat_terminal(index);
        self.focus_chat_terminal()
    }

    /// Abre o terminal do chat rodando o `claude`, se o canal de eventos já existe.
    fn spawn_chat_terminal(&mut self, index: usize) {
        let Some(events) = self.term_events.clone() else {
            return;
        };
        let chat = &mut self.chats[index];
        if chat.terminal.is_some() || !chat.is_alive() {
            return;
        }
        let command = claude::command(&chat.start, chat.model.as_deref(), chat.prompt.take().as_deref());
        match term::spawn(chat.id, events, chat.cwd.clone(), Some(command), &theme::active()) {
            Ok(terminal) => chat.terminal = Some(terminal),
            Err(err) => {
                log::error!("não consegui abrir o claude: {err}");
                chat.status = Status::Exited;
            }
        }
    }

    fn focus_chat_terminal(&self) -> Task<Message> {
        widget::text_input::focus(self.chat_term_id.clone())
    }

    /// Lê o status de cada `claude` (e o contexto do chat em foco) fora da thread da UI.
    fn poll(&mut self) -> Task<Message> {
        if self.polling {
            return Task::none();
        }
        let focused = self.chats.get(self.focused).map(|chat| chat.id);
        let targets: Vec<(term::TabId, Option<u32>, Option<String>)> = self
            .chats
            .iter()
            .filter(|chat| chat.is_alive())
            .map(|chat| (chat.id, chat.pid(), chat.session_id.clone()))
            .collect();
        self.polling = true;
        cosmic::task::future(async move {
            let list = tokio::task::spawn_blocking(move || {
                targets
                    .into_iter()
                    .map(|(chat, pid, session_id)| {
                        let running = pid.and_then(sessions::running);
                        let session_id = running
                            .as_ref()
                            .map(|running| running.session_id.clone())
                            .or(session_id);
                        let stats = (Some(chat) == focused)
                            .then(|| session_id.as_deref().and_then(sessions::stats))
                            .flatten();
                        Polled { chat, running, stats }
                    })
                    .collect()
            })
            .await
            .unwrap_or_default();
            Message::Polled(list)
        })
    }

    fn apply_poll(&mut self, list: Vec<Polled>) {
        for polled in list {
            let Some(chat) = self.chats.iter_mut().find(|chat| chat.id == polled.chat) else {
                continue;
            };
            // O `claude` pode ter saído depois da leitura.
            if !chat.is_alive() {
                continue;
            }
            if let Some(running) = polled.running {
                // `/clear` troca a sessão: o contexto volta a zero. Num sub-chat, o id
                // aparece aqui pela primeira vez e o contexto herdado continua valendo.
                if chat
                    .session_id
                    .as_ref()
                    .is_some_and(|id| *id != running.session_id)
                {
                    chat.context_tokens = 0;
                }
                chat.session_id = Some(running.session_id);
                if let Some(status) = Status::from_claude(&running.status) {
                    chat.status = status;
                }
            }
            if let Some(stats) = polled.stats {
                chat.context_tokens = stats.context_tokens;
                if stats.model.is_some() {
                    chat.active_model = stats.model;
                }
            }
        }
    }

    fn chat_index_for_session(&self, session_id: &str) -> Option<usize> {
        self.chats
            .iter()
            .position(|chat| chat.is_alive() && chat.session_id.as_deref() == Some(session_id))
    }

    /// Abre um shell numa aba nova, na pasta do chat em foco.
    fn add_terminal_tab(&mut self) {
        let Some(events) = self.term_events.clone() else {
            return;
        };
        let id = self.take_terminal_id();
        let entity = self
            .terminal_tabs
            .insert()
            .text("Terminal")
            .closable()
            .data(id)
            .activate()
            .id();
        match term::spawn(id, events, self.focused_dir(), None, &theme::active()) {
            Ok(terminal) => self.terminal_tabs.data_set::<Mutex<Terminal>>(entity, terminal),
            Err(err) => {
                log::error!("não consegui abrir o terminal: {err}");
                self.terminal_tabs.text_set(entity, "Erro ao abrir");
            }
        }
    }

    fn terminal_entity(&self, id: term::TabId) -> Option<segmented_button::Entity> {
        self.terminal_tabs
            .iter()
            .find(|entity| self.terminal_tabs.data::<term::TabId>(*entity) == Some(&id))
    }

    /// Eventos do emulador, como no `Message::TermEvent` do cosmic-term.
    fn handle_term_event(&mut self, id: term::TabId, event: term::Event) -> Task<Message> {
        if let Some(chat) = self.chats.iter_mut().find(|chat| chat.id == id) {
            match event {
                term::Event::Title(title) => {
                    if let Some(title) = claude::title_from_terminal(&title) {
                        chat.title = short_title(&title);
                    }
                }
                // O `claude` saiu: o terminal fica na tela com a última saída.
                term::Event::Exit => chat.status = Status::Exited,
                event => {
                    if let Some(terminal) = &chat.terminal {
                        return terminal_io(terminal, event);
                    }
                }
            }
            return Task::none();
        }

        let Some(entity) = self.terminal_entity(id) else {
            return Task::none();
        };
        match event {
            term::Event::Title(title) => {
                self.terminal_tabs.text_set(entity, title);
            }
            term::Event::ResetTitle => {
                self.terminal_tabs.text_set(entity, "Terminal");
            }
            // O shell saiu (`exit`): fecha a aba.
            term::Event::Exit => self.terminal_tabs.remove(entity),
            event => {
                if let Some(terminal) = self.terminal_tabs.data::<Mutex<Terminal>>(entity) {
                    return terminal_io(terminal, event);
                }
            }
        }
        Task::none()
    }

    fn section_view(&self, section: Section) -> Element<'_, Message> {
        match section {
            Section::Sidebar => widget::Column::new()
                .push(self.chats_card())
                .push(self.info_card())
                .spacing(theme::spacing().space_s)
                .height(Length::Fill)
                .into(),
            Section::Chat => self.chat_section(),
            Section::Terminal => self.terminal_section(),
        }
    }

    fn chats_card(&self) -> Element<'_, Message> {
        let space = theme::spacing();

        let mut list = widget::Column::new().spacing(space.space_xxs);
        for (index, chat) in self.chats.iter().enumerate() {
            let subtitle = format!(
                "{} · {}",
                chat.status.label(),
                claude::display_path(&chat.cwd)
            );
            let item = widget::Column::new()
                .push(widget::text::body(chat.title.as_str()))
                .push(widget::text::caption(subtitle));
            list = list.push(
                widget::button::custom(item)
                    .class(theme::Button::ListItem([8.0; 4]))
                    .selected(index == self.focused)
                    .padding([space.space_xxs, space.space_xs])
                    .width(Length::Fill)
                    .on_press(Message::SelectChat(index)),
            );
        }

        let new_chat = icon_button("list-add-symbolic", "Nova sessão", Some(Message::NewChat));

        let header = widget::Row::new()
            .push(widget::text::heading("Chats ativos"))
            .push(horizontal_fill())
            .push(icon_button(
                "document-open-recent-symbolic",
                "Sessões salvas",
                Some(Message::ShowSessions),
            ))
            .push(icon_button(
                "pan-start-symbolic",
                "Esconder o painel",
                Some(Message::HideSection(Section::Sidebar)),
            ))
            .align_y(Alignment::Center);

        card(
            widget::Column::new()
                .push(header)
                .push(widget::scrollable(list).height(Length::Fill))
                .push(
                    widget::Row::new()
                        .push(horizontal_fill())
                        .push(new_chat)
                        .align_y(Alignment::Center),
                )
                .spacing(space.space_xs),
        )
        .height(Length::FillPortion(3))
        .into()
    }

    fn info_card(&self) -> Element<'_, Message> {
        let space = theme::spacing();
        let chat = &self.chats[self.focused];
        let working = self
            .chats
            .iter()
            .filter(|chat| chat.status == Status::Working)
            .count();

        let header = widget::Row::new()
            .push(widget::text::heading("Uso"))
            .push(horizontal_fill())
            .push(icon_button(
                "view-refresh-symbolic",
                "Atualizar agora",
                (!self.fetching_usage).then_some(Message::RefreshInfo),
            ))
            .align_y(Alignment::Center);

        let context = if chat.context_tokens == 0 {
            meter("Contexto do chat em foco", 0.0, "—".into(), None)
        } else {
            // O histórico não diz o tamanho da janela: acima de 200 mil, só pode ser a de 1 milhão.
            let window: u32 = if chat.context_tokens > 200_000 { 1_000_000 } else { 200_000 };
            meter(
                "Contexto do chat em foco",
                chat.context_tokens as f32 / window as f32,
                format!("{} / {} mil", thousands(chat.context_tokens), window / 1000),
                None,
            )
        };

        let mut body = widget::Column::new().push(context).spacing(space.space_s);
        if self.limits.is_empty() {
            body = body
                .push(meter("Limite de 5 horas", 0.0, "—".into(), None))
                .push(meter("Limite semanal", 0.0, "—".into(), None));
        }
        for limit in &self.limits {
            let label = match &limit.window {
                Window::Session => "Limite de 5 horas".to_owned(),
                Window::Week(models) if models == "all models" => "Limite semanal".to_owned(),
                Window::Week(models) => format!("Semanal ({models})"),
            };
            body = body.push(meter(
                label,
                limit.used,
                format!("{:.0}%", limit.used * 100.0),
                limit.resets.map(usage::describe_reset),
            ));
        }
        body = body.push(widget::text::body(format!(
            "{} chats abertos · {} trabalhando",
            self.chats.len(),
            working
        )));

        let status = if self.fetching_usage {
            "Atualizando…".to_owned()
        } else if let Some(error) = &self.usage_error {
            format!("Erro ao atualizar: {error}")
        } else if let Some(time) = &self.usage_updated {
            format!("Atualizado às {time} · a cada 3 min")
        } else {
            "Ainda não atualizado".to_owned()
        };
        body = body.push(widget::text::caption(status));

        card(
            widget::Column::new()
                .push(header)
                // Rola em vez de cortar quando a janela é baixa.
                .push(
                    widget::scrollable(body.padding([0, space.space_s, 0, 0]))
                        .height(Length::Fill),
                )
                .spacing(space.space_s),
        )
        .height(Length::FillPortion(2))
        .into()
    }

    fn chat_section(&self) -> Element<'_, Message> {
        let space = theme::spacing();
        let chat = &self.chats[self.focused];

        let mut details = vec![claude::display_path(&chat.cwd)];
        if let Some(model) = chat.active_model.as_ref().or(chat.model.as_ref()) {
            details.push(model.clone());
        }
        if let Some(parent) = &chat.parent {
            details.push(format!("sub-chat de \"{parent}\""));
        }
        let title = widget::Column::new()
            .push(widget::text::heading(chat.title.as_str()))
            .push(widget::text::caption(details.join(" · ")));

        let mut header = widget::Row::new();
        if !self.sidebar_visible {
            header = header.push(show_sidebar_button());
        }
        let can_fork = chat.session_id.is_some() && chat.status != Status::Starting;
        header = header
            .push(title)
            .push(horizontal_fill())
            .push(icon_button(
                "edit-copy-symbolic",
                "Criar sub-chat com o contexto deste chat",
                can_fork.then_some(Message::SubChat),
            ))
            .align_y(Alignment::Center)
            .spacing(space.space_xxs);
        header = if self.terminal_visible {
            header.push(icon_button(
                "pan-start-symbolic",
                "Esconder o chat",
                Some(Message::HideSection(Section::Chat)),
            ))
        } else {
            header.push(icon_button(
                "utilities-terminal-symbolic",
                "Mostrar o terminal",
                Some(Message::ShowSection(Section::Terminal)),
            ))
        };

        let mut column = widget::Column::new().push(header);
        if !chat.is_alive() {
            let text = if chat.terminal.is_some() {
                "O claude desta sessão foi encerrado."
            } else {
                "Não consegui abrir o claude."
            };
            column = column.push(
                widget::container(
                    widget::Row::new()
                        .push(widget::text::body(text).width(Length::Fill))
                        .push(
                            widget::button::standard("Reabrir sessão").on_press_maybe(
                                chat.session_id.is_some().then_some(Message::ReopenChat),
                            ),
                        )
                        .spacing(space.space_s)
                        .align_y(Alignment::Center),
                )
                .padding(space.space_xs)
                .width(Length::Fill)
                .class(theme::Container::Secondary),
            );
        }

        let body: Element<'_, Message> = match &chat.terminal {
            Some(terminal) => {
                let current = theme::active();
                let cosmic = current.cosmic();
                term::terminal_box(terminal, &term::KEY_BINDS)
                    .id(self.chat_term_id.clone())
                    .padding(space.space_xxs)
                    .pane_border_radius(Some(cosmic.corner_radii.radius_s.into()))
                    .on_open_hyperlink(Some(Box::new(Message::OpenLink)))
                    .into()
            }
            None => widget::container(widget::text::body(if chat.is_alive() {
                "Iniciando o Claude Code…"
            } else {
                ""
            }))
            .center(Length::Fill)
            .into(),
        };

        card(column.push(body).spacing(space.space_s)).into()
    }

    fn sessions_list(&self) -> Element<'_, Message> {
        let space = theme::spacing();
        let Some(list) = &self.saved_sessions else {
            return widget::text::body("Carregando…").into();
        };
        if list.is_empty() {
            return widget::text::body("Nenhuma sessão salva encontrada.").into();
        }

        // Agrupa por diretório; os grupos seguem a ordem da sessão mais recente de cada um.
        let mut groups: Vec<(&PathBuf, Vec<usize>)> = Vec::new();
        for (index, saved) in list.iter().enumerate() {
            match groups.iter_mut().find(|(dir, _)| **dir == saved.cwd) {
                Some((_, indices)) => indices.push(index),
                None => groups.push((&saved.cwd, vec![index])),
            }
        }

        let mut column = widget::Column::new().spacing(space.space_xxs);
        for (dir, indices) in groups {
            let expanded = self.expanded_dirs.contains(dir);
            let icon = if expanded { "pan-down-symbolic" } else { "pan-end-symbolic" };
            let header = widget::Row::new()
                .push(widget::icon::from_name(icon).size(16))
                .push(widget::text::heading(claude::display_path(dir)))
                .push(horizontal_fill())
                .push(widget::text::caption(indices.len().to_string()))
                .spacing(space.space_xs)
                .align_y(Alignment::Center);
            column = column.push(
                widget::button::custom(header)
                    .class(theme::Button::Text)
                    .padding([space.space_xs, space.space_xxs])
                    .width(Length::Fill)
                    .on_press(Message::ToggleDirGroup(dir.clone())),
            );
            if expanded {
                for index in indices {
                    column = column.push(self.saved_session_item(index, &list[index]));
                }
            }
        }
        column.into()
    }

    fn saved_session_item<'a>(&self, index: usize, saved: &'a SavedSession) -> Element<'a, Message> {
        let space = theme::spacing();
        let open_here = self.chat_index_for_session(&saved.id).is_some();
        let mut details = sessions::relative_time(saved.modified);
        let blocked = match (&saved.running, open_here) {
            (_, true) => {
                details.push_str(" · aberta aqui");
                false
            }
            (Some(_), false) => {
                details.push_str(" · aberta em outro terminal");
                true
            }
            (None, false) => false,
        };
        let item = widget::Column::new()
            .push(widget::text::body(saved.title.as_str()))
            .push(widget::text::caption(details));
        widget::button::custom(item)
            .class(theme::Button::ListItem([8.0; 4]))
            .padding([space.space_xxs, space.space_m])
            .width(Length::Fill)
            .on_press_maybe((!blocked).then_some(Message::ResumeSession(index)))
            .into()
    }

    fn terminal_section(&self) -> Element<'_, Message> {
        let space = theme::spacing();

        let toggle = if self.chat_visible {
            icon_button(
                "pan-end-symbolic",
                "Esconder o terminal",
                Some(Message::HideSection(Section::Terminal)),
            )
        } else {
            icon_button(
                "pan-start-symbolic",
                "Mostrar o chat",
                Some(Message::ShowSection(Section::Chat)),
            )
        };

        let mut header = widget::Row::new();
        if !self.sidebar_visible && !self.chat_visible {
            header = header.push(show_sidebar_button());
        }
        let header = header
            .push(toggle)
            .push(
                widget::tab_bar::horizontal(&self.terminal_tabs)
                    .on_activate(Message::TerminalTabActivated)
                    .on_close(Message::TerminalTabClosed)
                    .width(Length::Fill),
            )
            .push(icon_button(
                "list-add-symbolic",
                "Nova aba do terminal",
                Some(Message::NewTerminalTab),
            ))
            .spacing(space.space_xxs)
            .align_y(Alignment::Center);

        let active = self.terminal_tabs.active();
        let body: Element<'_, Message> =
            match self.terminal_tabs.data::<Mutex<Terminal>>(active) {
                Some(terminal) => {
                    let current = theme::active();
                    let cosmic = current.cosmic();
                    term::terminal_box(terminal, &term::KEY_BINDS)
                        .padding(space.space_xxs)
                        .pane_border_radius(Some(cosmic.corner_radii.radius_s.into()))
                        .on_open_hyperlink(Some(Box::new(Message::OpenLink)))
                        .into()
                }
                None => widget::container(
                    widget::Column::new()
                        .push(widget::text::body("Nenhum terminal aberto."))
                        .push(
                            widget::button::standard("Abrir terminal")
                                .on_press(Message::NewTerminalTab),
                        )
                        .spacing(space.space_s)
                        .align_x(Alignment::Center),
                )
                .center(Length::Fill)
                .into(),
            };

        card(
            widget::Column::new()
                .push(header)
                .push(body)
                .spacing(space.space_s),
        )
        .into()
    }
}

fn card<'a>(content: impl Into<Element<'a, Message>>) -> widget::Container<'a, Message, cosmic::Theme> {
    widget::container(content)
        .padding(theme::spacing().space_s)
        .width(Length::Fill)
        .height(Length::Fill)
        .class(theme::Container::Card)
}

/// Botão de ícone com dica; sem mensagem, fica desativado.
fn icon_button<'a>(
    icon: &'static str,
    label: &'static str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    tooltip::tooltip(
        widget::button::icon(widget::icon::from_name(icon)).on_press_maybe(on_press),
        widget::text::body(label),
        tooltip::Position::Bottom,
    )
    .into()
}

/// Recebe os eventos de todos os terminais (vêm das threads do PTY).
fn term_events() -> impl cosmic::iced::futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(100, async |mut output| {
        use cosmic::iced::futures::SinkExt;

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let _ = output.send(Message::TermEventTx(sender)).await;
        while let Some((id, event)) = receiver.recv().await {
            let _ = output.send(Message::TermEvent(id, event)).await;
        }
    })
}

/// Eventos do emulador comuns aos terminais dos chats e às abas.
fn terminal_io(terminal: &Mutex<Terminal>, event: term::Event) -> Task<Message> {
    use alacritty_terminal::term::ClipboardType;

    match event {
        term::Event::Wakeup | term::Event::MouseCursorDirty => {
            terminal.lock().unwrap().needs_update = true;
        }
        term::Event::PtyWrite(text) => {
            terminal.lock().unwrap().input_no_scroll(text.into_bytes());
        }
        term::Event::ColorRequest(index, format) => {
            let terminal = terminal.lock().unwrap();
            let text = format(terminal.effective_color(index));
            terminal.input_no_scroll(text.into_bytes());
        }
        term::Event::TextAreaSizeRequest(format) => {
            let terminal = terminal.lock().unwrap();
            let text = format(terminal.size().into());
            terminal.input_no_scroll(text.into_bytes());
        }
        term::Event::ClipboardStore(ClipboardType::Clipboard, data) => {
            return cosmic::iced::clipboard::write(data);
        }
        term::Event::ClipboardLoad(ClipboardType::Clipboard, format) => {
            return cosmic::iced::clipboard::read().map(move |data| {
                let _ = format(&data.unwrap_or_default());
                cosmic::action::none()
            });
        }
        _ => {}
    }
    Task::none()
}

fn show_sidebar_button<'a>() -> Element<'a, Message> {
    icon_button(
        "pan-end-symbolic",
        "Mostrar o painel",
        Some(Message::ShowSection(Section::Sidebar)),
    )
}

fn horizontal_fill() -> widget::Space {
    widget::Space::new().width(Length::Fill)
}

fn meter<'a>(
    label: impl Into<std::borrow::Cow<'a, str>> + 'a,
    value: f32,
    detail: String,
    note: Option<String>,
) -> Element<'a, Message> {
    let mut column = widget::Column::new()
        .push(
            widget::Row::new()
                .push(widget::text::body(label).width(Length::Fill))
                .push(widget::text::caption(detail))
                .spacing(theme::spacing().space_xs),
        )
        .push(cosmic::iced::widget::progress_bar(0.0..=1.0, value.clamp(0.0, 1.0)).girth(6.0))
        .spacing(theme::spacing().space_xxs);
    if let Some(note) = note {
        column = column.push(widget::text::caption(note));
    }
    column.into()
}

fn now_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn short_title(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    if line.chars().count() > TITLE_MAX_CHARS {
        format!("{}…", line.chars().take(TITLE_MAX_CHARS).collect::<String>())
    } else {
        line.to_owned()
    }
}

fn thousands(tokens: u32) -> String {
    format!("{:.1} mil", tokens as f32 / 1000.0).replace('.', ",")
}
