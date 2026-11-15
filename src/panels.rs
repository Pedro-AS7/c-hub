//! Painéis mostrados no lugar do terminal do chat: nova sessão e sessões salvas.
//!
//! O de nova sessão é montado uma vez (o campo de texto não pode ser recriado a cada mensagem,
//! senão perde o cursor); `sync` só troca seleção, avisos e o que está habilitado. O de sessões é
//! uma lista de linhas clicáveis, refeita quando `App::sessions_version` muda.

use std::cell::Cell;
use std::path::PathBuf;

use async_channel::Sender;
use gtk4::prelude::*;
use gtk4::{gio, glib, pango};

use crate::app::{App, Launch, Message, RemoteState};
use crate::chat::Status;
use crate::claude;
use crate::dir_shortcuts::DirShortcut;
use crate::remote;
use crate::sessions::{self, SavedSession};
use crate::ui::{
    ICON_LAPTOP, ICON_OMARCHY, ICON_SERVER, button_message, choice, clickable_row, icon_button, label, row, status_glyph,
    text_button,
};

const ICON_RIGHT: &str = "\u{f0da}";
const ICON_DOWN: &str = "\u{f0d7}";
const ICON_REFRESH: &str = "\u{f0450}";
const ICON_WARNING: &str = "\u{f071}";
const ICON_TMUX: &str = "\u{ebc8}";
const ICON_IDLE: &str = "\u{f444}";
const ICON_EDIT: &str = "\u{f03eb}";
const ICON_DELETE: &str = "\u{f01b4}";
const ICON_BOOKMARK: &str = "\u{f00c0}";

/// Painel de nova sessão, no estilo dos menus do Omarchy: agente, lugar e pasta, cada um com o
/// rótulo em cima.
pub struct NewChatPanel {
    pub root: gtk4::ScrolledWindow,
    launches: [(Launch, gtk4::Button); 3],
    places: Vec<(Option<String>, gtk4::Button)>,
    pub entry: gtk4::Entry,
    preview: gtk4::Label,
    shortcuts: gtk4::Label,
    remote_note: gtk4::Label,
    create: gtk4::Button,
    browse: gtk4::Button,
    tx: Sender<Message>,
    /// "salvar atalho" e o balão dele: pasta, nome, aviso e o botão de salvar.
    save_shortcut: gtk4::MenuButton,
    save_target: gtk4::Label,
    save_name: gtk4::Entry,
    save_note: gtk4::Label,
    save_confirm: gtk4::Button,
    /// "atalhos" e a lista do balão dele, refeita quando `App::shortcuts_version` muda.
    show_shortcuts: gtk4::MenuButton,
    shortcut_popover: gtk4::Popover,
    shortcut_list: gtk4::Box,
    shortcuts_version: Cell<u64>,
}

