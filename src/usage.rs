//! Consumo do plano, lido com o `/usage` do Claude Code.
//!
//! `claude -p --no-session-persistence "/usage"` roda o comando localmente: não chama o modelo,
//! não gasta limite e não cria sessão salva. A saída é texto, por exemplo:
//!
//! ```text
//! Current session: 34% used · resets Sep 27, 12:50am (America/Sao_Paulo)
//! Current week (all models): 29% used · resets Sep 30, 5am (America/Sao_Paulo)
//! ```
//!
//! Do mesmo jeito, `/context` com `--model` dá o tamanho da janela de contexto de um modelo
//! (`**Tokens:** 11.3k / 1m (1%)`), que o histórico da sessão não grava.

use std::process::Stdio;
use std::time::Duration;

use chrono::{Datelike, Local, NaiveDateTime, TimeZone};
use serde_json::Value;

use crate::claude::{claude_bin, home_dir};

const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq)]
pub enum Window {
    /// Janela de 5 horas.
    Session,
    /// Limite semanal; o texto entre parênteses, como `all models`.
    Week(String),
}

#[derive(Clone, Debug)]
pub struct Limit {
    pub window: Window,
    /// Fração usada, de 0 a 1.
    pub used: f32,
    pub resets: Option<chrono::DateTime<Local>>,
}

pub async fn fetch() -> Result<Vec<Limit>, String> {
    let text = run_local("/usage", None).await?;
    let limits = parse(&text);
    if limits.is_empty() {
        let first_line = text.lines().next().unwrap_or("resposta vazia");
        return Err(format!("não achei os limites no /usage: {first_line}"));
    }
    Ok(limits)
}

/// Tamanho da janela de contexto de `model`, em tokens.
pub async fn context_window(model: String) -> Result<u32, String> {
    let text = run_local("/context", Some(&model)).await?;
    parse_window(&text).ok_or_else(|| format!("não achei a janela de contexto no /context de {model}"))
}

/// Roda um comando `/…` local do Claude Code e devolve o texto da saída.
async fn run_local(command: &str, model: Option<&str>) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new(claude_bin());
    cmd.args(["-p", "--no-session-persistence", "--output-format", "json"]);
    if let Some(model) = model {
        cmd.args(["--model", model]);
    }
    let output = cmd
        .arg(command)
        .current_dir(home_dir())
        .env_remove("ANTHROPIC_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(TIMEOUT, output)
        .await
        .map_err(|_| format!("o {command} demorou demais"))?
        .map_err(|err| format!("não consegui rodar o claude: {err}"))?;

    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| format!("resposta inesperada do {command}"))?;
    Ok(value["result"].as_str().unwrap_or_default().to_owned())
}

/// `**Tokens:** 11.3k / 1m (1%)` → 1 000 000.
fn parse_window(text: &str) -> Option<u32> {
    let line = text.lines().find(|line| line.contains("Tokens:"))?;
    let (_, rest) = line.split_once(" / ")?;
    let size = rest.split_whitespace().next()?.to_ascii_lowercase();
    let (number, scale) = match size.chars().last()? {
        'k' => (&size[..size.len() - 1], 1_000.0),
        'm' => (&size[..size.len() - 1], 1_000_000.0),
        _ => (size.as_str(), 1.0),
    };
    let window = number.parse::<f64>().ok()? * scale;
    (window >= 1.0).then(|| window.round() as u32)
}

fn parse(text: &str) -> Vec<Limit> {
    text.lines().filter_map(parse_line).collect()
}

/// `Current week (all models): 29% used · resets Sep 30, 5am (America/Sao_Paulo)`
fn parse_line(line: &str) -> Option<Limit> {
    let rest = line.trim().strip_prefix("Current ")?;
    let (name, rest) = rest.split_once(": ")?;
    let window = if name == "session" {
        Window::Session
    } else {
        let label = name.strip_prefix("week")?.trim();
        Window::Week(label.trim_start_matches('(').trim_end_matches(')').to_owned())
    };

    let (percent, rest) = rest.split_once("% used")?;
    let used = percent.trim().parse::<f32>().ok()? / 100.0;
    let resets = rest
        .split_once("resets ")
        .and_then(|(_, when)| parse_reset(when));
    Some(Limit {
        window,
        used,
        resets,
    })
}

/// `Sep 30, 5am (America/Sao_Paulo)` ou `Sep 27, 12:50am (…)`, sem o ano.
fn parse_reset(text: &str) -> Option<chrono::DateTime<Local>> {
    let mut text = text.split(" (").next()?.trim().to_uppercase();
    // "5AM" vira "5:00AM": o chrono não aceita horário sem minutos.
    let hour = text.rsplit(' ').next()?;
    if !hour.contains(':') {
        let at = text.len() - 2;
        text.insert_str(at, ":00");
    }
    let now = Local::now();
    for year in [now.year(), now.year() + 1] {
        let with_year = format!("{year} {text}");
        let parsed = NaiveDateTime::parse_from_str(&with_year, "%Y %b %d, %I:%M%p").ok();
        // O reinício é sempre no futuro; em dezembro, "Jan 2" é do ano seguinte.
        if let Some(time) = parsed.and_then(|naive| Local.from_local_datetime(&naive).single()) {
            if time > now - chrono::Duration::hours(1) {
                return Some(time);
            }
        }
    }
    None
}

/// "reinicia em 1 h 20 min" ou "reinicia 30/09 às 05:00".
pub fn describe_reset(time: chrono::DateTime<Local>) -> String {
    let minutes = (time - Local::now()).num_minutes().max(0);
    if minutes < 60 {
        format!("reinicia em {minutes} min")
    } else if minutes < 24 * 60 {
        format!("reinicia em {} h {} min", minutes / 60, minutes % 60)
    } else {
        format!("reinicia {}", time.format("%d/%m às %H:%M"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_a_janela_do_context() {
        let text = "## Context Usage\n\n**Model:** claude-opus-5-5  \n**Tokens:** 11.3k / 1m (1%)\n";
        assert_eq!(parse_window(text), Some(1_000_000));
        assert_eq!(parse_window("**Tokens:** 17.8k / 200k (9%)"), Some(200_000));
        assert_eq!(parse_window("**Tokens:** 0 / 1.5m (0%)"), Some(1_500_000));
        assert_eq!(parse_window("sem janela"), None);
    }

    #[test]
    fn le_a_saida_do_usage() {
        let text = "You are currently using your subscription\n\n\
            Current session: 34% used · resets Sep 27, 12:50am (America/Sao_Paulo)\n\
            Current week (all models): 29% used · resets Sep 30, 5am (America/Sao_Paulo)\n\
            Current week (Sonnet only): 3% used\n\nLast 24h · 265 requests";
        let limits = parse(text);
        assert_eq!(limits.len(), 3);
        assert_eq!(limits[0].window, Window::Session);
        assert!((limits[0].used - 0.34).abs() < 1e-6);
        assert_eq!(limits[1].window, Window::Week("all models".into()));
        assert!((limits[1].used - 0.29).abs() < 1e-6);
        assert_eq!(limits[2].window, Window::Week("Sonnet only".into()));
        assert!(limits[2].resets.is_none());
    }

    #[test]
    fn horario_de_reinicio() {
        let reset = parse_reset("Dec 31, 11:59pm (America/Sao_Paulo)").expect("com minutos");
        assert_eq!(reset.format("%m-%d %H:%M").to_string(), "12-31 23:59");
        let reset = parse_reset("Dec 31, 5am (America/Sao_Paulo)").expect("sem minutos");
        assert_eq!(reset.format("%H:%M").to_string(), "05:00");
    }
}
