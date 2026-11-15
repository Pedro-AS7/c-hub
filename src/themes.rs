//! Cores do Chube: as do tema atual do Omarchy (`omarchy.rs`, com o vidro, relidas quando o tema
//! muda), as de um tema próprio (`palettes.rs`, opacos, para qualquer distro) ou as do tema
//! personalizado (`palettes::CUSTOM`, gravado no `config.json`).
//!
//! A paleta fica num `static` para ser lida de qualquer parte da interface (CSS e terminais).
//! Os tokens seguem o `shell.toml` do Omarchy (menus, controles e popups do shell): linhas
//! retas, seleção com fundo do texto a 8% e texto no acento, bordas do texto a 25–40%.

use std::sync::{Arc, RwLock};

use crate::omarchy::{Rgb, ThemeColors};

/// Cor RGBA com componentes de 0 a 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// `rgba(…)` para o CSS do GTK.
    pub fn css(self) -> String {
        let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("rgba({}, {}, {}, {:.3})", byte(self.r), byte(self.g), byte(self.b), self.a)
    }

    pub fn rgba(self) -> gtk4::gdk::RGBA {
        gtk4::gdk::RGBA::new(self.r, self.g, self.b, self.a)
    }
}

/// Paleta em uso.
pub struct Palette {
    /// Fundo da janela, opaco; `alpha` é a opacidade do modo vidro.
    pub background: Color,
    pub alpha: f32,
    pub selection: Color,
    pub text: Color,
    pub dim: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    /// Tamanho da fonte do terminal (a do foot do Omarchy), em pixels.
    pub font_px: f32,
    pub colors: ThemeColors,
}

impl Palette {
    pub fn new(colors: ThemeColors, font_px: f32) -> Self {
        Self {
            background: rgb(colors.background),
            alpha: colors.opacity,
            selection: rgb(colors.selection),
            text: rgb(colors.foreground),
            dim: rgb(colors.muted),
            accent: rgb(colors.accent),
            danger: rgb(colors.ansi[1]),
            success: rgb(success_color(&colors)),
            warning: rgb(colors.ansi[3]),
            font_px,
            colors,
        }
    }

    /// Fundo da janela com a opacidade do modo vidro (o Hyprland desfoca o que fica atrás).
    pub fn window(&self) -> Color {
        Color { a: self.alpha, ..self.background }
    }
}

/// Cor do "terminou": o verde do tema, a menos que ele se confunda com o amarelo ("precisa de
/// você") ou o vermelho. No Starship o verde é laranja, quase o amarelo; aí vale a primeira cor
/// do tema que se distinga das duas (e um pouco do acento, que marca o "trabalhando").
fn success_color(colors: &ThemeColors) -> Rgb {
    let [_, red, green, yellow, blue, _, cyan, ..] = colors.ansi;
    let bright_green = colors.ansi[10];
    let distinct = |color: Rgb| {
        let Some(hue) = hue_of(color) else { return false };
        let far = |other: Rgb, min: f32| hue_of(other).is_none_or(|other| hue_distance(hue, other) >= min);
        far(yellow, 45.0) && far(red, 45.0) && far(colors.accent, 20.0)
    };
    [green, bright_green, cyan, blue].into_iter().find(|&color| distinct(color)).unwrap_or(green)
}

