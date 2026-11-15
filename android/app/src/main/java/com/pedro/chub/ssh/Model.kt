package com.pedro.chub.ssh

import org.json.JSONArray
import org.json.JSONObject

/** Uma máquina com sessões, por SSH: o PC com o C-hub desktop ou um servidor (de preferência pelo Tailscale). */
data class Host(
    val id: String,
    val name: String,
    val address: String,
    val port: Int = 22,
    val user: String,
    /** Servidor do tmux (`-L`); `null` = o padrão. No PC com o C-hub desktop, `chube` (o dos chats dele). */
    val tmuxSocket: String? = null,
    /** Pasta de um chat novo quando o campo fica vazio. */
    val defaultDir: String = "~",
    /** Tem o agente do Omarchy (modo `auto`). */
    val omarchy: Boolean = false,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("id", id).put("name", name).put("address", address).put("port", port).put("user", user)
        .put("tmuxSocket", tmuxSocket ?: "").put("defaultDir", defaultDir).put("omarchy", omarchy)

    companion object {
        fun fromJson(json: JSONObject) = Host(
            id = json.getString("id"),
            name = json.getString("name"),
            address = json.getString("address"),
            port = json.optInt("port", 22),
            user = json.getString("user"),
            tmuxSocket = json.optString("tmuxSocket").ifBlank { null },
            defaultDir = json.optString("defaultDir", "~").ifBlank { "~" },
            omarchy = json.optBoolean("omarchy", false),
        )

        fun listToJson(hosts: List<Host>): String = JSONArray(hosts.map { it.toJson() }).toString()

        fun listFromJson(text: String): List<Host> {
            val array = JSONArray(text)
            return (0 until array.length()).map { fromJson(array.getJSONObject(it)) }
        }

        /** Servidor do tmux dos chats locais do C-hub desktop (`LOCAL_TMUX` em `src/remote.rs`). */
        const val DESKTOP_TMUX = "chube"

        /** Host novo, vazio. `desktop` = um PC com o C-hub desktop: entra no tmux dos chats dele. */
        fun blank(desktop: Boolean) = Host(
            id = "h" + System.currentTimeMillis(), name = "", address = "", port = 22, user = "",
            tmuxSocket = if (desktop) DESKTOP_TMUX else null,
        )
    }
}

enum class Status(val label: String) {
    WORKING("trabalhando"),
    WAITING("precisa de você"),
    READY("pronto"),
    /** Terminou enquanto você não olhava; volta a "pronto" ao abrir o chat. */
    DONE("terminou"),
    UNKNOWN(""),
    ;

    companion object {
        /** Status gravado pelo Claude Code em `~/.claude/sessions/<pid>.json`. */
        fun fromClaude(status: String?): Status? = when (status) {
            "busy" -> WORKING
            "waiting" -> WAITING
            "idle" -> READY
            else -> null
        }

        /** Pelo título do terminal: `✳` parado, outro símbolo (o spinner) trabalhando. */
        fun fromTitle(title: String): Status? {
            val first = title.trimStart().firstOrNull() ?: return null
            return when {
                first == '✳' -> READY
                !first.isLetterOrDigit() && first.code >= 128 -> WORKING
                else -> null
            }
        }
    }
}

/** Uma sessão do tmux num host, com o que o agente dentro dela diz. */
data class RemoteChat(
    val name: String,
    val attached: Boolean,
    /** Pasta, com `~` no lugar da home. */
    val path: String,
    val paneTitle: String,
    /** Última atividade (segundos desde 1970). */
    val activity: Long,
    val sessionId: String?,
    val claudeStatus: String?,
    val model: String?,
    val contextTokens: Int,
) {
    val status: Status get() = Status.fromClaude(claudeStatus) ?: Status.fromTitle(paneTitle) ?: Status.UNKNOWN

    /** Assunto que o agente pôs no título (sem o símbolo do status), ou o nome do tmux. */
    val title: String get() = titleFromTerminal(paneTitle) ?: name

    val isAgent: Boolean get() = sessionId != null || Status.fromTitle(paneTitle) != null

    companion object {
        fun titleFromTerminal(title: String): String? {
            val clean = title.trimStart { !it.isLetterOrDigit() && !(it.code < 128 && it in "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~") }.trim()
            return clean.takeIf { it.isNotEmpty() && it != "Claude Code" }
        }
    }
}

