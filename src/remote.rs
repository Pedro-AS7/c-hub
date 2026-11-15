//! Sessões em outra máquina por SSH (a VPS), cada uma dentro de um tmux de lá.
//!
//! O chat roda `ssh -t <host> … tmux new-session -A -s <nome> …`: o `-A` entra na sessão do
//! tmux se ela ainda existe e, se não, cria uma rodando o agente. Assim, fechar o chat, fechar o
//! Chube ou cair a rede só desconecta: o agente continua na VPS, e reabrir o chat volta a ele.
//! Também dá para entrar pelo celular (`tmux attach -t <nome>`).
//!
//! Cada conexão autentica de novo, sem mestre compartilhado (`ControlMaster`): com a chave no
//! agente com confirmação (`AddKeysToAgent confirm`), cada uma pede permissão numa janela, e um
//! processo qualquer não consegue pegar carona numa conexão aberta pelo Chube. Para o estado dos
//! chats não abrir uma janela a cada consulta, ele vem por uma conexão fixa por host
//! (`poll_stream`), aberta uma vez e reaproveitada enquanto houver chat lá.
//!
//! Nada é gravado na VPS além do que o agente e o tmux já gravam. As opções do tmux valem só
//! para as sessões criadas pelo Chube (`set-option -t`), menos a de cor (`terminal-features`),
//! que é do servidor do tmux e só acrescenta o suporte a truecolor.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use crate::agent::Agent;
use crate::claude::home_dir;
use crate::sessions::SavedSession;

/// Tempo máximo de uma consulta em segundo plano.
const TIMEOUT: Duration = Duration::from_secs(20);
/// Prefixo das sessões do tmux criadas pelo Chube.
pub const TMUX_PREFIX: &str = "chube-";

/// Hosts do `~/.ssh/config` (só os nomes exatos, sem `*` nem `?`).
pub fn hosts() -> Vec<String> {
    std::fs::read_to_string(home_dir().join(".ssh/config"))
        .map(|text| parse_hosts(&text))
        .unwrap_or_default()
}

fn parse_hosts(config: &str) -> Vec<String> {
    let mut hosts = Vec::new();
    for line in config.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once(char::is_whitespace) else { continue };
        if !key.eq_ignore_ascii_case("host") {
            continue;
        }
        for name in rest.split_whitespace() {
            let name = name.trim_matches('"');
            if !name.contains(['*', '?', '!']) && !hosts.iter().any(|host| host == name) {
                hosts.push(name.to_owned());
            }
        }
    }
    hosts
}

/// Nome novo para a sessão do tmux de um chat.
pub fn new_tmux_name() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    format!("{TMUX_PREFIX}{}", &id[..8])
}

/// Aspas simples do shell: o texto passa literal, mesmo com espaços, `$` ou aspas.
pub fn quote(text: &str) -> String {
    if !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@%+,".contains(c)) {
        return text.to_owned();
    }
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Pasta da VPS para o `cd`: `~` e `~/…` viram `"$HOME"…` (o til entre aspas não expande).
fn quote_dir(dir: &str) -> String {
    match dir.strip_prefix('~') {
        Some("") => "\"$HOME\"".into(),
        Some(rest) if rest.starts_with('/') => format!("\"$HOME\"{}", quote(rest)),
        _ => quote(dir),
    }
}

/// Opções de SSH de todas as conexões do Chube: sem mestre compartilhado (mesmo que o
/// `~/.ssh/config` tenha um), para cada conexão autenticar e pedir a confirmação da chave, e um
/// aviso de vida, para uma rede caída fechar o chat em vez de deixá-lo congelado.
fn ssh_options() -> Vec<String> {
    ["ControlMaster=no", "ControlPath=none", "ServerAliveInterval=15", "ServerAliveCountMax=4"]
        .into_iter()
        .flat_map(|option| ["-o".to_owned(), option.to_owned()])
        .collect()
}

