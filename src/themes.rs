//! Temas do Chube: o do sistema (cores do COSMIC) e três próprios. O escolhido fica em
//! `~/.config/chube/theme` e vale para a interface, os diálogos do libcosmic (um
//! `CosmicTheme` montado a partir da paleta) e as cores dos terminais.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use cosmic::cosmic_theme::palette::{Srgb, Srgba};
use cosmic::cosmic_theme::{BlurStrength, ThemeBuilder};
use cosmic::iced::Color;

use crate::claude::home_dir;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    /// Cores do tema do COSMIC, como antes dos temas.
    #[default]
    System,
    /// Monitor de fósforo: fundo preto, texto verde, bordas cinza.
    Phosphor,
    /// Interface de filme: azul neon, gradiente e transparência.
    Neon,
    /// Paleta Gruvbox: tons quentes e contraste baixo, para o dia a dia.
    Gruvbox,
}

/// Tema em uso, lido pelas funções de estilo (que não recebem o `App`).
static CURRENT: AtomicU8 = AtomicU8::new(0);

impl Theme {
    pub const ALL: [Theme; 4] = [Theme::System, Theme::Phosphor, Theme::Neon, Theme::Gruvbox];

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "sistema",
            Theme::Phosphor => "fósforo",
            Theme::Neon => "neon",
            Theme::Gruvbox => "gruvbox",
        }
    }

    /// Nome gravado no arquivo de configuração.
    fn id(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Phosphor => "phosphor",
            Theme::Neon => "neon",
            Theme::Gruvbox => "gruvbox",
        }
    }

    fn from_id(id: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|theme| theme.id() == id)
    }

    pub fn current() -> Theme {
        Theme::ALL[CURRENT.load(Ordering::Relaxed) as usize]
    }

    pub fn make_current(self) {
        let index = Theme::ALL.iter().position(|theme| *theme == self).unwrap_or(0);
        CURRENT.store(index as u8, Ordering::Relaxed);
    }

    /// Tema gravado; o do sistema se não houver (ou se o nome for desconhecido).
    pub fn load() -> Theme {
        fs::read_to_string(path())
            .ok()
            .and_then(|text| Theme::from_id(text.trim()))
            .unwrap_or_default()
    }

    pub fn save(self) -> std::io::Result<()> {
        let path = path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, format!("{}\n", self.id()))
    }

    /// Paleta própria; `None` no tema do sistema.
    pub fn palette(self) -> Option<&'static Palette> {
        match self {
            Theme::System => None,
            Theme::Phosphor => Some(&PHOSPHOR),
            Theme::Neon => Some(&NEON),
            Theme::Gruvbox => Some(&GRUVBOX),
        }
    }

    /// Tema do libcosmic: usado nos widgets dele (diálogo, dicas, gaveta de sessões).
    /// O do sistema continua acompanhando as mudanças feitas nas Configurações.
    pub fn cosmic(self) -> cosmic::Theme {
        let Some(palette) = self.palette() else {
            return cosmic::theme::system_preference();
        };
        let mut builder = ThemeBuilder::dark()
            .bg_color(srgba(palette.window))
            .text_tint(srgb(palette.text))
            .accent(srgb(palette.accent))
            .success(srgb(palette.success))
            .warning(srgb(palette.warning))
            .destructive(srgb(palette.danger));
        if let Some(neutral) = palette.neutral {
            builder = builder.neutral_tint(srgb(neutral));
        }
        builder.frosted_windows = palette.frosted;
        builder.frosted_maximized_apps = palette.frosted;
        builder.frosted = BlurStrength::Medium;
        cosmic::Theme::custom(Arc::new(builder.build()))
    }
}

fn path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("chube/theme")
}

/// Cores de um tema próprio.
pub struct Palette {
    /// Fundo da janela; com `window_end`, um gradiente na diagonal até essa cor.
    pub window: Color,
    pub window_end: Option<Color>,
    /// Fundo das seções, por cima do da janela.
    pub card: Option<Color>,
    pub border: Color,
    /// Brilho em volta das seções (sombra sem deslocamento).
    pub glow: Option<Color>,
    pub text: Color,
    pub dim: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    /// Fundo dos botões ao passar o mouse e selecionados (o alfa vem do botão).
    pub highlight: Color,
    /// Tom dos controles do libcosmic (botões, campos); `None` deixa o cinza padrão.
    pub neutral: Option<Color>,
    /// Desfoque do COSMIC atrás da janela, que fica translúcida.
    pub frosted: bool,
    pub terminal: TerminalPalette,
}

