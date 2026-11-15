//! Temas próprios do C-hub, para qualquer distro: opacos (sem o vidro do Omarchy) e com as
//! cores oficiais de cada esquema. O seletor fica no cabeçalho "# chats" e a escolha vai para
//! `config.rs`.
//!
//! O tema personalizado (`CUSTOM`) parte das cores do tema em uso e muda os campos de
//! `ColorField` e a opacidade; fica gravado em `config.json` (`custom_theme`).

use serde_json::{Value, json};

use crate::omarchy::{Rgb, ThemeColors};

/// Id do tema personalizado.
pub const CUSTOM: &str = "personalizado";

/// Uma cor do tema que o painel "tema" deixa mudar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorField {
    Accent,
    Background,
    Foreground,
    Muted,
    Selection,
    /// Uma das 16 cores do terminal (0–7 normais, 8–15 claras).
    Ansi(usize),
}

impl ColorField {
    /// As cores da interface, com o nome e onde aparecem, na ordem do painel.
    pub const INTERFACE: [(ColorField, &'static str, &'static str); 5] = [
        (ColorField::Accent, "destaque", "títulos, item escolhido, \"trabalhando\""),
        (ColorField::Background, "fundo", "fundo da janela e dos menus"),
        (ColorField::Foreground, "texto", "texto e cursor do terminal"),
        (ColorField::Muted, "apagado", "detalhes, ícones e bordas"),
        (ColorField::Selection, "seleção", "seleção no terminal e barras de uso"),
    ];