/// Linha de comando (para o shell da VPS) que roda o agente. Num login sem terminal, o PATH do
/// `bash -l` pode não ter `~/.local/bin`, onde o instalador do Claude Code põe o `claude`.
pub fn agent_shell_command(agent: Agent, args: &[String], fallback: Option<&[String]>) -> String {
    let (binary, unset) = match agent {
        Agent::Claude => ("claude", "-u ANTHROPIC_API_KEY"),
        Agent::Codex => ("codex", "-u OPENAI_API_KEY -u CODEX_API_KEY"),
    };
    let run = |args: &[String]| {
        let args: Vec<String> = args.iter().map(|arg| quote(arg)).collect();
        format!("env {unset} {binary} {}", args.join(" ")).trim_end().to_owned()
    };
    let mut command = format!("export PATH=\"$HOME/.local/bin:$PATH\"; {}", run(args));
    // Uma sessão nova cujo histórico não chegou a ser gravado não tem o que retomar: começa de
    // novo com o mesmo id.
    if let Some(fallback) = fallback {
        command = format!("{command} || {}", run(fallback));
    }
    command
}

/// Programa e argumentos do chat: `ssh -t` que entra na sessão do tmux (ou a cria rodando
/// `agent_command` na pasta `dir`).
pub fn command(host: &str, dir: &str, tmux: &str, agent_command: Option<&str>) -> (String, Vec<String>) {
    (
        "ssh".into(),
        ssh_args(host, true, &remote_script(dir, tmux, agent_command)),
    )
}

fn ssh_args(host: &str, tty: bool, script: &str) -> Vec<String> {
    let mut args = Vec::new();
    if tty {
        args.push("-t".to_owned());
    } else {
        args.extend(["-o".to_owned(), "BatchMode=yes".to_owned(), "-T".to_owned()]);
    }
    args.extend(ssh_options());
    args.extend(["--".to_owned(), host.to_owned(), script.to_owned()]);
    args
}

/// Script do shell da VPS. Sem `agent_command`, só entra numa sessão do tmux que já existe
/// (uma aberta pelo celular, por exemplo).
fn remote_script(dir: &str, tmux: &str, agent_command: Option<&str>) -> String {
    // Num login sem terminal, o `bash -l` carrega o PATH de lá (`~/.local/bin`, nvm…).
    let agent = agent_command.map(|agent| format!("bash -lc {}", quote(agent)));
    tmux_script("tmux", &quote_dir(dir), tmux, agent.as_deref(), &[])
}

/// Servidor do tmux dos chats deste notebook: separado do tmux do usuário e sem a configuração
/// dele (a do Omarchy liga Alt+Esc a "fechar painel", Alt+Enter a "dividir"… em qualquer modo).
/// O celular entra nele com o mesmo `-L`.
pub const LOCAL_TMUX: &str = "tmux -L chube -f /dev/null";

/// Variáveis da sessão gráfica levadas aos tmux deste notebook: o servidor do tmux pode ter sido
/// aberto pelo celular (pelo `sshd`, sem Wayland), e o agente precisa delas para colar imagens,
/// abrir o navegador ou notificar.
const DESKTOP_ENV: &[&str] = &[
    "PATH",
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "HYPRLAND_INSTANCE_SIGNATURE",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
];

/// Programa e argumentos de um chat deste notebook: o mesmo tmux dos chats da VPS, mas rodado
/// aqui. Assim o celular entra nele por SSH, e fechar o Chube não encerra o agente.
/// `agent_command` são as palavras do comando já entre aspas do shell: o tmux as recebe como
/// argumentos e roda o programa direto, sem shell (o pid do painel é o do agente).
pub fn local_command(dir: &Path, tmux: &str, agent_command: Option<&str>) -> (String, Vec<String>) {
    let env: Vec<String> = DESKTOP_ENV
        .iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| format!("{key}={value}")))
        .collect();
    let script = tmux_script(LOCAL_TMUX, &quote(&dir.to_string_lossy()), tmux, agent_command, &env);
    ("sh".into(), vec!["-c".into(), script])
}