impl NewChatPanel {
    pub fn new(app: &App, window: &gtk4::ApplicationWindow, tx: &Sender<Message>) -> Self {
        let omarchy_label = match app.omarchy_agent {
            Some(agent) => format!("{ICON_OMARCHY} omarchy ({})", agent.key()),
            None => format!("{ICON_OMARCHY} omarchy"),
        };
        let launches = [
            (Launch::Omarchy, choice(&omarchy_label, tx, move || Message::NewChatLaunch(Launch::Omarchy))),
            (Launch::Claude, choice("claude", tx, move || Message::NewChatLaunch(Launch::Claude))),
            (Launch::Codex, choice("codex", tx, move || Message::NewChatLaunch(Launch::Codex))),
        ];
        let agents = flow();
        for (_, button) in &launches {
            agents.append(button);
        }

        let mut places = vec![(None, choice(&format!("{ICON_LAPTOP} local"), tx, || Message::NewChatHost(None)))];
        for host in &app.hosts {
            let value = host.clone();
            let button = choice(&format!("{ICON_SERVER} {host}"), tx, move || Message::NewChatHost(Some(value.clone())));
            places.push((Some(host.clone()), button));
        }
        let place_box = flow();
        for (_, button) in &places {
            place_box.append(button);
        }

        let entry = gtk4::Entry::new();
        entry.set_hexpand(true);
        let sender = tx.clone();
        entry.connect_changed(move |entry| {
            let _ = sender.try_send(Message::NewChatInput(entry.text().to_string()));
        });
        let sender = tx.clone();
        entry.connect_activate(move |_| {
            let _ = sender.try_send(Message::NewChatConfirm);
        });
        let preview = label("", &["dim"]);
        preview.set_ellipsize(pango::EllipsizeMode::Middle);
        let shortcuts = label("", &["dim"]);
        shortcuts.set_wrap(true);
        let (save_shortcut, save_target, save_name, save_note, save_confirm) = save_shortcut_button(tx);
        let (show_shortcuts, shortcut_popover, shortcut_list) = shortcuts_button();
        let remote_note = label(
            "roda num tmux da VPS: fechar o chat só desconecta. A senha da chave é pedida no terminal.",
            &["dim"],
        );
        remote_note.set_wrap(true);

        let create = text_button("criar", "Abrir o chat (Enter)", tx, || Message::NewChatConfirm);
        let cancel = text_button("cancelar", "Voltar ao chat (Esc)", tx, || Message::ClosePanel);
        let browse = gtk4::Button::with_label("escolher pasta…");
        browse.add_css_class("text");
        browse.set_tooltip_text(Some("Abrir o seletor de pastas"));
        let sender = tx.clone();
        let parent = window.clone();
        browse.connect_clicked(move |_| {
            // O seletor do GTK (pelo portal do sistema, quando há um).
            let dialog = gtk4::FileDialog::builder().title("Pasta do novo chat").modal(true).build();
            let sender = sender.clone();
            dialog.select_folder(Some(&parent), None::<&gio::Cancellable>, move |result| {
                let folder: Option<PathBuf> = result.ok().and_then(|file| file.path());
                let _ = sender.try_send(Message::NewChatPicked(folder));
            });
        });
        let actions = flow();
        actions.append(&create);
        actions.append(&cancel);
        actions.append(&browse);

        let form = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        form.add_css_class("panel");
        form.set_width_request(-1);
        form.append(&group("agente", &agents));
        form.append(&group("onde", &place_box));
        let folder = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        folder.append(&label("pasta", &["dim"]));
        let entry_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        entry_row.append(&entry);
        entry_row.append(&save_shortcut);
        entry_row.append(&show_shortcuts);
        folder.append(&entry_row);
        folder.append(&preview);
        folder.append(&shortcuts);
        folder.append(&remote_note);
        form.append(&folder);
        form.append(&actions);
        let root = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&form)
            .build();
        Self {
            root,
            launches,
            places,
            entry,
            preview,
            shortcuts,
            remote_note,
            create,
            browse,
            tx: tx.clone(),
            save_shortcut,
            save_target,
            save_name,
            save_note,
            save_confirm,
            show_shortcuts,
            shortcut_popover,
            shortcut_list,
            shortcuts_version: Cell::new(u64::MAX),
        }
    }

    pub fn sync(&self, app: &mut App) {
        for (launch, button) in &self.launches {
            set_selected(button, *launch == app.new_chat_launch);
        }
        let omarchy = app.new_chat_launch == Launch::Omarchy;
        for (host, button) in &self.places {
            set_selected(button, *host == app.new_chat_host);
            // Com o agente do Omarchy, só "local": ele é do sistema deste notebook.
            button.set_visible(host.is_none() || !omarchy);
        }
        if std::mem::take(&mut app.new_chat_input_reset) {
            self.entry.set_text(&app.new_chat_input);
            self.entry.set_position(-1);
        }
        let remote = app.new_chat_host.is_some();
        let placeholder = match (remote, omarchy) {
            (true, _) => "pasta na VPS (vazio = ~)",
            (false, true) => "pasta, atalho ou . (vazio = ~/Work)",
            (false, false) => "pasta, atalho ou . (vazio = a do chat em foco)",
        };
        self.entry.set_placeholder_text(Some(placeholder));
        let preview = app.new_chat_preview();
        match &preview {
            Ok(path) => {
                self.preview.set_text(&format!("{ICON_RIGHT} {path}"));
                self.preview.set_css_classes(&["dim"]);
            }
            Err(error) => {
                self.preview.set_text(error);
                self.preview.set_css_classes(&["danger"]);
            }
        }
        self.create.set_sensitive(preview.is_ok());
        self.remote_note.set_visible(remote);
        self.browse.set_visible(!remote);

        // Atalhos: só desta máquina (na VPS, a pasta vai como foi digitada).
        self.save_shortcut.set_visible(!remote);
        self.show_shortcuts.set_visible(!remote);
        self.save_shortcut.set_sensitive(!app.new_chat_input.trim().is_empty() && preview.is_ok());
        if std::mem::take(&mut app.shortcut_name_reset) {
            self.save_name.set_text(&app.shortcut_name);
            self.save_name.set_position(-1);
        }
        if let Ok(path) = &preview {
            self.save_target.set_text(&format!("atalho para {path}"));
        }
        match app.shortcut_save_check() {
            Ok((_, note)) => {
                self.save_note.set_text(note.as_deref().unwrap_or_default());
                self.save_note.set_css_classes(&["dim"]);
                self.save_note.set_visible(note.is_some());
                self.save_confirm.set_sensitive(true);
            }
            Err(error) => {
                self.save_note.set_text(&error);
                self.save_note.set_css_classes(&["danger"]);
                self.save_note.set_visible(true);
                self.save_confirm.set_sensitive(false);
            }
        }
        if self.shortcuts_version.replace(app.shortcuts_version) != app.shortcuts_version {
            let names: Vec<&str> = app.dir_shortcuts.iter().map(|shortcut| shortcut.name.as_str()).collect();
            self.shortcuts.set_text(&format!("atalhos: {}", names.join(", ")));
            self.rebuild_shortcuts(app);
        }
        self.shortcuts.set_visible(!remote && !app.dir_shortcuts.is_empty());
    }

    /// Refaz a lista do balão "atalhos": os do C-hub (usar, editar, excluir) e os do fshort (só usar).
    fn rebuild_shortcuts(&self, app: &App) {
        while let Some(child) = self.shortcut_list.first_child() {
            self.shortcut_list.remove(&child);
        }
        let own: Vec<&DirShortcut> = app.dir_shortcuts.iter().filter(|shortcut| !shortcut.fshort).collect();
        let fshort: Vec<&DirShortcut> = app.dir_shortcuts.iter().filter(|shortcut| shortcut.fshort).collect();
        let header = row!(
            &label(&format!("{ICON_BOOKMARK} "), &["accent"]),
            &label("atalhos", &["bold"]),
            &label(&format!(" {}", own.len()), &["dim"])
        );
        self.shortcut_list.append(&header);
        if let Some(error) = &app.shortcut_error {
            let error = label(error, &["danger"]);
            error.set_wrap(true);
            self.shortcut_list.append(&error);
        }
        if own.is_empty() {
            let empty = label("Nenhum atalho ainda. Digite uma pasta no campo e clique em \"salvar atalho\".", &["dim"]);
            empty.set_wrap(true);
            self.shortcut_list.append(&empty);
        }
        let mut focus = None;
        for shortcut in own {
            let item = if app.shortcut_editing.as_deref() == Some(shortcut.name.as_str()) {
                let (item, name) = self.editing_row(shortcut);
                focus = Some(name);
                item
            } else if app.shortcut_deleting.as_deref() == Some(shortcut.name.as_str()) {
                self.deleting_row(shortcut)
            } else {
                self.shortcut_row(shortcut, true)
            };
            self.shortcut_list.append(&item);
        }
        if !fshort.is_empty() {
            let title = label("do fshort (~/.shortcuts, só leitura)", &["dim"]);
            title.set_margin_top(8);
            self.shortcut_list.append(&title);
            for shortcut in fshort {
                self.shortcut_list.append(&self.shortcut_row(shortcut, false));
            }
        }
        // No ocioso: a linha em edição acabou de entrar no balão.
        if let Some(entry) = focus {
            glib::idle_add_local_once(move || {
                entry.grab_focus();
            });
        }
    }

    /// Nome e pasta; clicar põe o atalho no campo. Os do C-hub têm editar e excluir.
    fn shortcut_row(&self, shortcut: &DirShortcut, editable: bool) -> gtk4::Box {
        let name = label(&shortcut.name, &["accent"]);
        name.set_width_chars(10);
        let path = label(&claude::display_path(&shortcut.path), &["dim"]);
        path.set_ellipsize(pango::EllipsizeMode::Middle);
        path.set_hexpand(true);
        let item = row!(&name, &path);
        item.set_spacing(8);
        if editable {
            let value = shortcut.name.clone();
            item.append(&icon_button(ICON_EDIT, "Editar", &self.tx, move || Message::EditShortcut(Some(value.clone()))));
            let value = shortcut.name.clone();
            item.append(&icon_button(ICON_DELETE, "Excluir", &self.tx, move || {
                Message::AskDeleteShortcut(Some(value.clone()))
            }));
        }
        item.set_tooltip_text(Some("Usar no campo de pasta"));
        let popover = self.shortcut_popover.clone();
        let value = shortcut.name.clone();
        let sender = self.tx.clone();
        let click = gtk4::GestureClick::new();
        click.connect_released(move |gesture, _, _, _| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            popover.popdown();
            let _ = sender.try_send(Message::UseShortcut(value.clone()));
        });
        item.add_controller(click);
        item.add_css_class("chat-row");
        item.set_cursor_from_name(Some("pointer"));
        item
    }

    /// Edição: nome e pasta em campos, salvar e cancelar. Devolve também o campo do nome (foco).
    fn editing_row(&self, shortcut: &DirShortcut) -> (gtk4::Box, gtk4::Entry) {
        let name = gtk4::Entry::new();
        name.set_text(&shortcut.name);
        name.set_width_chars(10);
        name.set_tooltip_text(Some("Nome do atalho"));
        let path = gtk4::Entry::new();
        path.set_text(&claude::display_path(&shortcut.path));
        path.set_hexpand(true);
        path.set_tooltip_text(Some("Pasta (/… ou ~/…)"));
        let save = gtk4::Button::with_label("salvar");
        save.add_css_class("text");
        let old = shortcut.name.clone();
        let sender = self.tx.clone();
        let (name_value, path_value) = (name.clone(), path.clone());
        let submit = move || {
            let message = Message::UpdateShortcut(old.clone(), name_value.text().to_string(), path_value.text().to_string());
            let _ = sender.try_send(message);
        };
        let on_click = submit.clone();
        save.connect_clicked(move |_| on_click());
        let on_name = submit.clone();
        name.connect_activate(move |_| on_name());
        path.connect_activate(move |_| submit());
        let cancel = text_button("cancelar", "Desistir da edição", &self.tx, || Message::EditShortcut(None));
        let item = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        item.append(&name);
        item.append(&path);
        item.append(&save);
        item.append(&cancel);
        (item, name)
    }

    /// Confirmação antes de excluir.
    fn deleting_row(&self, shortcut: &DirShortcut) -> gtk4::Box {
        let name = label(&shortcut.name, &["danger"]);
        name.set_width_chars(10);
        let question = label(&format!("excluir ({})?", claude::display_path(&shortcut.path)), &[]);
        question.set_ellipsize(pango::EllipsizeMode::Middle);
        question.set_hexpand(true);
        let value = shortcut.name.clone();
        let yes = text_button("excluir", "Excluir o atalho", &self.tx, move || Message::DeleteShortcut(value.clone()));
        let no = text_button("não", "Manter o atalho", &self.tx, || Message::AskDeleteShortcut(None));
        let item = row!(&name, &question, &yes, &no);
        item.set_spacing(4);
        item
    }
}

