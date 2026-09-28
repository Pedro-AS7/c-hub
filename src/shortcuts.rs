//! Atalhos de teclado do app. Valem também com o foco num terminal: os terminais
//! recebem esta mesma tabela (`term::KEY_BINDS`) e deixam essas teclas passar sem mandá-las ao PTY.

use std::collections::HashMap;
use std::sync::LazyLock;

use cosmic::iced::keyboard::key::{Named, Physical};
use cosmic::iced::keyboard::{Key, Modifiers};
use cosmic::widget::menu::key_bind::{KeyBind, Modifier};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shortcut {
    NewChat,
    ToggleSidebar,
    ToggleChat,
    ToggleTerminal,
    /// Passa o foco do teclado entre o chat em foco e o terminal integrado.
    SwitchFocus,
    /// Chat de cima / de baixo na lista "# chats".
    PreviousChat,
    NextChat,
}

impl Shortcut {
    pub fn label(self) -> &'static str {
        match self {
            Shortcut::NewChat => "nova sessão",
            Shortcut::ToggleSidebar => "mostrar/esconder o painel",
            Shortcut::ToggleChat => "mostrar/esconder o chat",
            Shortcut::ToggleTerminal => "mostrar/esconder o terminal",
            Shortcut::SwitchFocus => "teclado: chat ↔ terminal",
            Shortcut::PreviousChat => "chat de cima na lista",
            Shortcut::NextChat => "chat de baixo na lista",
        }
    }
}

/// Teclas e ações, na ordem mostrada na ajuda.
pub const LIST: [(&str, Shortcut); 7] = [
    ("Ctrl+Shift+N", Shortcut::NewChat),
    ("Ctrl+Shift+1", Shortcut::ToggleSidebar),
    ("Ctrl+Shift+2", Shortcut::ToggleChat),
    ("Ctrl+Shift+3", Shortcut::ToggleTerminal),
    ("Ctrl+Tab", Shortcut::SwitchFocus),
    ("Ctrl+Shift+↑", Shortcut::PreviousChat),
    ("Ctrl+Shift+↓", Shortcut::NextChat),
];

pub static KEY_BINDS: LazyLock<HashMap<KeyBind, Shortcut>> = LazyLock::new(|| {
    use Modifier::{Ctrl, Shift};
    let bind = |modifiers: &[Modifier], key: Key| KeyBind {
        modifiers: modifiers.to_vec(),
        key,
    };
    // Os números valem pela tecla física: com Shift, o caractere muda conforme o layout.
    HashMap::from([
        (bind(&[Ctrl, Shift], Key::Character("n".into())), Shortcut::NewChat),
        (bind(&[Ctrl, Shift], Key::Character("1".into())), Shortcut::ToggleSidebar),
        (bind(&[Ctrl, Shift], Key::Character("2".into())), Shortcut::ToggleChat),
        (bind(&[Ctrl, Shift], Key::Character("3".into())), Shortcut::ToggleTerminal),
        (bind(&[Ctrl], Key::Named(Named::Tab)), Shortcut::SwitchFocus),
        (bind(&[Ctrl, Shift], Key::Named(Named::ArrowUp)), Shortcut::PreviousChat),
        (bind(&[Ctrl, Shift], Key::Named(Named::ArrowDown)), Shortcut::NextChat),
    ])
});

/// O atalho de uma tecla pressionada, se houver.
pub fn find(modifiers: Modifiers, key: &Key, physical_key: &Physical) -> Option<Shortcut> {
    KEY_BINDS
        .iter()
        .find(|(bind, _)| bind.matches(modifiers, key, Some(physical_key)))
        .map(|(_, shortcut)| *shortcut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::keyboard::key::Code;

    fn ctrl_shift() -> Modifiers {
        Modifiers::CTRL | Modifiers::SHIFT
    }

    #[test]
    fn acha_os_atalhos() {
        let physical = Physical::Code(Code::KeyN);
        assert_eq!(find(ctrl_shift(), &Key::Character("n".into()), &physical), Some(Shortcut::NewChat));
        // Caps Lock ou Shift deixam a letra maiúscula.
        assert_eq!(find(ctrl_shift(), &Key::Character("N".into()), &physical), Some(Shortcut::NewChat));
        assert_eq!(find(Modifiers::CTRL, &Key::Character("n".into()), &physical), None);

        let physical = Physical::Code(Code::Digit2);
        assert_eq!(find(ctrl_shift(), &Key::Character("2".into()), &physical), Some(Shortcut::ToggleChat));

        let physical = Physical::Code(Code::Tab);
        assert_eq!(find(Modifiers::CTRL, &Key::Named(Named::Tab), &physical), Some(Shortcut::SwitchFocus));
        assert_eq!(find(Modifiers::SHIFT, &Key::Named(Named::Tab), &physical), None);

        let physical = Physical::Code(Code::ArrowDown);
        assert_eq!(find(ctrl_shift(), &Key::Named(Named::ArrowDown), &physical), Some(Shortcut::NextChat));
        assert_eq!(find(Modifiers::CTRL, &Key::Named(Named::ArrowDown), &physical), None);
    }
}