/// Entra na sessão `tmux` (ou a cria rodando `agent`, palavras já entre aspas, na pasta `dir`,
/// também já entre aspas) e põe as opções das sessões do Chube. Sem `agent`, só entra.
/// `binary` é o tmux com as opções do servidor (`LOCAL_TMUX` aqui).
fn tmux_script(binary: &str, dir: &str, tmux: &str, agent: Option<&str>, env: &[String]) -> String {
    let name = quote(tmux);
    // Só nas sessões do Chube: sem a barra (o Chube já mostra o título), mouse para rolar o
    // histórico com a roda, e o título do agente repassado ao terminal (dá o status do chat).
    let options = [
        format!("set-option -t {name} status off"),
        format!("set-option -t {name} mouse on"),
        format!("set-option -t {name} set-titles on"),
        format!("set-option -t {name} set-titles-string '#{{pane_title}}'"),
        // Com o celular e o desktop na mesma sessão, a janela tem o tamanho de quem mexeu por
        // último; o que sobra no maior fica em branco, não pontilhado. (`-q`: tmux antigo ignora.)
        format!("set-option -wq -t {name} fill-character ' '"),
        "set-option -as terminal-features ',xterm-256color:RGB'".to_owned(),
    ]
    .join(" \\; ");
    match agent {
        Some(agent) => {
            let env: String = env.iter().map(|pair| format!(" -e {}", quote(pair))).collect();
            // Sem prefixo nas sessões criadas pelo Chube: o C-b é o "mandar para o fundo" do
            // Claude Code. (Numa sessão de fora, o prefixo de quem a abriu fica.)
            format!(
                "cd {dir} 2>/dev/null || cd; exec {binary} new-session -A -s {name} -e COLORTERM=truecolor{env} {agent} \\; {options} \\; set-option -t {name} prefix None \\; set-option -t {name} prefix2 None"
            )
        }
        None => format!("exec {binary} attach-session -t {name} \\; {options}"),
    }
}