/// "salvar atalho": abre um balão com o nome do atalho para a pasta do campo.
fn save_shortcut_button(tx: &Sender<Message>) -> (gtk4::MenuButton, gtk4::Label, gtk4::Entry, gtk4::Label, gtk4::Button) {
    let target = label("", &["dim"]);
    target.set_ellipsize(pango::EllipsizeMode::Middle);
    target.set_max_width_chars(48);
    let name = gtk4::Entry::new();
    name.set_placeholder_text(Some("nome do atalho (ex.: proj)"));
    name.set_width_chars(24);
    let sender = tx.clone();
    name.connect_changed(move |entry| {
        let _ = sender.try_send(Message::ShortcutNameInput(entry.text().to_string()));
    });
    let note = label("", &["dim"]);
    note.set_wrap(true);
    note.set_max_width_chars(48);
    let confirm = gtk4::Button::with_label("salvar");
    confirm.add_css_class("text");
    confirm.set_tooltip_text(Some("Salvar o atalho (Enter)"));
    let cancel = gtk4::Button::with_label("cancelar");
    cancel.add_css_class("text");
    cancel.set_tooltip_text(Some("Fechar sem salvar (Esc)"));

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    content.append(&target);
    content.append(&name);
    content.append(&note);
    let actions = flow();
    actions.append(&confirm);
    actions.append(&cancel);
    content.append(&actions);
    let popover = gtk4::Popover::new();
    popover.set_has_arrow(false);
    popover.add_css_class("menu-card");
    popover.set_child(Some(&content));

    let parent = popover.clone();
    let sender = tx.clone();
    confirm.connect_clicked(move |_| {
        parent.popdown();
        let _ = sender.try_send(Message::SaveShortcut);
    });
    let button = confirm.clone();
    name.connect_activate(move |_| {
        if button.is_sensitive() {
            button.emit_clicked();
        }
    });
    let parent = popover.clone();
    cancel.connect_clicked(move |_| parent.popdown());
    let sender = tx.clone();
    let field = name.clone();
    popover.connect_show(move |_| {
        let _ = sender.try_send(Message::ShortcutSaveOpened);
        let field = field.clone();
        glib::idle_add_local_once(move || {
            field.grab_focus();
        });
    });

    let menu = gtk4::MenuButton::builder().popover(&popover).label("salvar atalho").build();
    menu.add_css_class("text");
    menu.set_tooltip_text(Some("Dar um nome curto a esta pasta"));
    menu.set_valign(gtk4::Align::Center);
    (menu, target, name, note, confirm)
}

