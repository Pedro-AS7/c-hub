//! Temas do Chube: o do sistema (cores do COSMIC) e sete próprios. O escolhido fica em
//! `~/.config/chube/theme` (e o desfoque do fundo, ligado ou não, em `~/.config/chube/blur`) e
//! vale para a interface, os diálogos do libcosmic (um `CosmicTheme` montado a partir da paleta)
//! e as cores dos terminais.
//!
//! No GNOME, quem desfoca atrás da janela é a extensão `desfoque-chube@pedro` (em `gnome/`), a
//! mesma camada de vidro fosco da dock, do painel e dos menus. O Chube grava o pedido em
//! `~/.config/chube/desfoque` (`save_blur_request`), que a extensão acompanha.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

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
    /// Vidro claro, como o tema "Vidro" da dock, do painel e dos menus.
    Glass,
    /// Vidro escuro, como o "Fumê" dos menus.
    Smoke,
    /// Sem fundo: só o desfoque (e um escurecido leve dele) atrás do texto.
    Transparent,
    /// Cinza opaco, como o "Grafite" da dock, do painel e dos menus.
    Graphite,
}

/// Tema em uso, lido pelas funções de estilo (que não recebem o `App`).
static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Desfoque do fundo ligado, lido pelas mesmas funções.
static BLUR: AtomicBool = AtomicBool::new(true);

/// Com o desfoque desligado, a janela não pede o blur ao compositor e o fundo é pintado opaco.
pub fn blur() -> bool {
    BLUR.load(Ordering::Relaxed)
}

pub fn set_blur(on: bool) {
    BLUR.store(on, Ordering::Relaxed);
}

/// Desfoque gravado; ligado se não houver.
pub fn load_blur() -> bool {
    fs::read_to_string(config_dir().join("blur")).map_or(true, |text| text.trim() != "off")
}

pub fn save_blur(on: bool) -> std::io::Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("blur"), if on { "on\n" } else { "off\n" })
}

impl Theme {
    pub const ALL: [Theme; 8] = [
        Theme::System,
        Theme::Phosphor,
        Theme::Neon,
        Theme::Gruvbox,
        Theme::Glass,
        Theme::Smoke,
        Theme::Transparent,
        Theme::Graphite,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "sistema",
            Theme::Phosphor => "fósforo",
            Theme::Neon => "neon",
            Theme::Gruvbox => "gruvbox",
            Theme::Glass => "vidro",
            Theme::Smoke => "fumê",
            Theme::Transparent => "transparente",
            Theme::Graphite => "grafite",
        }
    }

    /// Nome gravado no arquivo de configuração.
    fn id(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Phosphor => "phosphor",
            Theme::Neon => "neon",
            Theme::Gruvbox => "gruvbox",
            Theme::Glass => "glass",
            Theme::Smoke => "smoke",
            Theme::Transparent => "transparent",
            Theme::Graphite => "graphite",
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
        fs::read_to_string(config_dir().join("theme"))
            .ok()
            .and_then(|text| Theme::from_id(text.trim()))
            .unwrap_or_default()
    }

    pub fn save(self) -> std::io::Result<()> {
        let dir = config_dir();
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("theme"), format!("{}\n", self.id()))
    }

    /// Paleta própria; `None` no tema do sistema.
    pub fn palette(self) -> Option<&'static Palette> {
        match self {
            Theme::System => None,
            Theme::Phosphor => Some(&PHOSPHOR),
            Theme::Neon => Some(&NEON),
            Theme::Gruvbox => Some(&GRUVBOX),
            Theme::Glass => Some(&GLASS),
            Theme::Smoke => Some(&SMOKE),
            Theme::Transparent => Some(&TRANSPARENT),
            Theme::Graphite => Some(&GRAPHITE),
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
        builder.frosted_windows = palette.blur.is_some();
        builder.frosted_maximized_apps = palette.blur.is_some();
        builder.frosted = BlurStrength::Medium;
        cosmic::Theme::custom(Arc::new(builder.build()))
    }
}

