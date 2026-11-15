//! Atalhos de pasta do próprio C-hub: um nome curto que o campo de pasta da nova sessão aceita
//! no lugar do caminho (`proj` ou `proj/src`). Ficam em `config.json` (`shortcuts`, lista de
//! `{name, path}`), para valer em qualquer máquina, com ou sem o `fshort`.
//!
//! Onde houver o `fshort` (`~/.shortcuts`, linhas `nome=/caminho`), os atalhos dele também valem;
//! um atalho do C-hub com o mesmo nome tem prioridade. Editar ou excluir um do fshort muda o
//! `~/.shortcuts` (no formato e com a regra de nome do `fshort`): vale também para o `go` do terminal.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::claude::home_dir;

/// Um atalho e de onde ele vem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirShortcut {
    pub name: String,
    pub path: PathBuf,
    /// Veio do `~/.shortcuts` do fshort: não é editado pelo app.
    pub fshort: bool,
}

/// Os atalhos do C-hub, em ordem alfabética.
pub fn own() -> Vec<(String, PathBuf)> {
    crate::config::get("shortcuts").map(|value| from_json(&value)).unwrap_or_default()
}

/// Grava os atalhos do C-hub (em ordem alfabética).
pub fn save(list: &[(String, PathBuf)]) -> std::io::Result<()> {
    crate::config::set("shortcuts", to_json(list))
}

fn fshort_file() -> PathBuf {
    std::env::var_os("SHORTCUTS_FILE").map_or_else(|| home_dir().join(".shortcuts"), PathBuf::from)
}

/// Os atalhos do `fshort`, na ordem do arquivo; vazio se ele não existe.
pub fn fshort() -> Vec<(String, PathBuf)> {
    std::fs::read_to_string(fshort_file()).map(|text| parse_fshort(&text)).unwrap_or_default()
}

/// Edita o atalho `old` do fshort (nome e pasta), no mesmo lugar do arquivo.
pub fn update_fshort(old: &str, name: &str, path: &Path) -> Result<(), String> {
    let file = fshort_file();
    let text = std::fs::read_to_string(&file).map_err(|error| format!("não consegui ler {}: {error}", file.display()))?;
    write_fshort(&file, &edit_fshort(&text, old, name, path)?)
}

/// Exclui o atalho `name` do fshort.
pub fn delete_fshort(name: &str) -> Result<(), String> {
    let file = fshort_file();
    let text = std::fs::read_to_string(&file).map_err(|error| format!("não consegui ler {}: {error}", file.display()))?;
    write_fshort(&file, &remove_fshort(&text, name))
}

/// Grava como o `fshort` faz (`.tmp` + `mv`).
fn write_fshort(file: &Path, text: &str) -> Result<(), String> {
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, text)
        .and_then(|()| std::fs::rename(&tmp, file))
        .map_err(|error| format!("não consegui gravar {}: {error}", file.display()))
}

/// A regra de nome do `fshort` (`^[A-Za-z0-9._-]+$`), mais estrita que a do C-hub: o nome editado
/// precisa continuar valendo no `go`.
fn check_fshort_name(name: &str) -> Result<String, String> {
    let name = check_name(name)?;
    if name.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        Ok(name)
    } else {
        Err("No fshort o nome só pode ter letras, números, . _ -".into())
    }
}

/// O `~/.shortcuts` com a linha de `old` trocada por `name=path`, no mesmo lugar. Erro se `name`
/// já for de outro atalho do arquivo.
fn edit_fshort(text: &str, old: &str, name: &str, path: &Path) -> Result<String, String> {
    let name = check_fshort_name(name)?;
    let key = |line: &str| line.split_once('=').map(|(key, _)| key.trim().to_owned());
    if name != old && text.lines().any(|line| key(line).as_deref() == Some(name.as_str())) {
        return Err(format!("Já existe um atalho \"{name}\""));
    }
    let mut found = false;
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if key(line).as_deref() == Some(old) {
            // Repetido (o fshort não deixa, mas o arquivo é texto): só o primeiro fica.
            if !found {
                out.push(format!("{name}={}", path.display()));
                found = true;
            }
        } else {
            out.push(line.to_owned());
        }
    }
    if !found {
        return Err(format!("O atalho \"{old}\" não está mais no ~/.shortcuts"));
    }
    Ok(out.join("\n") + "\n")
}

/// O `~/.shortcuts` sem as linhas de `name` (como o `fshort -d`).
fn remove_fshort(text: &str, name: &str) -> String {
    let prefix = format!("{name}=");
    let lines: Vec<&str> = text.lines().filter(|line| !line.starts_with(&prefix)).collect();
    if lines.is_empty() { String::new() } else { lines.join("\n") + "\n" }
}