/// "atalhos": abre um balão com a lista dos atalhos (montada em `rebuild_shortcuts`).
fn shortcuts_button() -> (gtk4::MenuButton, gtk4::Popover, gtk4::Box) {
    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(420)
        .child(&list)
        .build();
    let popover = gtk4::Popover::new();
    popover.set_has_arrow(false);
    popover.add_css_class("menu-card");
    popover.set_child(Some(&scroll));
    popover.set_size_request(520, -1);
    let menu = gtk4::MenuButton::builder().popover(&popover).label("atalhos").build();
    menu.add_css_class("text");
    menu.set_tooltip_text(Some("Ver, editar e excluir os atalhos de pasta"));
    menu.set_valign(gtk4::Align::Center);
    (menu, popover, list)
}

/// Painel das sessões salvas: desta máquina ou de um host do SSH.
pub struct SessionsPanel {
    pub root: gtk4::Box,
    places: Vec<(Option<String>, gtk4::Button)>,
    list: gtk4::Box,
    version: Cell<u64>,
}

impl SessionsPanel {
    pub fn new(app: &App, tx: &Sender<Message>) -> Self {
        let mut places = vec![(None, choice(&format!("{ICON_LAPTOP} local"), tx, || Message::SessionsHost(None)))];
        for host in &app.hosts {
            let value = host.clone();
            let button = choice(&format!("{ICON_SERVER} {host}"), tx, move || Message::SessionsHost(Some(value.clone())));
            places.push((Some(host.clone()), button));
        }
        let tabs = flow();
        for (_, button) in &places {
            tabs.append(button);
        }
        let refresh = gtk4::Button::with_label(ICON_REFRESH);
        refresh.add_css_class("icon");
        refresh.set_tooltip_text(Some("Recarregar"));
        refresh.set_valign(gtk4::Align::Start);
        button_message(&refresh, tx, || Message::ReloadSessions);
        tabs.set_hexpand(true);
        let header = row!(&tabs, &refresh);

        let list = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();
        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        root.add_css_class("panel");
        root.append(&header);
        root.append(&scroll);
        Self { root, places, list, version: Cell::new(u64::MAX) }
    }

