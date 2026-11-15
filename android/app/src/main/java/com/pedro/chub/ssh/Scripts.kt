package com.pedro.chub.ssh

import java.util.UUID

/** Agente que roda no tmux. */
enum class Agent(val key: String) {
    CLAUDE("claude"),
    CODEX("codex"),
}

/** Um chat novo (ou uma sessão salva retomada) num host. */
data class NewChat(
    val agent: Agent,
    /** Pasta no host; vazio = a pasta padrão do host. */
    val dir: String,
    val model: String? = null,
    val prompt: String? = null,
    /** Modo do `omarchy agent`: o agente aprova sozinho. Só no notebook. */
    val auto: Boolean = false,
    /** Sessão salva do Claude Code a retomar (`--resume`). */
    val resumeId: String? = null,
)

/**
 * Os scripts que o celular roda nos hosts. São os mesmos do C-hub desktop (`src/remote.rs`), para os
 * dois verem e criarem as mesmas sessões do tmux: `chube-<8 hex>`, sem barra, mouse ligado, título
 * do agente repassado ao terminal e sem prefixo.
 */
object Scripts {
    const val PREFIX = "chube-"

    /**
     * O tmux do host. No notebook, os chats do desktop rodam num servidor próprio (`-L chube`,
     * sem a configuração do usuário); o celular entra no mesmo. O `-u` força UTF-8: a sessão do sshd
     * não traz `LANG`, e sem ele o tmux troca os símbolos do agente por `_`.
     */
    fun tmux(host: Host): String {
        val socket = host.tmuxSocket?.takeIf { it.isNotBlank() } ?: return "tmux -u"
        return "tmux -u -L ${Shell.quote(socket)} -f /dev/null"
    }

    private const val LANG = "export LANG=\"\${LANG:-C.UTF-8}\"; "

    fun newName(): String = PREFIX + UUID.randomUUID().toString().replace("-", "").take(8)

    /** Linha do shell (para o `bash -lc`) que roda o agente. */
    fun agentLine(chat: NewChat): String {
        val (binary, unset) = when (chat.agent) {
            Agent.CLAUDE -> "claude" to "-u ANTHROPIC_API_KEY"
            Agent.CODEX -> "codex" to "-u OPENAI_API_KEY -u CODEX_API_KEY"
        }
        val args = mutableListOf<String>()
        when (chat.agent) {
            Agent.CLAUDE -> {
                if (chat.resumeId != null) args += listOf("--resume", chat.resumeId)
                else args += listOf("--session-id", UUID.randomUUID().toString())
                if (chat.auto) args += listOf("--permission-mode", "auto")
            }
            Agent.CODEX -> {
                if (chat.resumeId != null) args += listOf("resume", chat.resumeId)
                if (chat.auto) args += "--approve-for-me"
                // O desktop reconhece a conversa do Codex por este título.
                args += listOf("-c", "tui.terminal_title=[\"thread-id\"]")
            }
        }
        chat.model?.takeIf { it.isNotBlank() }?.let { args += listOf("--model", it.trim()) }
        // `--` para um prompt que comece com `-` não virar opção.
        chat.prompt?.takeIf { it.isNotBlank() }?.let { args += listOf("--", it) }
        val words = args.joinToString(" ") { Shell.quote(it) }
        // Num login sem terminal, o PATH pode não ter `~/.local/bin`, onde fica o `claude`.
        return "export PATH=\"\$HOME/.local/bin:\$PATH\"; env $unset $binary $words".trimEnd()
    }

    private fun options(name: String): String {
        val q = Shell.quote(name)
        return listOf(
            "set-option -t $q status off",
            "set-option -t $q mouse on",
            "set-option -t $q set-titles on",
            "set-option -t $q set-titles-string '#{pane_title}'",
            // Na mesma sessão que o desktop, a sobra do maior fica em branco (não pontilhada).
            "set-option -wq -t $q fill-character ' '",
            "set-option -as terminal-features ',xterm-256color:RGB'",
        ).joinToString(" \\; ")
    }

    /** Cria a sessão `name` rodando o agente (ou entra nela, se já existe). */
    fun create(host: Host, name: String, chat: NewChat): String {
        val q = Shell.quote(name)
        val dir = Shell.quoteDir(chat.dir.ifBlank { host.defaultDir })
        val agent = Shell.quote(agentLine(chat))
        return "${LANG}cd $dir 2>/dev/null || cd; exec ${tmux(host)} new-session -A -s $q -e COLORTERM=truecolor bash -lc $agent" +
            " \\; ${options(name)} \\; set-option -t $q prefix None \\; set-option -t $q prefix2 None"
    }

    /** Entra numa sessão que já existe. */
    fun attach(host: Host, name: String): String =
        "${LANG}exec ${tmux(host)} attach-session -t ${Shell.quote(name)} \\; ${options(name)}"

