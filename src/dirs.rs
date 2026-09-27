//! Resolve o diretório digitado para uma sessão nova.
//!
//! Aceita `.` (pasta do chat em foco), `~`, caminhos absolutos ou relativos à pasta do chat
//! em foco, e os atalhos do `fshort` (`~/.shortcuts`, linhas `nome=/caminho`), inclusive
//! com subpastas: `mvp/src` vira `<atalho mvp>/src`.

use std::path::{Path, PathBuf};

use crate::claude::home_dir;

/// Os atalhos do `fshort`, na ordem do arquivo.
pub fn shortcuts() -> Vec<(String, PathBuf)> {
    let file = std::env::var_os("SHORTCUTS_FILE")
        .map_or_else(|| home_dir().join(".shortcuts"), PathBuf::from);
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, path)| (name.to_owned(), PathBuf::from(path)))
        .collect()
}

/// Converte o texto digitado num diretório existente.
pub fn resolve(input: &str, focused_dir: &Path) -> Result<PathBuf, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Digite um diretório, um atalho do fshort ou \".\"".into());
    }

    let path = if input == "~" {
        home_dir()
    } else if let Some(rest) = input.strip_prefix("~/") {
        home_dir().join(rest)
    } else if Path::new(input).is_absolute() {
        PathBuf::from(input)
    } else if let Some(path) = from_shortcut(input) {
        path
    } else {
        focused_dir.join(input)
    };

    let path = path
        .canonicalize()
        .map_err(|_| format!("{} não existe", path.display()))?;
    if path.is_dir() {
        Ok(path)
    } else {
        Err(format!("{} não é um diretório", path.display()))
    }
}

/// `mvp` ou `mvp/sub/pasta`, se `mvp` for um atalho. Atalhos têm prioridade sobre
/// pastas de mesmo nome dentro da pasta do chat em foco, como no `go` do terminal.
fn from_shortcut(input: &str) -> Option<PathBuf> {
    let (name, rest) = input.split_once('/').unwrap_or((input, ""));
    let (_, base) = shortcuts().into_iter().find(|(shortcut, _)| shortcut == name)?;
    Some(if rest.is_empty() { base } else { base.join(rest) })
}