/// Cores do terminal em `0xRRGGBB`.
pub struct TerminalPalette {
    /// Cores ANSI 0–15: preto, vermelho, verde, amarelo, azul, magenta, ciano, branco e as claras.
    pub ansi: [u32; 16],
    pub foreground: u32,
    pub cursor: u32,
    /// Fundo "sem cor" (o do terminal fica transparente, mas o vídeo reverso usa esta).
    pub background: u32,
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const fn rgba(hex: u32, alpha: f32) -> Color {
    Color::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, alpha)
}

fn srgb(color: Color) -> Srgb {
    Srgb::new(color.r, color.g, color.b)
}

fn srgba(color: Color) -> Srgba {
    Srgba::new(color.r, color.g, color.b, color.a)
}

static PHOSPHOR: Palette = Palette {
    window: rgb(0x000000),
    window_end: None,
    card: None,
    border: rgb(0x5A5A5A),
    glow: None,
    text: rgb(0x33FF66),
    dim: rgba(0x33FF66, 0.5),
    // Quase branco, como o fósforo mais aceso: destaca o item selecionado do resto do texto.
    accent: rgb(0xC8FFD6),
    success: rgb(0x33FF66),
    warning: rgb(0xE6FF4D),
    danger: rgb(0xFF4D4D),
    highlight: rgb(0x33FF66),
    neutral: None,
    frosted: false,
    terminal: TerminalPalette {
        ansi: [
            0x000000, 0xFF4D4D, 0x33FF66, 0xE6FF4D, 0x2BD99F, 0x7DFFB3, 0x4DFFD2, 0xB3FFC6, //
            0x1F8F3D, 0xFF8080, 0x80FF9F, 0xF2FF99, 0x5CE6B8, 0xA6FFCC, 0x99FFE6, 0xE6FFEC,
        ],
        foreground: 0x33FF66,
        cursor: 0x33FF66,
        background: 0x000000,
    },
};

static NEON: Palette = Palette {
    window: rgba(0x020A1C, 0.66),
    window_end: Some(rgba(0x0B2352, 0.52)),
    card: Some(rgba(0x00AEFF, 0.06)),
    border: rgba(0x00D4FF, 0.85),
    glow: Some(rgba(0x00B4FF, 0.7)),
    text: rgb(0xD6F6FF),
    dim: rgba(0x7FD4FF, 0.6),
    accent: rgb(0x00E5FF),
    success: rgb(0x3DFFB5),
    warning: rgb(0xFFD166),
    danger: rgb(0xFF4F7B),
    highlight: rgb(0x00E5FF),
    neutral: Some(rgb(0x1F6FA8)),
    frosted: true,
    terminal: TerminalPalette {
        ansi: [
            0x0A1A2F, 0xFF4F7B, 0x3DFFB5, 0xFFD166, 0x3D8BFF, 0xC77DFF, 0x00E5FF, 0xBFEFFF, //
            0x3A5A7A, 0xFF7A9C, 0x7CFFCB, 0xFFE08A, 0x6FAEFF, 0xDDA6FF, 0x66F0FF, 0xEAFBFF,
        ],
        foreground: 0xD6F6FF,
        cursor: 0x00E5FF,
        background: 0x04112A,
    },
};

static GRUVBOX: Palette = Palette {
    window: rgb(0x282828),
    window_end: None,
    card: None,
    border: rgb(0x504945),
    glow: None,
    text: rgb(0xEBDBB2),
    dim: rgb(0x928374),
    accent: rgb(0xFE8019),
    success: rgb(0xB8BB26),
    warning: rgb(0xFABD2F),
    danger: rgb(0xFB4934),
    highlight: rgb(0xEBDBB2),
    neutral: Some(rgb(0x7C6F64)),
    frosted: false,
    terminal: TerminalPalette {
        ansi: [
            0x282828, 0xCC241D, 0x98971A, 0xD79921, 0x458588, 0xB16286, 0x689D6A, 0xA89984, //
            0x928374, 0xFB4934, 0xB8BB26, 0xFABD2F, 0x83A598, 0xD3869B, 0x8EC07C, 0xEBDBB2,
        ],
        foreground: 0xEBDBB2,
        cursor: 0xEBDBB2,
        background: 0x282828,
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_gravado_volta_ao_mesmo_tema() {
        for theme in Theme::ALL {
            assert_eq!(Theme::from_id(theme.id()), Some(theme));
        }
        assert_eq!(Theme::from_id("inexistente"), None);
    }

    #[test]
    fn so_o_tema_do_sistema_nao_tem_paleta() {
        for theme in Theme::ALL {
            assert_eq!(theme.palette().is_none(), theme == Theme::System);
        }
    }
}
