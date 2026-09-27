mod app;
mod chat;
mod claude;
mod dirs;
mod sessions;
mod term;
mod usage;

use cosmic::iced::{Limits, Size};

/// Variáveis que um Claude Code define para os processos filhos. Herdadas (Chube aberto de
/// dentro de uma sessão), fazem o `claude` dos chats se achar aninhado e não gravar a sessão.
const CLAUDE_SESSION_VARS: [&str; 10] = [
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
];

fn main() -> cosmic::iced::Result {
    for var in CLAUDE_SESSION_VARS {
        // SAFETY: nenhuma outra thread existe ainda.
        unsafe { std::env::remove_var(var) };
    }

    let settings = cosmic::app::Settings::default()
        .size(Size::new(1400.0, 860.0))
        .size_limits(Limits::NONE.min_width(900.0).min_height(560.0));

    cosmic::app::run::<app::App>(settings, parse_args())
}

/// `chube [--model <modelo>] [--prompt <mensagem>]`
fn parse_args() -> app::Flags {
    let mut flags = app::Flags::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--model" => flags.model = args.next(),
            "--prompt" => flags.prompt = args.next(),
            _ => eprintln!("argumento ignorado: {arg}"),
        }
    }
    flags
}