/// Todos os atalhos: os do C-hub e, depois, os do fshort que eles não substituem.
pub fn all() -> Vec<DirShortcut> {
    merge(own(), fshort())
}

/// Pasta de um atalho, se ele existe.
pub fn find(name: &str) -> Option<PathBuf> {
    all().into_iter().find(|shortcut| shortcut.name == name).map(|shortcut| shortcut.path)
}

fn parse_fshort(text: &str) -> Vec<(String, PathBuf)> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, path)| (name.trim().to_owned(), PathBuf::from(path.trim())))
        .filter(|(name, path)| !name.is_empty() && !path.as_os_str().is_empty())
        .collect()
}

fn merge(own: Vec<(String, PathBuf)>, fshort: Vec<(String, PathBuf)>) -> Vec<DirShortcut> {
    let mut list: Vec<DirShortcut> =
        own.into_iter().map(|(name, path)| DirShortcut { name, path, fshort: false }).collect();
    for (name, path) in fshort {
        if !list.iter().any(|shortcut| shortcut.name == name) {
            list.push(DirShortcut { name, path, fshort: true });
        }
    }
    list
}

fn from_json(value: &Value) -> Vec<(String, PathBuf)> {
    let mut list: Vec<(String, PathBuf)> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?;
            let path = item.get("path")?.as_str()?;
            check_name(name).ok().map(|name| (name, PathBuf::from(path)))
        })
        .collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    list.dedup_by(|a, b| a.0 == b.0);
    list
}

fn to_json(list: &[(String, PathBuf)]) -> Value {
    Value::Array(list.iter().map(|(name, path)| json!({ "name": name, "path": path.to_string_lossy() })).collect())
}

/// Confere um nome de atalho e o devolve sem os espaços das pontas. Ele vira a primeira parte de
/// um caminho (`nome/sub`), então não pode ter `/` nem espaços, nem parecer `.`, `..` ou `~`.
pub fn check_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Digite um nome para o atalho".into());
    }
    if name.contains('/') || name.chars().any(char::is_whitespace) {
        return Err("O nome não pode ter / nem espaços".into());
    }
    if name == "." || name == ".." || name.starts_with('~') {
        return Err(format!("\"{name}\" não pode ser nome de atalho"));
    }
    if name.chars().count() > 32 {
        return Err("Nome comprido demais (máximo 32 letras)".into());
    }
    Ok(name.to_owned())
}

/// Grava `name` → `path` na lista. Com `old`, edita o atalho `old` (pode trocar o nome, desde que
/// o novo não seja de outro atalho); sem `old`, cria ou troca a pasta de um atalho que já existe.
pub fn upsert(list: &mut Vec<(String, PathBuf)>, old: Option<&str>, name: &str, path: PathBuf) -> Result<(), String> {
    let name = check_name(name)?;
    if let Some(old) = old {
        if name != old && list.iter().any(|(existing, _)| *existing == name) {
            return Err(format!("Já existe um atalho \"{name}\""));
        }
        list.retain(|(existing, _)| existing != old);
    }
    list.retain(|(existing, _)| *existing != name);
    list.push((name, path));
    list.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(())
}

pub fn remove(list: &mut Vec<(String, PathBuf)>, name: &str) {
    list.retain(|(existing, _)| existing != name);
}

/// Pasta digitada na edição de um atalho: `~`, `~/…` ou um caminho absoluto, que exista.
pub fn expand(text: &str) -> Result<PathBuf, String> {
    expand_in(text, &home_dir())
}

fn expand_in(text: &str, home: &Path) -> Result<PathBuf, String> {
    let text = text.trim();
    let path = if text == "~" {
        home.to_path_buf()
    } else if let Some(rest) = text.strip_prefix("~/") {
        home.join(rest)
    } else if Path::new(text).is_absolute() {
        PathBuf::from(text)
    } else {
        return Err("Use um caminho completo (/… ou ~/…)".into());
    };
    let path = path.canonicalize().map_err(|_| format!("{} não existe", path.display()))?;
    if path.is_dir() { Ok(path) } else { Err(format!("{} não é uma pasta", path.display())) }
}