/// Roda um script neste notebook (o mesmo que iria à VPS), com o tempo máximo das consultas.
async fn run_local(script: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("sh")
        .args(["-c", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(TIMEOUT, output)
        .await
        .map_err(|_| "o tmux não respondeu".to_owned())?
        .map_err(|error| format!("não consegui rodar o sh: {error}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(stderr.lines().last().unwrap_or("erro no tmux").trim().to_owned())
}

/// Encerra a sessão do tmux de um chat deste notebook (e o agente dentro dela).
pub async fn kill_local(tmux: String) -> Result<(), String> {
    run_local(&format!("{LOCAL_TMUX} kill-session -t {}", quote(&tmux))).await.map(|_| ())
}

/// Sessões do tmux abertas neste notebook (as do celular, as de chats fechados). Sem servidor do
/// tmux, uma lista vazia.
pub fn list_local_tmux() -> Vec<TmuxSession> {
    let script = format!("printf 'H\\t%s\\n' \"$HOME\"\n{}", LIST_TMUX.replacen("tmux", LOCAL_TMUX, 1));
    let Ok(output) = std::process::Command::new("sh")
        .args(["-c", &script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    parse_listing("local", &String::from_utf8_lossy(&output.stdout)).tmux
}

/// Pid do painel de cada sessão do tmux deste notebook (o agente é ele ou um filho dele).
pub fn local_panes() -> std::collections::HashMap<String, u32> {
    let Ok(output) = std::process::Command::new("tmux")
        .args(["-L", "chube", "list-panes", "-a", "-F", "#{session_name}\t#{pane_pid}"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return Default::default();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(name, pid)| Some((name.to_owned(), pid.trim().parse().ok()?)))
        .collect()
}

/// Roda um script na VPS sem terminal e sem pedir senha (usa o mestre do SSH ou o agente SSH).
async fn run(host: &str, script: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("ssh")
        .args(ssh_args(host, false, script))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(TIMEOUT, output)
        .await
        .map_err(|_| format!("{host} não respondeu"))?
        .map_err(|error| format!("não consegui rodar o ssh: {error}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(if stderr.contains("Permission denied") {
        format!("sem acesso a {host}: a chave não está no agente ou o uso foi negado. Abra um chat lá (o terminal pede a senha da chave) e tente de novo")
    } else {
        stderr.lines().last().unwrap_or("erro no ssh").trim().to_owned()
    })
}

/// Encerra a sessão do tmux de um chat (e o agente dentro dela).
pub async fn kill(host: String, tmux: String) -> Result<(), String> {
    run(&host, &format!("tmux kill-session -t {}", quote(&tmux))).await.map(|_| ())
}

/// Estado de um chat da VPS, lido lá: o que o `~/.claude/sessions/<pid>.json` do `claude` do tmux
/// diz (id e status) e a última resposta do histórico (modelo e contexto).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteStatus {
    pub tmux: String,
    pub session_id: Option<String>,
    /// `busy`, `idle` ou `waiting`, como nesta máquina.
    pub status: Option<String>,
    pub model: Option<String>,
    pub context_tokens: u32,
}

/// Resultado de uma leitura de estado de um host: a lista ou, quando a conexão caiu ou foi negada,
/// o erro (depois do erro, a leitura termina).
pub type PollItem = (String, Result<Vec<RemoteStatus>, String>);

/// Estado dos chats do Claude Code de um host, por uma conexão SSH só (uma confirmação da chave):
/// lá, um laço lê todas as sessões do tmux a cada 3 s e manda o resultado a `output`. Termina com
/// o erro quando a conexão cai ou é negada, ou em silêncio quando `output` é fechado (o app não
/// precisa mais do host); nos dois casos o `ssh` morre junto (`kill_on_drop`).
pub async fn poll(host: String, output: async_channel::Sender<PollItem>) {
    let error = poll_connection(&host, &output).await;
    if !error.is_empty() {
        let _ = output.send((host, Err(error))).await;
    }
}

async fn poll_connection(host: &str, output: &async_channel::Sender<PollItem>) -> String {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

    let child = tokio::process::Command::new("ssh")
        .args(ssh_args(host, false, &poll_loop_script()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => return format!("não consegui rodar o ssh: {error}"),
    };
    let (Some(stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return "sem saída do ssh".into();
    };
    let mut lines = BufReader::new(stdout).lines();
    let mut batch = String::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if line == "E" {
            if output.send((host.to_owned(), Ok(parse_poll(&batch)))).await.is_err() {
                return String::new();
            }
            batch.clear();
        } else {
            batch.push_str(&line);
            batch.push('\n');
        }
    }
    let mut text = String::new();
    let _ = stderr.read_to_string(&mut text).await;
    let _ = child.wait().await;
    text.lines().last().unwrap_or("a conexão do estado fechou").trim().to_owned()
}

/// Laço da conexão de estado: `chube_poll` em cada sessão do tmux e um `E` no fim de cada volta.
/// Quando o Chube fecha a conexão, o próximo `echo` recebe SIGPIPE e o laço termina.
fn poll_loop_script() -> String {
    format!(
        "{POLL_FUNCTION}while :; do\n  tmux list-sessions -F '#{{session_name}}' 2>/dev/null | while IFS= read -r t; do chube_poll \"$t\"; done\n  echo E\n  sleep 3\ndone\n"
    )
}

/// Os mesmos `chube_poll` só para os tmux `names` (usado nos testes do script).
#[cfg(test)]
fn poll_script(names: &[String]) -> String {
    let calls: Vec<String> = names.iter().map(|name| format!("chube_poll {}", quote(name))).collect();
    format!("{POLL_FUNCTION}{}\n", calls.join("\n"))
}

/// Para cada tmux: o `claude` é o processo do painel ou um filho dele (no chat do Chube, o `env`
/// que roda o `claude` é filho do `bash -lc`; num tmux de fora, o `claude` é filho do shell).
const POLL_FUNCTION: &str = r#"chube_poll() {
  t=$1; sid=; st=; l=
  pp=$(tmux display-message -p -t "$t" '#{pane_pid}' 2>/dev/null) || return 0
  f=
  for p in $pp $(pgrep -P "$pp" 2>/dev/null); do
    [ -f "$HOME/.claude/sessions/$p.json" ] && { f="$HOME/.claude/sessions/$p.json"; break; }
  done
  if [ -n "$f" ]; then
    sid=$(grep -o '"sessionId":"[^"]*"' "$f" | head -n 1 | cut -d '"' -f 4)
    st=$(grep -o '"status":"[^"]*"' "$f" | head -n 1 | cut -d '"' -f 4)
  fi
  if [ -n "$sid" ]; then
    jf=$(ls "$HOME"/.claude/projects/*/"$sid".jsonl 2>/dev/null | head -n 1)
    [ -n "$jf" ] && l=$(tail -c 524288 "$jf" | grep '"type":"assistant"' | grep -v '"isSidechain":true' | tail -n 1)
  fi
  m=$(printf '%s' "$l" | grep -o '"model":"[^"]*"' | head -n 1 | cut -d '"' -f 4)
  u=$(printf '%s' "$l" | grep -o '"usage":{[^}]*' | head -n 1)
  printf 'C\t%s\t%s\t%s\t%s\t%s\n' "$t" "$sid" "$st" "$m" "$u"
}
"#;

fn parse_poll(text: &str) -> Vec<RemoteStatus> {
    text.lines()
        .filter_map(|line| line.strip_prefix("C\t"))
        .filter_map(|rest| {
            let fields: Vec<&str> = rest.splitn(5, '\t').collect();
            let field = |index: usize| fields.get(index).map(|value| value.trim()).filter(|value| !value.is_empty());
            Some(RemoteStatus {
                tmux: field(0)?.to_owned(),
                session_id: field(1).map(str::to_owned),
                status: field(2).map(str::to_owned),
                model: field(3).map(str::to_owned),
                context_tokens: field(4).map_or(0, usage_tokens),
            })
        })
        .collect()
}

/// Tokens de contexto no começo do `usage` cortado pelo `grep -o` (até o primeiro `}`, o de um
/// objeto aninhado): a entrada, a criação e a leitura do cache, como nesta máquina.
fn usage_tokens(usage: &str) -> u32 {
    let body = usage.strip_prefix("\"usage\":{").unwrap_or(usage);
    body.split(',')
        .filter_map(|pair| pair.split_once(':'))
        .filter(|(key, _)| {
            matches!(
                key.trim().trim_matches('"'),
                "input_tokens" | "cache_creation_input_tokens" | "cache_read_input_tokens"
            )
        })
        .filter_map(|(_, value)| value.trim().parse::<u64>().ok())
        .sum::<u64>()
        .min(u64::from(u32::MAX)) as u32
}

/// Sessão do tmux aberta na VPS.
#[derive(Clone, Debug, PartialEq)]
pub struct TmuxSession {
    pub name: String,
    pub path: String,
    /// Título do painel (o agente põe o assunto da conversa).
    pub title: String,
    pub attached: bool,
}

/// O que a VPS tem: sessões salvas do Claude Code e sessões do tmux abertas.
#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub sessions: Vec<SavedSession>,
    pub tmux: Vec<TmuxSession>,
}

/// Linhas `T` das sessões do tmux (nome, se há alguém conectado, pasta e título do painel).
const LIST_TMUX: &str =
    "tmux list-sessions -F 'T\t#{session_name}\t#{session_attached}\t#{pane_current_path}\t#{pane_title}' 2>/dev/null";

/// Lista as sessões da VPS. Só lê os campos usados na lista (pasta e título), com `grep -o`,
/// para não trazer o histórico inteiro pela rede.
pub async fn list(host: String) -> Result<Listing, String> {
    const SCRIPT: &str = r#"
printf 'H\t%s\n' "$HOME"
TMUX_LIST
cd ~/.claude/projects 2>/dev/null || exit 0
ls -t -- */*.jsonl 2>/dev/null | head -n 150 | while IFS= read -r f; do
  printf 'F\t%s\t%s\n' "$(stat -c %Y -- "$f")" "$f"
  grep -m1 -o '"cwd":"[^"]*"' -- "$f"
  grep -o '"\(customTitle\|aiTitle\|lastPrompt\)":"[^"]*' -- "$f" | tail -n 3
done
"#;
    let text = run(&host, &SCRIPT.replace("TMUX_LIST", LIST_TMUX)).await?;
    Ok(parse_listing(&host, &text))
}

fn parse_listing(host: &str, text: &str) -> Listing {
    let mut listing = Listing::default();
    let mut home = String::new();
    // Sessão salva em montagem: id, data, pasta e os títulos achados.
    let mut current: Option<(String, SystemTime, Option<String>, [Option<String>; 3])> = None;
    let finish = |current: Option<(String, SystemTime, Option<String>, [Option<String>; 3])>,
                      listing: &mut Listing| {
        let Some((id, modified, cwd, [custom, ai, prompt])) = current else { return };
        let (Some(cwd), Some(title)) = (cwd, custom.or(ai).or(prompt)) else { return };
        listing.sessions.push(SavedSession {
            agent: Agent::Claude,
            id,
            cwd: PathBuf::from(cwd),
            title: title.lines().next().unwrap_or_default().chars().take(120).collect(),
            modified,
            running: None,
            host: Some(host.to_owned()),
        });
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("H\t") {
            home = rest.to_owned();
        } else if let Some(rest) = line.strip_prefix("T\t") {
            let fields: Vec<&str> = rest.splitn(4, '\t').collect();
            if let [name, attached, path, title] = fields[..] {
                listing.tmux.push(TmuxSession {
                    name: name.to_owned(),
                    attached: attached != "0",
                    path: tilde(path, &home),
                    title: title.to_owned(),
                });
            }
        } else if let Some(rest) = line.strip_prefix("F\t") {
            finish(current.take(), &mut listing);
            let Some((time, path)) = rest.split_once('\t') else { continue };
            let Some(id) = Path::new(path).file_stem().and_then(|stem| stem.to_str()) else { continue };
            let seconds = time.parse::<u64>().unwrap_or(0);
            current = Some((id.to_owned(), SystemTime::UNIX_EPOCH + Duration::from_secs(seconds), None, Default::default()));
        } else if let Some((_, _, cwd, titles)) = current.as_mut() {
            let Some((key, value)) = line.split_once("\":\"") else { continue };
            let value = unescape(value.trim_end_matches('"'));
            match key.trim_start_matches('"') {
                "cwd" => *cwd = Some(tilde(&value, &home)),
                "customTitle" => titles[0] = Some(value),
                "aiTitle" => titles[1] = Some(value),
                "lastPrompt" => titles[2] = Some(value),
                _ => {}
            }
        }
    }
    finish(current.take(), &mut listing);
    listing
}

/// Os escapes do JSON que aparecem num título (o `grep` corta antes de uma aspa escapada).
fn unescape(text: &str) -> String {
    serde_json::from_str::<String>(&format!("\"{}\"", text.trim_end_matches('\\')))
        .unwrap_or_else(|_| text.to_owned())
}

/// Caminho da VPS com `~` no lugar da home de lá.
fn tilde(path: &str, home: &str) -> String {
    if home.is_empty() {
        return path.to_owned();
    }
    match path.strip_prefix(home) {
        Some("") => "~".into(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_do_ssh_config() {
        let config = "Host minha-vps\n    HostName 1.2.3.4\nHost *\n  User x\nhost outro \"terceiro\" minha-vps\n";
        assert_eq!(parse_hosts(config), ["minha-vps", "outro", "terceiro"]);
    }

    #[test]
    fn aspas_do_shell() {
        assert_eq!(quote("simples/caminho-1"), "simples/caminho-1");
        assert_eq!(quote("com espaço"), "'com espaço'");
        assert_eq!(quote("it's $HOME"), r"'it'\''s $HOME'");
        assert_eq!(quote(""), "''");
        assert_eq!(quote_dir("~"), "\"$HOME\"");
        assert_eq!(quote_dir("~/meus projetos"), "\"$HOME\"'/meus projetos'");
        assert_eq!(quote_dir("/srv/app"), "/srv/app");
    }

    #[test]
    fn comando_do_chat() {
        let args = vec!["--session-id".to_owned(), "abc".to_owned(), "--".to_owned(), "oi; rm -rf /".to_owned()];
        let agent = agent_shell_command(Agent::Claude, &args, None);
        assert_eq!(
            agent,
            "export PATH=\"$HOME/.local/bin:$PATH\"; env -u ANTHROPIC_API_KEY claude --session-id abc -- 'oi; rm -rf /'"
        );
        let (program, args) = command("minha-vps", "~/x", "chube-1234", Some(&agent));
        assert_eq!(program, "ssh");
        assert_eq!(args[0], "-t");
        let script = args.last().unwrap();
        assert_eq!(args[args.len() - 2], "minha-vps");
        assert!(script.starts_with("cd \"$HOME\"/x 2>/dev/null || cd; exec tmux new-session -A -s chube-1234 "));
        assert!(script.contains("bash -lc 'export PATH="));
        assert!(script.contains("set-option -t chube-1234 status off"));
        let (_, attach) = command("minha-vps", "~", "do celular", None);
        assert!(attach.last().unwrap().starts_with("exec tmux attach-session -t 'do celular' \\; "));
    }

    #[test]
    fn sessao_nova_volta_a_comecar_se_nao_houver_historico() {
        let resume = vec!["--resume".to_owned(), "abc".to_owned()];
        let new = vec!["--session-id".to_owned(), "abc".to_owned()];
        let command = agent_shell_command(Agent::Claude, &resume, Some(&new));
        assert!(command.ends_with("claude --resume abc || env -u ANTHROPIC_API_KEY claude --session-id abc"));
    }

    #[test]
    fn lista_da_vps() {
        let text = "H\t/home/user\n\
T\tchube-1234\t0\t/home/user/projects/x\t✳ Corrigir testes\n\
T\tcelular\t1\t/srv\tbash\n\
F\t1794617482\tprojeto/abc.jsonl\n\
\"cwd\":\"/home/user/projects/x\"\n\
\"aiTitle\":\"Título da IA\n\
\"customTitle\":\"Meu t\\u00edtulo\n\
F\t1794617000\toutro/sem-titulo.jsonl\n\
\"cwd\":\"/tmp\"\n";
        let listing = parse_listing("minha-vps", text);
        assert_eq!(listing.tmux.len(), 2);
        assert_eq!(listing.tmux[0].path, "~/projects/x");
        assert!(!listing.tmux[0].attached && listing.tmux[1].attached);
        assert_eq!(listing.sessions.len(), 1);
        let saved = &listing.sessions[0];
        assert_eq!(saved.id, "abc");
        assert_eq!(saved.cwd, PathBuf::from("~/projects/x"));
        assert_eq!(saved.title, "Meu título");
        assert_eq!(saved.host.as_deref(), Some("minha-vps"));
    }

    #[test]
    fn estado_dos_chats_da_vps() {
        let text = "lixo\n\
C\tchube-1\tabc\twaiting\tclaude-opus-5-5\t\"usage\":{\"input_tokens\":3,\"cache_creation_input_tokens\":1000,\"cache_read_input_tokens\":80000,\"cache_creation\":{\"ephemeral_5m_input_tokens\":0\n\
C\tcelular\t\t\t\t\n";
        let list = parse_poll(text);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].tmux, "chube-1");
        assert_eq!(list[0].session_id.as_deref(), Some("abc"));
        assert_eq!(list[0].status.as_deref(), Some("waiting"));
        assert_eq!(list[0].model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(list[0].context_tokens, 81_003);
        assert_eq!(list[1], RemoteStatus { tmux: "celular".into(), ..Default::default() });
    }

    #[test]
    fn script_de_estado_com_nomes_entre_aspas() {
        let script = poll_script(&["chube-1".into(), "do celular".into()]);
        assert!(script.starts_with("chube_poll() {"));
        assert!(script.ends_with("chube_poll chube-1\nchube_poll 'do celular'\n"));
    }
}