/// Pedido de desfoque para a extensão do GNOME: `on <raio em px> <brilho em %> <cantos em px>`
/// ou `off`. Os cantos são os da janela, que o libcosmic arredonda (sem os cantos, o desfoque
/// sairia quadrado para fora deles). Só grava se mudou: a extensão relê a cada mudança.
pub fn save_blur_request(request: Option<Blur>, corner: f32) -> std::io::Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join("desfoque");
    let text = match request {
        Some(blur) => format!("on {} {} {corner}\n", blur.radius, blur.brightness),
        None => "off\n".to_owned(),
    };
    if fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    // Arquivo temporário + rename: a extensão nunca lê um arquivo pela metade.
    let temp = dir.join(".desfoque.tmp");
    fs::write(&temp, text)?;
    fs::rename(temp, path)
}

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("chube")
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
    /// Desfoque atrás da janela, que fica translúcida (no COSMIC, o do compositor; no GNOME, o
    /// da extensão). `None`: tema opaco.
    pub blur: Option<Blur>,
    pub terminal: TerminalPalette,
}

/// Intensidade do desfoque, nas unidades dos ajustes da dock (`desfoque-intensidade` e
/// `desfoque-brilho`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blur {
    pub radius: u32,
    /// 100 deixa como está; menos escurece o que aparece atrás.
    pub brightness: u32,
}

/// O mesmo da dock e do painel (padrão das extensões `temas-*@pedro`).
pub const DOCK_BLUR: Blur = Blur { radius: 30, brightness: 85 };