    pub fn sync(&self, app: &App, tx: &Sender<Message>) {
        for (host, button) in &self.places {
            set_selected(button, *host == app.sessions_host);
        }
        // Os marcadores "aberta aqui" dependem dos chats abertos: a lista de chats entra na versão.
        let version = app.sessions_version.wrapping_mul(1_000).wrapping_add(app.chats.len() as u64);
        if self.version.replace(version) == version {
            return;
        }
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        match &app.sessions_host {
            None => self.local(app, tx),
            Some(host) => self.remote(app, host, tx),
        }
    }

    fn local(&self, app: &App, tx: &Sender<Message>) {
        let Some(list) = &app.saved_sessions else {
            self.list.append(&label("carregando…", &["dim"]));
            return;
        };
        // Os tmux daqui que nenhum chat mostra: criados pelo celular ou de chats fechados.
        let loose: Vec<(usize, &remote::TmuxSession)> = app
            .local_tmux
            .iter()
            .enumerate()
            .filter(|(_, tmux)| {
                !app.chats.iter().any(|chat| chat.host.is_none() && chat.tmux.as_deref() == Some(tmux.name.as_str()) && chat.is_alive())
            })
            .collect();
        if !loose.is_empty() {
            self.list.append(&label("tmux abertos aqui", &["dim"]));
            for (index, tmux) in loose {
                self.list.append(&tmux_item(tmux, tmux.attached.then_some("conectado em outro lugar"), tx, move || {
                    Message::AttachTmux(None, index)
                }));
            }
            let gap = label("sessões salvas", &["dim"]);
            gap.set_margin_top(8);
            self.list.append(&gap);
        }
        if list.is_empty() {
            self.list.append(&label("nenhuma sessão salva encontrada.", &["dim"]));
            return;
        }
        // Agrupa por pasta; os grupos seguem a ordem da sessão mais recente de cada um.
        let mut groups: Vec<(&PathBuf, Vec<usize>)> = Vec::new();
        for (index, saved) in list.iter().enumerate() {
            match groups.iter_mut().find(|(dir, _)| **dir == saved.cwd) {
                Some((_, indices)) => indices.push(index),
                None => groups.push((&saved.cwd, vec![index])),
            }
        }
        for (dir, indices) in groups {
            let expanded = app.expanded_dirs.contains(dir);
            let path = label(&claude::display_path(dir), &["bold"]);
            path.set_ellipsize(pango::EllipsizeMode::Middle);
            path.set_hexpand(true);
            let arrow = if expanded { ICON_DOWN } else { ICON_RIGHT };
            let header = row!(&label(&format!("{arrow} "), &["accent"]), &path, &label(&format!(" {}", indices.len()), &["dim"]));
            let dir = dir.clone();
            self.list.append(&clickable(header, tx, move || Message::ToggleDirGroup(dir.clone())));
            if expanded {
                for index in indices {
                    self.list.append(&saved_item(app, &list[index], tx, move || Message::ResumeSession(index)));
                }
            }
        }
    }