/// Matiz (0–360) de uma cor com saturação suficiente para a matiz contar; cinzas não têm.
fn hue_of(color: Rgb) -> Option<f32> {
    let [r, g, b] = [(color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff].map(|c| c as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let lightness = (max + min) / 2.0;
    let saturation = if delta == 0.0 { 0.0 } else { delta / (1.0 - (2.0 * lightness - 1.0).abs()) };
    if saturation < 0.25 {
        return None;
    }
    let hue = if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    Some(hue)
}

fn hue_distance(a: f32, b: f32) -> f32 {
    let diff = (a - b).abs() % 360.0;
    diff.min(360.0 - diff)
}

static CURRENT: RwLock<Option<Arc<Palette>>> = RwLock::new(None);
/// Tema escolhido: `OMARCHY`, o id de um tema próprio ou `palettes::CUSTOM`.
static CHOICE: RwLock<Option<String>> = RwLock::new(None);
/// Tema personalizado: `None` antes de ler o `config.json`; `Some(None)` se não há um.
static CUSTOM_COLORS: RwLock<Option<Option<ThemeColors>>> = RwLock::new(None);

/// Seguir o tema do Omarchy (só oferecido onde ele existe).
pub const OMARCHY: &str = "omarchy";

/// Temas que o seletor oferece: (id, nome).
pub fn available() -> Vec<(&'static str, &'static str)> {
    let mut list = Vec::new();
    if crate::omarchy::installed() {
        list.push((OMARCHY, "seguir o Omarchy"));
    }
    list.extend(crate::palettes::BUILTINS.iter().map(|builtin| (builtin.id, builtin.name)));
    list.push((crate::palettes::CUSTOM, "personalizado"));
    list
}

/// O tema personalizado gravado, se houver.
pub fn custom() -> Option<ThemeColors> {
    if let Some(custom) = CUSTOM_COLORS.read().ok().and_then(|custom| custom.clone()) {
        return custom;
    }
    let custom = crate::config::get("custom_theme").and_then(|value| crate::palettes::from_json(&value));
    if let Ok(mut current) = CUSTOM_COLORS.write() {
        *current = Some(custom.clone());
    }
    custom
}

/// Troca o tema personalizado (o chamador o grava e relê a paleta).
pub fn set_custom(colors: ThemeColors) {
    if let Ok(mut current) = CUSTOM_COLORS.write() {
        *current = Some(Some(colors));
    }
}

/// Tema em uso: o gravado, se ainda existir aqui; senão o do Omarchy, onde houver, ou o padrão.
pub fn choice() -> String {
    if let Some(choice) = CHOICE.read().ok().and_then(|choice| choice.clone()) {
        return choice;
    }
    let choice = resolve(crate::config::theme().as_deref(), crate::omarchy::installed(), custom().is_some());
    if let Ok(mut current) = CHOICE.write() {
        *current = Some(choice.clone());
    }
    choice
}

/// Troca o tema (o chamador grava a escolha e relê a paleta).
pub fn set_choice(id: &str) {
    if let Ok(mut current) = CHOICE.write() {
        *current = Some(id.to_owned());
    }
}

/// Escolha válida a partir da gravada: um tema que não existe mais, ou "seguir o Omarchy" numa
/// máquina sem ele, ou o personalizado sem um gravado, volta ao padrão.
fn resolve(saved: Option<&str>, omarchy: bool, custom: bool) -> String {
    match saved {
        Some(OMARCHY) if omarchy => OMARCHY.to_owned(),
        Some(crate::palettes::CUSTOM) if custom => crate::palettes::CUSTOM.to_owned(),
        Some(id) if crate::palettes::find(id).is_some() => id.to_owned(),
        _ if omarchy => OMARCHY.to_owned(),
        _ => crate::palettes::DEFAULT.to_owned(),
    }
}

/// Cores do tema escolhido.
fn colors() -> ThemeColors {
    let choice = choice();
    if choice == crate::palettes::CUSTOM
        && let Some(custom) = custom()
    {
        return custom;
    }
    match crate::palettes::find(&choice) {
        Some(builtin) => builtin.colors.clone(),
        None => ThemeColors::load(),
    }
}

/// Paleta em uso; lê o tema do Omarchy na primeira chamada.
pub fn current() -> Arc<Palette> {
    if let Some(palette) = CURRENT.read().ok().and_then(|current| current.clone()) {
        return palette;
    }
    reload()
}

/// Relê o tema (depois de um `omarchy theme set`, do modo vidro ou de uma troca no seletor).
pub fn reload() -> Arc<Palette> {
    let palette = Arc::new(Palette::new(colors(), crate::omarchy::terminal_font_px()));
    if let Ok(mut current) = CURRENT.write() {
        *current = Some(palette.clone());
    }
    palette
}

pub const fn rgb(hex: Rgb) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escolha_valida() {
        use crate::palettes::{CUSTOM, DEFAULT};
        assert_eq!(resolve(None, true, false), OMARCHY);
        assert_eq!(resolve(None, false, false), DEFAULT);
        assert_eq!(resolve(Some("nord"), true, false), "nord");
        // Configuração copiada de uma máquina com Omarchy para uma sem ele.
        assert_eq!(resolve(Some(OMARCHY), false, false), DEFAULT);
        assert_eq!(resolve(Some("tema-removido"), false, false), DEFAULT);
        assert_eq!(resolve(Some("tema-removido"), true, false), OMARCHY);
        // O personalizado só vale se estiver gravado.
        assert_eq!(resolve(Some(CUSTOM), true, true), CUSTOM);
        assert_eq!(resolve(Some(CUSTOM), true, false), OMARCHY);
        assert_eq!(resolve(Some(CUSTOM), false, false), DEFAULT);
    }

    #[test]
    fn paleta_segue_o_tema() {
        let colors = ThemeColors { accent: 0x7aa2f7, opacity: 0.65, ..ThemeColors::default() };
        let palette = Palette::new(colors, 12.0);
        assert_eq!(palette.accent, rgb(0x7aa2f7));
        assert_eq!(palette.window().a, 0.65);
        assert_eq!(palette.background.a, 1.0);
        // O verde do Starship é laranja, quase o amarelo do "precisa de você": o "terminou" fica
        // no ciano do tema.
        assert_eq!(palette.success, rgb(0xa6dcef));
    }

    #[test]
    fn terminou_se_distingue_do_aviso_em_todos_os_temas() {
        for builtin in &crate::palettes::BUILTINS {
            let colors = &builtin.colors;
            let success = success_color(colors);
            let (Some(done), Some(warning)) = (hue_of(success), hue_of(colors.ansi[3])) else {
                panic!("{}: sem matiz", builtin.id);
            };
            assert!(hue_distance(done, warning) >= 45.0, "{}: terminou parecido com o aviso", builtin.id);
        }
        // Onde o verde já é verde, ele fica.
        let tokyo = &crate::palettes::find("tokyo-night").unwrap().colors;
        assert_eq!(success_color(tokyo), tokyo.ansi[2]);
    }
}
