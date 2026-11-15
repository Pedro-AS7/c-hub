//! Preferências do usuário em `$XDG_CONFIG_HOME/chube/config.json` (padrão `~/.config/chube`):
//! o tema escolhido (`theme`), o tema personalizado (`custom_theme`) e os atalhos de pasta
//! (`shortcuts`). A lista de chats abertos é estado, não preferência (`restore.rs`).

use std::path::{Path, PathBuf};

use serde_json::Value;

fn path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| crate::claude::home_dir().join(".config"))
        .join("chube/config.json")
}

/// Valor de uma chave; `None` se ela não existe (ou o arquivo não existe ou está estragado).
pub fn get(key: &str) -> Option<Value> {
    get_in(&path(), key)
}

/// Grava uma chave, mantendo as outras do arquivo.
pub fn set(key: &str, value: Value) -> std::io::Result<()> {
    set_in(&path(), key, value)
}

/// Tema gravado (`omarchy`, o id de um tema próprio ou `personalizado`); `None` se nada foi escolhido.
pub fn theme() -> Option<String> {
    get("theme")?.as_str().map(str::to_owned)
}

pub fn set_theme(id: &str) -> std::io::Result<()> {
    set("theme", Value::String(id.to_owned()))
}

fn get_in(path: &Path, key: &str) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut value: Value = serde_json::from_str(&text).ok()?;
    value.get_mut(key).map(Value::take)
}

fn set_in(path: &Path, key: &str, new: Value) -> std::io::Result<()> {
    let mut value = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    value[key] = new;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Arquivo temporário + `rename`: um desligamento no meio não deixa o arquivo pela metade.
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_string_pretty(&value).unwrap_or_default())?;
    std::fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme_in(path: &Path) -> Option<String> {
        get_in(path, "theme")?.as_str().map(str::to_owned)
    }

    fn set_theme_in(path: &Path, id: &str) {
        set_in(path, "theme", Value::String(id.to_owned())).unwrap();
    }

    #[test]
    fn grava_e_le_o_tema_sem_perder_outras_chaves() {
        let dir = std::env::temp_dir().join(format!("chube-config-{}", uuid::Uuid::new_v4()));
        let file = dir.join("chube/config.json");
        assert_eq!(theme_in(&file), None);
        set_theme_in(&file, "nord");
        assert_eq!(theme_in(&file).as_deref(), Some("nord"));
        std::fs::write(&file, r#"{"outra": 1, "theme": "nord"}"#).unwrap();
        set_theme_in(&file, "gruvbox-dark");
        assert_eq!(theme_in(&file).as_deref(), Some("gruvbox-dark"));
        assert!(std::fs::read_to_string(&file).unwrap().contains("outra"));
        // Outras chaves gravadas não apagam o tema.
        set_in(&file, "shortcuts", serde_json::json!([{"name": "w", "path": "/tmp"}])).unwrap();
        assert_eq!(theme_in(&file).as_deref(), Some("gruvbox-dark"));
        assert_eq!(get_in(&file, "shortcuts").unwrap()[0]["name"], "w");
        // Arquivo estragado: volta a não ter escolha, e a próxima gravação o refaz.
        std::fs::write(&file, "{ não é json").unwrap();
        assert_eq!(theme_in(&file), None);
        set_theme_in(&file, "nord");
        assert_eq!(theme_in(&file).as_deref(), Some("nord"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