    /// Nomes das 8 cores do terminal; as claras usam o mesmo nome.
    pub const ANSI_NAMES: [&'static str; 8] = ["preto", "vermelho", "verde", "amarelo", "azul", "magenta", "ciano", "branco"];

    pub fn get(self, colors: &ThemeColors) -> Rgb {
        match self {
            ColorField::Accent => colors.accent,
            ColorField::Background => colors.background,
            ColorField::Foreground => colors.foreground,
            ColorField::Muted => colors.muted,
            ColorField::Selection => colors.selection,
            ColorField::Ansi(index) => colors.ansi[index % 16],
        }
    }

    pub fn set(self, colors: &mut ThemeColors, value: Rgb) {
        let value = value & 0xff_ffff;
        match self {
            ColorField::Accent => colors.accent = value,
            ColorField::Background => {
                colors.background = value;
                colors.light = luminance(value) > 0.5;
            }
            ColorField::Foreground => colors.foreground = value,
            ColorField::Muted => colors.muted = value,
            ColorField::Selection => colors.selection = value,
            ColorField::Ansi(index) => colors.ansi[index % 16] = value,
        }
    }
}

/// `#rrggbb`.
pub fn hex(color: Rgb) -> String {
    format!("#{:06x}", color & 0xff_ffff)
}

/// Opacidade mínima do tema personalizado: abaixo disso o texto some sobre o que está atrás.
pub const MIN_OPACITY: f32 = 0.3;

/// Tema personalizado para o `config.json`.
pub fn to_json(colors: &ThemeColors) -> Value {
    json!({
        "accent": hex(colors.accent),
        "selection": hex(colors.selection),
        "muted": hex(colors.muted),
        "background": hex(colors.background),
        "lighter_background": hex(colors.lighter_background),
        "foreground": hex(colors.foreground),
        "bright_foreground": hex(colors.bright_foreground),
        "ansi": colors.ansi.iter().map(|&color| hex(color)).collect::<Vec<_>>(),
        "opacity": colors.opacity,
    })
}

/// Tema personalizado lido do `config.json`; o que faltar ou estiver estragado fica com o padrão.
pub fn from_json(value: &Value) -> Option<ThemeColors> {
    value.as_object()?;
    let mut colors = find(DEFAULT).map(|builtin| builtin.colors.clone()).unwrap_or_default();
    let color = |key: &str| value.get(key).and_then(Value::as_str).and_then(crate::omarchy::parse_hex);
    let set = |target: &mut Rgb, key: &str| {
        if let Some(value) = color(key) {
            *target = value;
        }
    };
    set(&mut colors.accent, "accent");
    set(&mut colors.selection, "selection");
    set(&mut colors.muted, "muted");
    set(&mut colors.background, "background");
    set(&mut colors.lighter_background, "lighter_background");
    set(&mut colors.foreground, "foreground");
    set(&mut colors.bright_foreground, "bright_foreground");
    if let Some(ansi) = value.get("ansi").and_then(Value::as_array) {
        for (target, item) in colors.ansi.iter_mut().zip(ansi) {
            if let Some(value) = item.as_str().and_then(crate::omarchy::parse_hex) {
                *target = value;
            }
        }
    }
    if let Some(opacity) = value.get("opacity").and_then(Value::as_f64) {
        colors.opacity = (opacity as f32).clamp(MIN_OPACITY, 1.0);
    }
    colors.light = luminance(colors.background) > 0.5;
    Some(colors)
}

/// Luminância relativa (WCAG) de `0xRRGGBB`.
pub fn luminance(hex: Rgb) -> f64 {
    let channel = |shift: u32| {
        let value = f64::from((hex >> shift) & 0xff) / 255.0;
        if value <= 0.03928 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

/// Um tema embutido: id gravado na configuração, nome no seletor e cores.
pub struct Builtin {
    pub id: &'static str,
    pub name: &'static str,
    pub colors: ThemeColors,
}

/// Tema usado fora do Omarchy quando nada foi escolhido.
pub const DEFAULT: &str = "tokyo-night";

pub const BUILTINS: [Builtin; 6] = [
    Builtin {
        id: "escuro",
        name: "Escuro",
        // Básico: cinza escuro neutro, sem tom de cor no fundo, e um acento azul discreto.
        colors: ThemeColors {
            light: false,
            accent: 0x6ca0dc,
            selection: 0x3a3a3a,
            muted: 0x8a8a8a,
            background: 0x1e1e1e,
            lighter_background: 0x2a2a2a,
            foreground: 0xd4d4d4,
            bright_foreground: 0xffffff,
            ansi: [
                0x2e2e2e, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xd4d4d4, //
                0x5c5c5c, 0xef8f97, 0xb5dd96, 0xf0d197, 0x8cc5f5, 0xd9a0e8, 0x7fcfd8, 0xffffff,
            ],
            opacity: 1.0,
        },
    },
    Builtin {
        id: "tokyo-night",
        name: "Tokyo Night",
        colors: ThemeColors {
            light: false,
            accent: 0x7aa2f7,
            selection: 0x283457,
            // O `comment` (#565f89) some sobre o fundo nas linhas apagadas da interface.
            muted: 0x737aa2,
            background: 0x1a1b26,
            lighter_background: 0x24283b,
            foreground: 0xc0caf5,
            bright_foreground: 0xc0caf5,
            ansi: [
                0x15161e, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xa9b1d6, //
                0x414868, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xc0caf5,
            ],
            opacity: 1.0,
        },
    },
    Builtin {
        id: "catppuccin-mocha",
        name: "Catppuccin Mocha",
        colors: ThemeColors {
            light: false,
            accent: 0xcba6f7,
            selection: 0x45475a,
            muted: 0x7f849c,
            background: 0x1e1e2e,
            lighter_background: 0x313244,
            foreground: 0xcdd6f4,
            bright_foreground: 0xcdd6f4,
            ansi: [
                0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de, //
                0x585b70, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xa6adc8,
            ],
            opacity: 1.0,
        },
    },
    Builtin {
        id: "catppuccin-latte",
        name: "Catppuccin Latte",
        colors: ThemeColors {
            light: true,
            accent: 0x8839ef,
            selection: 0xbcc0cc,
            // O overlay1 (#8c8fa1) fica com contraste 2,8 sobre o fundo claro; o overlay2 dá 3,5.
            muted: 0x7c7f93,
            background: 0xeff1f5,
            lighter_background: 0xe6e9ef,
            foreground: 0x4c4f69,
            bright_foreground: 0x4c4f69,
            ansi: [
                0x5c5f77, 0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179299, 0xacb0be, //
                0x6c6f85, 0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179299, 0xbcc0cc,
            ],
            opacity: 1.0,
        },
    },
    Builtin {
        id: "gruvbox-dark",
        name: "Gruvbox",
        colors: ThemeColors {
            light: false,
            accent: 0xfabd2f,
            selection: 0x504945,
            muted: 0x928374,
            background: 0x282828,
            lighter_background: 0x3c3836,
            foreground: 0xebdbb2,
            bright_foreground: 0xfbf1c7,
            ansi: [
                0x282828, 0xcc241d, 0x98971a, 0xd79921, 0x458588, 0xb16286, 0x689d6a, 0xa89984, //
                0x928374, 0xfb4934, 0xb8bb26, 0xfabd2f, 0x83a598, 0xd3869b, 0x8ec07c, 0xebdbb2,
            ],
            opacity: 1.0,
        },
    },
    Builtin {
        id: "nord",
        name: "Nord",
        colors: ThemeColors {
            light: false,
            accent: 0x88c0d0,
            selection: 0x434c5e,
            // O nord3 (#4c566a) quase some sobre o fundo; este é o tom dos comentários do Nord.
            muted: 0x7b88a1,
            background: 0x2e3440,
            lighter_background: 0x3b4252,
            foreground: 0xd8dee9,
            bright_foreground: 0xeceff4,
            ansi: [
                0x3b4252, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x88c0d0, 0xe5e9f0, //
                0x4c566a, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x8fbcbb, 0xeceff4,
            ],
            opacity: 1.0,
        },
    },
];

pub fn find(id: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|builtin| builtin.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(a: u32, b: u32) -> f64 {
        let (high, low) = if luminance(a) > luminance(b) { (luminance(a), luminance(b)) } else { (luminance(b), luminance(a)) };
        (high + 0.05) / (low + 0.05)
    }

    #[test]
    fn temas_tem_contraste_para_ler() {
        assert!(find(DEFAULT).is_some());
        for builtin in &BUILTINS {
            let colors = &builtin.colors;
            assert_eq!(colors.opacity, 1.0, "{}: os temas próprios são opacos", builtin.id);
            // Texto normal: o mínimo do WCAG AA. Apagado e acento: legíveis como texto grande.
            assert!(contrast(colors.foreground, colors.background) >= 4.5, "{}: texto", builtin.id);
            assert!(contrast(colors.muted, colors.background) >= 3.0, "{}: apagado", builtin.id);
            assert!(contrast(colors.accent, colors.background) >= 3.0, "{}: acento", builtin.id);
            assert_eq!(colors.light, luminance(colors.background) > 0.5, "{}: claro/escuro", builtin.id);
        }
    }

    #[test]
    fn ids_unicos() {
        for (index, builtin) in BUILTINS.iter().enumerate() {
            assert!(BUILTINS[index + 1..].iter().all(|other| other.id != builtin.id));
        }
    }

    #[test]
    fn personalizado_ida_e_volta() {
        let mut colors = find("nord").unwrap().colors.clone();
        ColorField::Accent.set(&mut colors, 0x123456);
        ColorField::Ansi(9).set(&mut colors, 0xabcdef);
        colors.opacity = 0.8;
        assert_eq!(from_json(&to_json(&colors)), Some(colors.clone()));
        assert_eq!(ColorField::Accent.get(&colors), 0x123456);
        assert_eq!(hex(0x0a0b0c), "#0a0b0c");
        // Fundo claro vira tema claro.
        ColorField::Background.set(&mut colors, 0xfafafa);
        assert!(colors.light);
    }

    #[test]
    fn personalizado_estragado_usa_o_padrao_no_que_falta() {
        let default = &find(DEFAULT).unwrap().colors;
        let colors = from_json(&json!({"accent": "#ff0000", "muted": "azul", "ansi": ["#000001"], "opacity": 0.01})).unwrap();
        assert_eq!(colors.accent, 0xff0000);
        assert_eq!(colors.muted, default.muted);
        assert_eq!(colors.ansi[0], 0x000001);
        assert_eq!(colors.ansi[1], default.ansi[1]);
        assert_eq!(colors.opacity, MIN_OPACITY);
        assert_eq!(from_json(&json!("não é objeto")), None);
    }
}
