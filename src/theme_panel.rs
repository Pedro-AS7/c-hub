//! Painel "tema", no lugar do chat: escolhe o tema e muda as cores dele. Mudar qualquer cor (ou a
//! opacidade) cria o tema personalizado a partir do tema em uso, como os temas prontos fazem:
//! destaque, fundo, texto, apagado, seleção e as 16 cores do terminal.
//!
//! Montado uma vez, como o de nova sessão: `sync` só copia as cores do tema em uso para os
//! botões e campos. Enquanto copia, `syncing` cala os sinais (senão cada cópia viraria uma
//! mudança de cor).

use std::cell::Cell;
use std::rc::Rc;

use async_channel::Sender;
use gtk4::prelude::*;

use crate::app::Message;
use crate::omarchy::{Rgb, ThemeColors};
use crate::palettes::{self, ColorField};
use crate::themes;
use crate::ui::{choice, label};

pub struct ThemePanel {
    pub root: gtk4::ScrolledWindow,
    themes: Vec<(&'static str, gtk4::Button)>,
    hint: gtk4::Label,
    /// Botão de cor e, nas cores da interface, o campo `#rrggbb` de cada campo.
    fields: Vec<(ColorField, gtk4::ColorDialogButton, Option<gtk4::Entry>)>,
    opacity: gtk4::Scale,
    opacity_value: gtk4::Label,
    syncing: Rc<Cell<bool>>,
}

impl ThemePanel {
    pub fn new(tx: &Sender<Message>) -> Self {
        let syncing = Rc::new(Cell::new(false));

        let choices = gtk4::FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .max_children_per_line(12)
            .row_spacing(4)
            .column_spacing(4)
            .halign(gtk4::Align::Start)
            .build();
        let mut themes = Vec::new();
        for (id, name) in themes::available() {
            let button = choice(name, tx, move || Message::SetTheme(id.to_owned()));
            choices.append(&button);
            themes.push((id, button));
        }
        let hint = label("", &["dim"]);
        hint.set_wrap(true);

        let mut fields = Vec::new();
        let interface = gtk4::Grid::builder().row_spacing(4).column_spacing(8).build();
        for (row, (field, name, help)) in ColorField::INTERFACE.into_iter().enumerate() {
            let button = color_button(&format!("Cor: {name}"), help, field, tx, &syncing);
            let entry = hex_entry(field, tx, &syncing);
            let name = label(name, &[]);
            name.set_width_chars(10);
            let row = row as i32;
            interface.attach(&name, 0, row, 1, 1);
            interface.attach(&button, 1, row, 1, 1);
            interface.attach(&entry, 2, row, 1, 1);
            interface.attach(&label(help, &["dim"]), 3, row, 1, 1);
            fields.push((field, button, Some(entry)));
        }

        // As 16 cores do terminal: uma linha por cor, a normal e a clara lado a lado.
        let terminal = gtk4::Grid::builder().row_spacing(4).column_spacing(8).build();
        terminal.attach(&label("normal", &["dim"]), 1, 0, 1, 1);
        terminal.attach(&label("clara", &["dim"]), 2, 0, 1, 1);
        for (index, name) in ColorField::ANSI_NAMES.into_iter().enumerate() {
            let row = index as i32 + 1;
            let title = label(name, &[]);
            title.set_width_chars(10);
            terminal.attach(&title, 0, row, 1, 1);
            for (column, (offset, variant)) in [(0, ""), (8, " claro")].into_iter().enumerate() {
                let field = ColorField::Ansi(index + offset);
                let tip = format!("{name}{variant} do terminal");
                let button = color_button(&format!("Cor: {name}{variant}"), &tip, field, tx, &syncing);
                terminal.attach(&button, column as i32 + 1, row, 1, 1);
                fields.push((field, button, None));
            }
        }
        let terminal_note = label(
            "O vermelho também marca erros e o amarelo, \"precisa de você\"; \"terminou\" usa o verde, ou a \
             primeira cor que se distinga do amarelo e do vermelho.",
            &["dim"],
        );
        terminal_note.set_wrap(true);

        let opacity = gtk4::Scale::with_range(
            gtk4::Orientation::Horizontal,
            f64::from(palettes::MIN_OPACITY * 100.0),
            100.0,
            1.0,
        );
        opacity.set_draw_value(false);
        opacity.set_hexpand(true);
        opacity.set_tooltip_text(Some("Opacidade do fundo da janela"));
        let sender = tx.clone();
        let quiet = syncing.clone();
        opacity.connect_value_changed(move |scale| {
            if !quiet.get() {
                let _ = sender.try_send(Message::SetThemeOpacity(scale.value() as f32 / 100.0));
            }
        });
        let opacity_value = label("", &[]);
        opacity_value.set_width_chars(5);
        let opacity_name = label("opacidade", &[]);
        opacity_name.set_width_chars(10);
        let opacity_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        opacity_row.append(&opacity_name);
        opacity_row.append(&opacity);
        opacity_row.append(&opacity_value);
        let opacity_note = label("Abaixo de 100%, o compositor (o Hyprland, no Omarchy) desfoca o que fica atrás.", &["dim"]);
        opacity_note.set_wrap(true);

        let form = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        form.add_css_class("panel");
        form.append(&group("tema", &[choices.upcast(), hint.clone().upcast()]));
        form.append(&group("interface", &[interface.upcast()]));
        form.append(&group("terminal", &[terminal.upcast(), terminal_note.upcast()]));
        form.append(&group("fundo", &[opacity_row.upcast(), opacity_note.upcast()]));
        let root = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&form)
            .build();
        Self { root, themes, hint, fields, opacity, opacity_value, syncing }
    }

