//! Comandos `/…` sugeridos no campo de mensagem do chat.
//!
//! O texto vai para a TUI do Claude Code como se fosse digitado, então qualquer comando
//! funciona; a lista só ajuda a lembrar e completar os nomes.

use std::fs;
use std::path::Path;

use crate::claude::home_dir;

/// Máximo de sugestões mostradas de uma vez.
const MAX_SUGGESTIONS: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// Sem a `/`.
    pub name: String,
    pub description: String,
}

/// Comandos do próprio Claude Code (2.1.x).
const BUILTIN: [(&str, &str); 36] = [
    ("add-dir", "Adiciona outra pasta de trabalho"),
    ("agents", "Gerencia os subagentes"),
    ("btw", "Pergunta paralela, sem entrar na conversa"),
    ("clear", "Começa uma conversa nova (sessão nova)"),
    ("compact", "Resume a conversa para liberar contexto"),
    ("config", "Abre as configurações"),
    ("context", "Mostra o uso do contexto"),
    ("cost", "Custo e duração da sessão"),
    ("doctor", "Verifica a instalação"),
    ("effort", "Define o esforço de raciocínio"),
    ("exit", "Encerra a sessão"),
    ("export", "Exporta a conversa"),
    ("help", "Ajuda e atalhos"),
    ("hooks", "Gerencia os hooks"),
    ("init", "Cria o CLAUDE.md do projeto"),
    ("keybindings", "Personaliza os atalhos"),
    ("login", "Entra com outra conta"),
    ("logout", "Sai da conta"),
    ("mcp", "Gerencia os servidores MCP"),
    ("memory", "Edita os arquivos de memória"),
    ("model", "Troca o modelo"),
    ("permissions", "Regras de permissão"),
    ("plan", "Entra no modo de planejamento"),
    ("plugin", "Gerencia os plugins"),
    ("release-notes", "Novidades da versão"),
    ("resume", "Retoma outra sessão"),
    ("review", "Revisa um pull request"),
    ("rewind", "Volta a conversa e o código a um ponto anterior"),
    ("security-review", "Revisão de segurança das mudanças"),
    ("skills", "Lista as skills"),
    ("status", "Versão, modelo e conta"),
    ("statusline", "Configura a linha de status"),
    ("tasks", "Tarefas em segundo plano"),
    ("todos", "Lista de tarefas atual"),
    ("usage", "Limites do plano"),
    ("vim", "Liga/desliga o modo vim"),
];

/// Comandos do Claude Code mais os do usuário e do projeto (`.claude/commands`, `.claude/skills`).
/// Lê o disco: chamar fora da thread da UI.
pub fn load(cwd: &Path) -> Vec<Command> {
    let mut list: Vec<Command> = BUILTIN
        .iter()
        .map(|(name, description)| Command {
            name: (*name).to_owned(),
            description: (*description).to_owned(),
        })
        .collect();
    for base in [home_dir().join(".claude"), cwd.join(".claude")] {
        custom_commands(&base.join("commands"), "", &mut list);
        skills(&base.join("skills"), &mut list);
    }
    list
}

/// `commands/nome.md` vira `/nome`; subpastas viram `/pasta:nome`.
fn custom_commands(dir: &Path, prefix: &str, list: &mut Vec<Command>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if path.is_dir() {
            custom_commands(&path, &format!("{prefix}{stem}:"), list);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            push_unique(list, format!("{prefix}{stem}"), description(&path, "Comando personalizado"));
        }
    }
}

/// `skills/<nome>/SKILL.md` vira `/nome`.
fn skills(dir: &Path, list: &mut Vec<Command>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let file = entry.path().join("SKILL.md");
        if let (true, Some(name)) = (file.is_file(), entry.file_name().to_str()) {
            push_unique(list, name.to_owned(), description(&file, "Skill"));
        }
    }
}

fn push_unique(list: &mut Vec<Command>, name: String, description: String) {
    if !list.iter().any(|command| command.name == name) {
        list.push(Command { name, description });
    }
}

/// `description:` do cabeçalho do arquivo, se houver.
fn description(path: &Path, fallback: &str) -> String {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| {
            text.lines()
                .take(30)
                .find_map(|line| line.strip_prefix("description:"))
                .map(|value| value.trim().trim_matches('"').to_owned())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

/// Sugestões para o texto do campo: só enquanto ele é uma `/` seguida do começo de um nome.
/// Primeiro os que começam com o texto, depois os que o contêm.
pub fn suggestions<'a>(list: &'a [Command], text: &str) -> Vec<&'a Command> {
    let Some(typed) = text.strip_prefix('/') else {
        return Vec::new();
    };
    if typed.contains(char::is_whitespace) {
        return Vec::new();
    }
    let typed = typed.to_lowercase();
    let mut found: Vec<&Command> = list
        .iter()
        .filter(|command| command.name.to_lowercase().starts_with(&typed))
        .collect();
    found.extend(list.iter().filter(|command| {
        let name = command.name.to_lowercase();
        !name.starts_with(&typed) && name.contains(&typed)
    }));
    // Nome já completo: nada a sugerir.
    if found.len() == 1 && found[0].name.eq_ignore_ascii_case(&typed) {
        return Vec::new();
    }
    found.truncate(MAX_SUGGESTIONS);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(found: Vec<&Command>) -> Vec<&str> {
        found.iter().map(|command| command.name.as_str()).collect()
    }

    #[test]
    fn suggest() {
        let list = load(Path::new("/nonexistent"));
        assert_eq!(names(suggestions(&list, "/mod")), ["model"]);
        assert_eq!(names(suggestions(&list, "/model")), Vec::<&str>::new());
        assert_eq!(&names(suggestions(&list, "/re"))[..3], ["release-notes", "resume", "review"]);
        assert!(names(suggestions(&list, "/view")).contains(&"review"));
        assert!(suggestions(&list, "/model sonnet").is_empty());
        assert!(suggestions(&list, "oi").is_empty());
        assert_eq!(suggestions(&list, "/").len(), MAX_SUGGESTIONS);
    }
}