impl Palette {
    /// A cor como ficaria sobre o fundo do terminal do tema: o fundo opaco, com o desfoque
    /// desligado e nos menus. Só tirar o alfa deixaria o vidro (branco translúcido) branco.
    pub fn solid(&self, color: Color) -> Color {
        let base = rgb(self.terminal.background);
        let mix = |top: f32, bottom: f32| top * color.a + bottom * (1.0 - color.a);
        Color::from_rgb(mix(color.r, base.r), mix(color.g, base.g), mix(color.b, base.b))
    }
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
    blur: None,
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
    blur: Some(DOCK_BLUR),
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
    blur: None,
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

// Os quatro abaixo seguem os temas de mesmo nome da dock, do painel e dos menus do GNOME
// (`temas.json` das extensões `temas-*@pedro`). A paleta ANSI é a do GNOME Terminal (Tango),
// mais clara no vidro, onde o fundo atrás é claro.

static GLASS: Palette = Palette {
    window: rgba(0xFFFFFF, 0.14),
    window_end: Some(rgba(0xFFFFFF, 0.06)),
    card: None,
    border: rgba(0xFFFFFF, 0.22),
    glow: None,
    text: rgb(0xFFFFFF),
    dim: rgba(0xFFFFFF, 0.62),
    // Azul do GNOME (o dos botões ligados nos menus), mais claro para ler sobre o vidro.
    accent: rgb(0x99C1F1),
    success: rgb(0x8FF0A4),
    warning: rgb(0xF9F06B),
    danger: rgb(0xFF7B63),
    highlight: rgb(0xFFFFFF),
    neutral: None,
    // Um pouco mais escuro que o da dock: atrás de texto miúdo, um papel de parede claro apagaria
    // as cores do terminal.
    blur: Some(Blur { radius: 30, brightness: 70 }),
    terminal: TerminalPalette {
        ansi: [
            0x3D3846, 0xFF7B63, 0x8FF0A4, 0xF9F06B, 0x99C1F1, 0xDC8ADD, 0x93DDC2, 0xDEDDDA, //
            0x9A9996, 0xFFA38C, 0xB5F5C4, 0xFBF5A0, 0xC0D8F6, 0xEAB3EB, 0xB9E9D8, 0xFFFFFF,
        ],
        foreground: 0xFFFFFF,
        cursor: 0xFFFFFF,
        background: 0x3A3A3C,
    },
};

static SMOKE: Palette = Palette {
    window: rgba(0x181818, 0.55),
    window_end: None,
    card: None,
    border: rgba(0xFFFFFF, 0.14),
    glow: None,
    text: rgb(0xF2F2F2),
    dim: rgba(0xFFFFFF, 0.5),
    accent: rgb(0x78AEED),
    success: rgb(0x8FF0A4),
    warning: rgb(0xF8E45C),
    danger: rgb(0xF66151),
    highlight: rgb(0xFFFFFF),
    neutral: None,
    blur: Some(DOCK_BLUR),
    terminal: TerminalPalette {
        ansi: [
            0x241F31, 0xF66151, 0x57E389, 0xF8E45C, 0x62A0EA, 0xC061CB, 0x5BC8AF, 0xDEDDDA, //
            0x77767B, 0xFF7B63, 0x8FF0A4, 0xF9F06B, 0x99C1F1, 0xDC8ADD, 0x93DDC2, 0xFFFFFF,
        ],
        foreground: 0xF2F2F2,
        cursor: 0xF2F2F2,
        background: 0x181818,
    },
};

static TRANSPARENT: Palette = Palette {
    window: rgba(0x000000, 0.0),
    window_end: None,
    card: None,
    border: rgba(0xFFFFFF, 0.28),
    glow: None,
    text: rgb(0xFFFFFF),
    dim: rgba(0xFFFFFF, 0.6),
    accent: rgb(0x99C1F1),
    success: rgb(0x8FF0A4),
    warning: rgb(0xF9F06B),
    danger: rgb(0xFF7B63),
    highlight: rgb(0xFFFFFF),
    neutral: None,
    // Sem fundo nenhum, o texto fica legível por um desfoque mais forte e mais escuro.
    blur: Some(Blur { radius: 40, brightness: 60 }),
    terminal: TerminalPalette {
        ansi: [
            0x3D3846, 0xFF7B63, 0x8FF0A4, 0xF9F06B, 0x99C1F1, 0xDC8ADD, 0x93DDC2, 0xDEDDDA, //
            0x9A9996, 0xFFA38C, 0xB5F5C4, 0xFBF5A0, 0xC0D8F6, 0xEAB3EB, 0xB9E9D8, 0xFFFFFF,
        ],
        foreground: 0xFFFFFF,
        cursor: 0xFFFFFF,
        background: 0x1E1E1E,
    },
};

static GRAPHITE: Palette = Palette {
    window: rgb(0x2B2B2B),
    window_end: None,
    card: None,
    border: rgb(0x474747),
    glow: None,
    text: rgb(0xE6E6E6),
    dim: rgb(0x9A9A9A),
    accent: rgb(0x78AEED),
    success: rgb(0x57E389),
    warning: rgb(0xF8E45C),
    danger: rgb(0xF66151),
    highlight: rgb(0xFFFFFF),
    neutral: Some(rgb(0x5E5E5E)),
    blur: None,
    terminal: TerminalPalette {
        ansi: [
            0x241F31, 0xC01C28, 0x2EC27E, 0xF5C211, 0x1E78E4, 0x9841BB, 0x0AB9DC, 0xC0BFBC, //
            0x5E5C64, 0xED333B, 0x57E389, 0xF8E45C, 0x51A1FF, 0xC061CB, 0x4FD2FD, 0xF6F5F4,
        ],
        foreground: 0xE6E6E6,
        cursor: 0xE6E6E6,
        background: 0x2B2B2B,
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

    #[test]
    fn cor_solida_e_a_mistura_com_o_fundo_do_terminal() {
        let glass = Theme::Glass.palette().unwrap();
        let solid = glass.solid(glass.window);
        assert_eq!(solid.a, 1.0);
        // 14% de branco sobre o cinza do terminal: mais claro que ele, longe do branco.
        assert!(solid.r > rgb(glass.terminal.background).r && solid.r < 0.4);
        let graphite = Theme::Graphite.palette().unwrap();
        assert_eq!(graphite.solid(graphite.window), graphite.window);
    }
}