    fun kill(host: Host, name: String): String = "${tmux(host)} kill-session -t ${Shell.quote(name)}"

    /** Separador dos campos do estado: não é espaço, então campos vazios não somem no `read`. */
    const val SEP = '\u001f'

    /**
     * Laço do estado: a cada 3 s, uma linha `S` por sessão do tmux (nome, se há alguém conectado,
     * pasta, título do painel, última atividade e o que o `~/.claude/sessions/<pid>.json` do agente
     * diz) e um `E` no fim. Começa com `H` e a home. Termina quando a conexão fecha (SIGPIPE no echo).
     */
    fun poll(host: Host): String = """
export LANG="${'$'}{LANG:-C.UTF-8}"
T=${Shell.quote(tmux(host))}
S=$(printf '\037')
printf 'H%s%s\n' "${'$'}S" "${'$'}HOME"
cpoll() {
  t=${'$'}1; sid=; st=; l=
  pp=${'$'}(${'$'}T display-message -p -t "${'$'}t" '#{pane_pid}' 2>/dev/null) || return 0
  f=
  for p in ${'$'}pp ${'$'}(pgrep -P "${'$'}pp" 2>/dev/null); do
    [ -f "${'$'}HOME/.claude/sessions/${'$'}p.json" ] && { f="${'$'}HOME/.claude/sessions/${'$'}p.json"; break; }
  done
  if [ -n "${'$'}f" ]; then
    sid=${'$'}(grep -o '"sessionId":"[^"]*"' "${'$'}f" | head -n 1 | cut -d '"' -f 4)
    st=${'$'}(grep -o '"status":"[^"]*"' "${'$'}f" | head -n 1 | cut -d '"' -f 4)
  fi
  if [ -n "${'$'}sid" ]; then
    jf=${'$'}(ls "${'$'}HOME"/.claude/projects/*/"${'$'}sid".jsonl 2>/dev/null | head -n 1)
    [ -n "${'$'}jf" ] && l=${'$'}(tail -c 524288 "${'$'}jf" | grep '"type":"assistant"' | grep -v '"isSidechain":true' | tail -n 1)
  fi
  m=${'$'}(printf '%s' "${'$'}l" | grep -o '"model":"[^"]*"' | head -n 1 | cut -d '"' -f 4)
  u=${'$'}(printf '%s' "${'$'}l" | grep -o '"usage":{[^}]*' | head -n 1)
  printf '%s%s%s%s%s%s%s' "${'$'}sid" "${'$'}S" "${'$'}st" "${'$'}S" "${'$'}m" "${'$'}S" "${'$'}u"
}
while :; do
  ${'$'}T list-sessions -F "#{session_name}${'$'}S#{session_attached}${'$'}S#{pane_current_path}${'$'}S#{pane_title}${'$'}S#{session_activity}" 2>/dev/null |
  while IFS="${'$'}S" read -r n a p ti act; do
    printf 'S%s%s%s%s%s%s%s%s%s%s%s%s\n' "${'$'}S" "${'$'}n" "${'$'}S" "${'$'}a" "${'$'}S" "${'$'}p" "${'$'}S" "${'$'}ti" "${'$'}S" "${'$'}act" "${'$'}S" "${'$'}(cpoll "${'$'}n")"
  done
  echo E
  sleep 3
done
""".trimStart()

    /**
     * As últimas sessões salvas do Claude Code no host, só com os campos da lista (pasta e
     * títulos), lidos com `grep -o` para não trazer o histórico pela rede. Igual ao desktop.
     */
    const val SAVED = """
printf 'H\t%s\n' "${'$'}HOME"
cd ~/.claude/projects 2>/dev/null || exit 0
ls -t -- */*.jsonl 2>/dev/null | head -n 100 | while IFS= read -r f; do
  printf 'F\t%s\t%s\n' "${'$'}(stat -c %Y -- "${'$'}f")" "${'$'}f"
  grep -m1 -o '"cwd":"[^"]*"' -- "${'$'}f"
  grep -o '"\(customTitle\|aiTitle\|lastPrompt\)":"[^"]*' -- "${'$'}f" | tail -n 3
done
"""
}

/** Aspas do shell. */
object Shell {
    /** Aspas simples: o texto passa literal, mesmo com espaços, `$` ou aspas. */
    fun quote(text: String): String {
        if (text.isNotEmpty() && text.all { (it.code < 128 && it.isLetterOrDigit()) || it in "-_./=:@%+," }) return text
        return "'" + text.replace("'", "'\\''") + "'"
    }

    /** Pasta para o `cd`: `~` e `~/…` viram `"$HOME"…` (o til entre aspas não expande). */
    fun quoteDir(dir: String): String = when {
        dir == "~" || dir.isEmpty() -> "\"\$HOME\""
        dir.startsWith("~/") -> "\"\$HOME\"" + quote(dir.substring(1))
        else -> quote(dir)
    }
}