/** Sessão salva do Claude Code num host. */
data class SavedSession(val id: String, val cwd: String, val title: String, val modified: Long)

object Parse {
    /** Um bloco do laço de estado (as linhas antes de um `E`). */
    fun pollBatch(lines: List<String>, home: String): List<RemoteChat> = lines.mapNotNull { line ->
        val fields = line.split(Scripts.SEP)
        if (fields.size < 6 || fields[0] != "S") return@mapNotNull null
        val field = { index: Int -> fields.getOrNull(index)?.trim()?.ifEmpty { null } }
        RemoteChat(
            name = fields[1],
            attached = (field(2) ?: "0") != "0",
            path = tilde(field(3) ?: "", home),
            paneTitle = fields[4],
            activity = field(5)?.toLongOrNull() ?: 0,
            sessionId = field(6),
            claudeStatus = field(7),
            model = field(8),
            contextTokens = field(9)?.let(::usageTokens) ?: 0,
        )
    }

    /** Linha `H` do começo do laço de estado: a home do host. */
    fun home(line: String): String? =
        line.split(Scripts.SEP).takeIf { it.size == 2 && it[0] == "H" }?.get(1)

    /** Tokens de contexto no começo do `usage` cortado pelo `grep -o`: entrada e cache. */
    fun usageTokens(usage: String): Int {
        val body = usage.removePrefix("\"usage\":{")
        return body.split(',').mapNotNull { pair ->
            val (key, value) = pair.split(':', limit = 2).takeIf { it.size == 2 } ?: return@mapNotNull null
            if (key.trim().trim('"') in setOf("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")) {
                value.trim().toLongOrNull()
            } else null
        }.sum().coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
    }

    /** Saída do `Scripts.SAVED`. */
    fun saved(text: String): List<SavedSession> {
        val list = mutableListOf<SavedSession>()
        var home = ""
        var id: String? = null
        var modified = 0L
        var cwd: String? = null
        val titles = arrayOfNulls<String>(3)
        fun finish() {
            val sessionId = id ?: return
            val title = titles[0] ?: titles[1] ?: titles[2]
            if (cwd != null && title != null) {
                list += SavedSession(sessionId, cwd!!, title.lineSequence().first().take(120), modified)
            }
        }
        for (line in text.lineSequence()) {
            when {
                line.startsWith("H\t") -> home = line.removePrefix("H\t")
                line.startsWith("F\t") -> {
                    finish()
                    val parts = line.removePrefix("F\t").split('\t', limit = 2)
                    id = parts.getOrNull(1)?.substringAfterLast('/')?.removeSuffix(".jsonl")
                    modified = parts[0].toLongOrNull() ?: 0
                    cwd = null
                    titles.fill(null)
                }
                id != null && line.contains("\":\"") -> {
                    val key = line.substringBefore("\":\"").trimStart('"')
                    val value = unescape(line.substringAfter("\":\"").removeSuffix("\""))
                    when (key) {
                        "cwd" -> cwd = tilde(value, home)
                        "customTitle" -> titles[0] = value
                        "aiTitle" -> titles[1] = value
                        "lastPrompt" -> titles[2] = value
                    }
                }
            }
        }
        finish()
        return list
    }

    /** Os escapes do JSON que aparecem num título (o `grep` corta antes de uma aspa escapada). */
    fun unescape(text: String): String = runCatching {
        JSONObject("{\"v\":\"${text.trimEnd('\\')}\"}").getString("v")
    }.getOrDefault(text)

    fun tilde(path: String, home: String): String {
        if (home.isEmpty()) return path
        return when {
            path == home -> "~"
            path.startsWith("$home/") -> "~" + path.removePrefix(home)
            else -> path
        }
    }
}