/// Sugestão de nome para um atalho novo: o nome da pasta, só com o que o nome aceita.
pub fn suggest_name(path: &Path) -> String {
    let base = path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
    let name: String = base.chars().map(|c| if c.is_whitespace() || c == '/' { '-' } else { c }).take(32).collect();
    let name = name.trim_start_matches('~').to_owned();
    if check_name(&name).is_ok() { name } else { String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, path: &str) -> (String, PathBuf) {
        (name.to_owned(), PathBuf::from(path))
    }

    #[test]
    fn nomes_validos() {
        assert_eq!(check_name("  proj ").unwrap(), "proj");
        assert_eq!(check_name("meu-app_2").unwrap(), "meu-app_2");
        assert!(check_name("").is_err());
        assert!(check_name("a b").is_err());
        assert!(check_name("a/b").is_err());
        assert!(check_name(".").is_err());
        assert!(check_name("..").is_err());
        assert!(check_name("~x").is_err());
        assert!(check_name(&"x".repeat(33)).is_err());
    }

    #[test]
    fn criar_editar_e_excluir() {
        let mut list = Vec::new();
        upsert(&mut list, None, "web", "/srv/web".into()).unwrap();
        upsert(&mut list, None, "api", "/srv/api".into()).unwrap();
        assert_eq!(list, vec![pair("api", "/srv/api"), pair("web", "/srv/web")]);
        // Criar com um nome que já existe troca a pasta.
        upsert(&mut list, None, "web", "/srv/site".into()).unwrap();
        assert_eq!(list, vec![pair("api", "/srv/api"), pair("web", "/srv/site")]);
        // Renomear.
        upsert(&mut list, Some("web"), "site", "/srv/site".into()).unwrap();
        assert_eq!(list, vec![pair("api", "/srv/api"), pair("site", "/srv/site")]);
        // Renomear para o nome de outro atalho não apaga o outro.
        assert!(upsert(&mut list, Some("site"), "api", "/x".into()).is_err());
        assert_eq!(list.len(), 2);
        // Editar só a pasta.
        upsert(&mut list, Some("api"), "api", "/srv/api2".into()).unwrap();
        assert_eq!(list[0], pair("api", "/srv/api2"));
        remove(&mut list, "api");
        assert_eq!(list, vec![pair("site", "/srv/site")]);
    }

    #[test]
    fn json_ida_e_volta() {
        let list = vec![pair("a", "/x"), pair("b", "/y z")];
        assert_eq!(from_json(&to_json(&list)), list);
        // Itens estragados ou com nome inválido ficam de fora; repetidos, só o primeiro.
        let value = json!([{"name": "z", "path": "/z"}, {"name": "a b", "path": "/q"}, {"path": "/w"}, 3, {"name": "z", "path": "/z2"}]);
        assert_eq!(from_json(&value), vec![pair("z", "/z")]);
        assert!(from_json(&json!({"nao": "lista"})).is_empty());
    }

    #[test]
    fn os_proprios_tem_prioridade_sobre_os_do_fshort() {
        let fshort = parse_fshort("mvp=/home/u/dev\nmain=/home/u\n\nlixo\n=/sem/nome\n");
        assert_eq!(fshort, vec![pair("mvp", "/home/u/dev"), pair("main", "/home/u")]);
        let all = merge(vec![pair("mvp", "/outro")], fshort);
        assert_eq!(
            all,
            vec![
                DirShortcut { name: "mvp".into(), path: "/outro".into(), fshort: false },
                DirShortcut { name: "main".into(), path: "/home/u".into(), fshort: true },
            ]
        );
    }

    #[test]
    fn editar_e_excluir_no_fshort() {
        let text = "mvp=/home/u/dev\nmain=/home/u\nchb=/home/u/chube\n";
        // Editar fica no mesmo lugar.
        assert_eq!(edit_fshort(text, "main", "casa", Path::new("/home/u")).unwrap(), "mvp=/home/u/dev\ncasa=/home/u\nchb=/home/u/chube\n");
        assert_eq!(edit_fshort(text, "mvp", "mvp", Path::new("/srv")).unwrap(), "mvp=/srv\nmain=/home/u\nchb=/home/u/chube\n");
        assert!(edit_fshort(text, "main", "chb", Path::new("/x")).is_err(), "nome de outro atalho");
        assert!(edit_fshort(text, "sumiu", "x", Path::new("/x")).is_err());
        assert!(edit_fshort(text, "main", "ção", Path::new("/x")).is_err(), "regra do fshort");
        assert_eq!(remove_fshort(text, "main"), "mvp=/home/u/dev\nchb=/home/u/chube\n");
        // `main` não leva junto um `mainx`.
        assert_eq!(remove_fshort("main=/a\nmainx=/b\n", "main"), "mainx=/b\n");
        assert_eq!(remove_fshort("a=/a\n", "a"), "");
    }

    #[test]
    fn pasta_da_edicao() {
        let home = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(expand_in("~", &home).unwrap(), home);
        assert_eq!(expand_in(&home.display().to_string(), &home).unwrap(), home);
        assert!(expand_in("relativa", &home).is_err());
        assert!(expand_in("~/nao-existe-mesmo-123", &home).is_err());
    }

    #[test]
    fn nome_sugerido() {
        assert_eq!(suggest_name(Path::new("/home/u/Meu Projeto")), "meu-projeto");
        assert_eq!(suggest_name(Path::new("/")), "");
    }
}