    /// Copia o tema em uso para os controles.
    pub fn sync(&self) {
        let choice = themes::choice();
        let palette = themes::current();
        let colors: &ThemeColors = &palette.colors;
        self.syncing.set(true);
        for (id, button) in &self.themes {
            if *id == choice {
                button.add_css_class("selected");
            } else {
                button.remove_css_class("selected");
            }
        }
        let hint = if choice == palettes::CUSTOM {
            "Editando o tema personalizado. As mudanças valem na hora e ficam salvas."
        } else if themes::custom().is_some() {
            "Mudar uma cor cria o tema personalizado a partir deste (e substitui o personalizado salvo)."
        } else {
            "Mudar uma cor cria o tema personalizado a partir deste."
        };
        self.hint.set_text(hint);
        for (field, button, entry) in &self.fields {
            let value = field.get(colors);
            if to_rgb(&button.rgba()) != value {
                button.set_rgba(&themes::rgb(value).rgba());
            }
            // O campo em uso fica como está: o texto pela metade ainda não é uma cor.
            if let Some(entry) = entry
                && !entry.state_flags().contains(gtk4::StateFlags::FOCUS_WITHIN)
                && crate::omarchy::parse_hex(&entry.text()) != Some(value)
            {
                entry.set_text(&palettes::hex(value));
                entry.remove_css_class("error");
            }
        }
        let percent = f64::from((colors.opacity * 100.0).round());
        if (self.opacity.value() - percent).abs() >= 0.5 {
            self.opacity.set_value(percent);
        }
        self.opacity_value.set_text(&format!("{percent:.0}%"));
        self.syncing.set(false);
    }
}

/// Botão com a amostra da cor; abre o seletor de cores do GTK.
fn color_button(
    title: &str,
    help: &str,
    field: ColorField,
    tx: &Sender<Message>,
    syncing: &Rc<Cell<bool>>,
) -> gtk4::ColorDialogButton {
    let dialog = gtk4::ColorDialog::builder().title(title).modal(true).with_alpha(false).build();
    let button = gtk4::ColorDialogButton::new(Some(dialog));
    button.set_tooltip_text(Some(help));
    button.set_valign(gtk4::Align::Center);
    button.set_halign(gtk4::Align::Start);
    let sender = tx.clone();
    let quiet = syncing.clone();
    button.connect_rgba_notify(move |button| {
        if !quiet.get() {
            let _ = sender.try_send(Message::SetThemeColor(field, to_rgb(&button.rgba())));
        }
    });
    button
}

/// Campo `#rrggbb`: cada cor válida digitada já vale.
fn hex_entry(field: ColorField, tx: &Sender<Message>, syncing: &Rc<Cell<bool>>) -> gtk4::Entry {
    let entry = gtk4::Entry::new();
    entry.set_width_chars(8);
    entry.set_max_length(7);
    entry.set_valign(gtk4::Align::Center);
    entry.set_tooltip_text(Some("Cor em hexadecimal (#rrggbb)"));
    let sender = tx.clone();
    let quiet = syncing.clone();
    entry.connect_changed(move |entry| {
        if quiet.get() {
            return;
        }
        match crate::omarchy::parse_hex(&entry.text()) {
            Some(value) => {
                entry.remove_css_class("error");
                let _ = sender.try_send(Message::SetThemeColor(field, value));
            }
            None => entry.add_css_class("error"),
        }
    });
    entry
}

fn to_rgb(color: &gtk4::gdk::RGBA) -> Rgb {
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    (byte(color.red()) << 16) | (byte(color.green()) << 8) | byte(color.blue())
}

fn group(title: &str, children: &[gtk4::Widget]) -> gtk4::Box {
    let group = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    group.append(&label(title, &["dim"]));
    for child in children {
        group.append(child);
    }
    group
}