    fn remote(&self, app: &App, host: &str, tx: &Sender<Message>) {
        let listing = match app.remote_sessions.get(host) {
            None | Some(RemoteState::Loading) => {
                self.list.append(&label(&format!("conectando a {host}… (a senha da chave pode ser pedida)"), &["dim"]));
                return;
            }
            Some(RemoteState::Failed(error)) => {
                let message = label(&format!("{ICON_WARNING} {error}"), &["warning"]);
                message.set_wrap(true);
                self.list.append(&message);
                let host = host.to_owned();
                let retry = text_button("tentar de novo", "Conectar de novo", tx, move || Message::SessionsHost(Some(host.clone())));
                retry.set_halign(gtk4::Align::Start);
                self.list.append(&retry);
                return;
            }
            Some(RemoteState::Loaded(listing)) => listing,
        };

        self.list.append(&label(&format!("tmux abertos em {host}"), &["dim"]));
        if listing.tmux.is_empty() {
            self.list.append(&label("  nenhum", &["dim"]));
        }
        for (index, tmux) in listing.tmux.iter().enumerate() {
            let here = app.chats.iter().any(|chat| {
                chat.host.as_deref() == Some(host) && chat.tmux.as_deref() == Some(tmux.name.as_str()) && chat.is_alive()
            });
            let note = if here {
                Some("aberto aqui")
            } else {
                tmux.attached.then_some("conectado em outro lugar")
            };
            let host = host.to_owned();
            self.list.append(&tmux_item(tmux, note, tx, move || Message::AttachTmux(Some(host.clone()), index)));
        }

        let gap = label(&format!("sessões salvas em {host}"), &["dim"]);
        gap.set_margin_top(8);
        self.list.append(&gap);
        if listing.sessions.is_empty() {
            self.list.append(&label("  nenhuma", &["dim"]));
        }
        for (index, saved) in listing.sessions.iter().enumerate() {
            let host = host.to_owned();
            self.list.append(&saved_item(app, saved, tx, move || Message::ResumeRemote(host.clone(), index)));
        }
    }
}

