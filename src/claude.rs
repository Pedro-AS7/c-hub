//! Sessões do Claude Code rodando no modo interativo, cada uma no terminal do seu chat.
//!
//! O app só monta o comando; a conversa, as permissões e o resto da interface
//! são do próprio Claude Code, dentro do PTY.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// Sessão nova, com o id escolhido pelo app.
    New { session_id: String },
    /// Continua uma sessão salva (`--resume <id>`).
    Resume { session_id: String },
    /// Cópia de uma sessão existente (`--resume <id> --fork-session`), com um id novo.
    Fork { parent_session_id: String },
}

/// Programa e argumentos para rodar o `claude` no terminal do chat.
///
/// Passa pelo `env -u ANTHROPIC_API_KEY`: com a API key definida, o Claude Code cobraria
/// pela API em vez de usar o plano. O `env` faz `exec`, então o pid do PTY é o do `claude`.
pub fn command(start: &Start, model: Option<&str>, prompt: Option<&str>) -> (String, Vec<String>) {
    let mut args: Vec<String> = vec![
        "-u".into(),
        "ANTHROPIC_API_KEY".into(),
        claude_bin().display().to_string(),
    ];
    match start {
        Start::New { session_id } => args.extend(["--session-id".into(), session_id.clone()]),
        Start::Resume { session_id } => args.extend(["--resume".into(), session_id.clone()]),
        Start::Fork { parent_session_id } => args.extend([
            "--resume".into(),
            parent_session_id.clone(),
            "--fork-session".into(),
        ]),
    }
    if let Some(model) = model {
        args.extend(["--model".into(), model.to_owned()]);
    }
    if let Some(prompt) = prompt {
        // `--` para um prompt que comece com `-` não virar opção.
        args.extend(["--".into(), prompt.to_owned()]);
    }
    ("env".into(), args)
}

/// Procura o `claude` no PATH e, se não achar (apps abertos pelo launcher podem não ter
/// `~/.local/bin` no PATH), usa `~/.local/bin/claude`.
pub fn claude_bin() -> PathBuf {
    let in_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("claude"))
            .find(|candidate| candidate.is_file())
    });
    in_path.unwrap_or_else(|| home_dir().join(".local/bin/claude"))
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// Mostra o caminho com `~` no lugar da home.
pub fn display_path(path: &Path) -> String {
    match path.strip_prefix(home_dir()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Título do terminal definido pelo Claude Code (`✳ Assunto`, com um spinner no lugar
/// do `✳` enquanto trabalha), sem o símbolo. `None` enquanto é só o nome do programa.
pub fn title_from_terminal(title: &str) -> Option<String> {
    let title = title
        .trim_start_matches(|c: char| !c.is_alphanumeric() && !c.is_ascii_punctuation())
        .trim();
    (!title.is_empty() && title != "Claude Code").then(|| title.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_title() {
        assert_eq!(title_from_terminal("✳ Claude Code"), None);
        assert_eq!(title_from_terminal(""), None);
        assert_eq!(
            title_from_terminal("✳ Corrigir o parser"),
            Some("Corrigir o parser".into())
        );
        assert_eq!(title_from_terminal("⠂ Corrigir o parser"), Some("Corrigir o parser".into()));
        assert_eq!(title_from_terminal("\"cargo\" falhou"), Some("\"cargo\" falhou".into()));
    }

    #[test]
    fn command_args() {
        let start = Start::Fork {
            parent_session_id: "abc".into(),
        };
        let (program, args) = command(&start, Some("haiku"), Some("-oi"));
        assert_eq!(program, "env");
        assert_eq!(&args[..2], ["-u", "ANTHROPIC_API_KEY"]);
        assert_eq!(
            &args[3..],
            ["--resume", "abc", "--fork-session", "--model", "haiku", "--", "-oi"]
        );
    }
}