/// Uma sessão do tmux: status pelo título do painel, título; embaixo, nome, pasta e `note`.
fn tmux_item(
    tmux: &remote::TmuxSession,
    note: Option<&str>,
    tx: &Sender<Message>,
    message: impl Fn() -> Message + 'static,
) -> gtk4::Widget {
    let title = claude::title_from_terminal(&tmux.title).unwrap_or_else(|| tmux.title.clone());
    let mut details = vec![tmux.name.clone(), tmux.path.clone()];
    details.extend(note.map(str::to_owned));
    let (glyph, class) = match claude::status_from_title(&tmux.title) {
        Some(status) => status_glyph(status),
        None => (ICON_TMUX, "dim"),
    };
    clickable(two_lines(glyph, class, &title, &details.join(" · ")), tx, message).upcast()
}

/// Uma sessão salva: status e título; embaixo, agente, quando e onde está aberta.
fn saved_item(app: &App, saved: &SavedSession, tx: &Sender<Message>, message: impl Fn() -> Message + 'static) -> gtk4::Widget {
    let open_here = app.chat_index_for_session(saved.agent, &saved.id);
    let mut details = vec![saved.agent.key().to_owned(), sessions::relative_time(saved.modified)];
    if saved.host.is_some() {
        details.push(saved.cwd.display().to_string());
    }
    let blocked = match (&saved.running, open_here) {
        (_, Some(_)) => {
            details.push("aberta aqui".into());
            false
        }
        // Aberta em outro terminal: retomar aqui brigaria com ela.
        (Some(_), None) => {
            details.push("aberta em outro terminal".into());
            true
        }
        (None, None) => false,
    };
    let running = open_here
        .map(|index| app.chats[index].status)
        .or_else(|| saved.running.as_deref().and_then(Status::from_claude));
    let (glyph, class) = match running {
        Some(status) => status_glyph(status),
        None => (ICON_IDLE, "dim"),
    };
    let item = two_lines(glyph, class, &saved.title, &details.join(" · "));
    if blocked {
        item.add_css_class("blocked");
        item.add_css_class("chat-row");
        return item.upcast();
    }
    clickable(item, tx, message).upcast()
}

fn two_lines(glyph: &str, class: &str, title: &str, details: &str) -> gtk4::Box {
    let title = label(title, &[]);
    title.set_ellipsize(pango::EllipsizeMode::End);
    let details = label(&format!("  {details}"), &["dim"]);
    details.set_ellipsize(pango::EllipsizeMode::End);
    let item = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    item.append(&row!(&label(&format!("{glyph} "), &[class]), &title));
    item.append(&details);
    item
}

/// Linha clicável com o hover das linhas da lista de chats.
fn clickable(item: gtk4::Box, tx: &Sender<Message>, message: impl Fn() -> Message + 'static) -> gtk4::Box {
    clickable_row(&item, tx, message)
}

/// Uma linha de opções. (O GTK4 puro não tem caixa que quebra a linha; o `FlowBox` alinha as
/// opções numa grade, com espaços desiguais. Com poucos hosts, uma linha basta.)
fn flow() -> gtk4::Box {
    let flow = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    flow.set_halign(gtk4::Align::Start);
    flow
}

fn group(title: &str, content: &impl IsA<gtk4::Widget>) -> gtk4::Box {
    let group = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    group.append(&label(title, &["dim"]));
    group.append(content);
    group
}

fn set_selected(button: &gtk4::Button, selected: bool) {
    if selected {
        button.add_css_class("selected");
    } else {
        button.remove_css_class("selected");
    }
}
