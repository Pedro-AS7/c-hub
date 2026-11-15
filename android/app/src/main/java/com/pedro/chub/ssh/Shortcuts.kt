package com.pedro.chub.ssh

import org.json.JSONArray
import org.json.JSONObject

/** Um atalho de pasta: nome curto que o campo de pasta aceita (`proj` ou `proj/src`). */
data class Shortcut(val name: String, val path: String, val fshort: Boolean = false)

/**
 * Atalhos de pasta, como no desktop (`src/dir_shortcuts.rs`). Os de um host ficam nele, em
 * `~/.config/chube/config.json` (`shortcuts`, lista de `{name, path}`, caminho absoluto): os do
 * notebook são os mesmos do C-hub desktop. Os do `fshort` (`~/.shortcuts`) também valem e podem ser
 * editados ou excluídos (muda o arquivo dele, então vale para o `go` do terminal); um atalho do C-hub
 * com o mesmo nome passa na frente.
 */
object Shortcuts {
    /** Lê o `config.json` (inteiro, para regravar sem perder as outras chaves) e o `~/.shortcuts`. */
    const val READ_CONFIG = "cat \"\${XDG_CONFIG_HOME:-\$HOME/.config}/chube/config.json\" 2>/dev/null; true"
    const val READ_FSHORT = "cat \"\${SHORTCUTS_FILE:-\$HOME/.shortcuts}\" 2>/dev/null; true"

    /** Grava a entrada padrão como o novo `config.json` (arquivo temporário + `mv`, como o desktop). */
    const val WRITE_CONFIG = "d=\"\${XDG_CONFIG_HOME:-\$HOME/.config}/chube\"; mkdir -p \"\$d\" && " +
        "cat > \"\$d/config.json.tmp\" && mv \"\$d/config.json.tmp\" \"\$d/config.json\""

    /** Grava a entrada padrão como o novo `~/.shortcuts` (`.tmp` + `mv`, como o `fshort`). */
    const val WRITE_FSHORT = "f=\"\${SHORTCUTS_FILE:-\$HOME/.shortcuts}\"; cat > \"\$f.tmp\" && mv \"\$f.tmp\" \"\$f\""

    /** Regra de nome do `fshort` (`^[A-Za-z0-9._-]+$`): o nome editado tem que valer no `go`. */
    fun checkFshortName(raw: String): Result<String> {
        val name = checkName(raw).getOrElse { return Result.failure(it) }
        return if (name.all { (it.code < 128 && it.isLetterOrDigit()) || it in "._-" }) Result.success(name)
        else Result.failure(IllegalArgumentException("No fshort o nome só pode ter letras, números, . _ -"))
    }

    /** O `~/.shortcuts` com a linha de `old` trocada por `name=path`, no mesmo lugar (igual ao desktop). */
    fun editFshort(text: String, old: String, name: String, path: String): Result<String> {
        val clean = checkFshortName(name).getOrElse { return Result.failure(it) }
        fun key(line: String) = line.split('=', limit = 2).takeIf { it.size == 2 }?.get(0)?.trim()
        if (clean != old && text.lines().any { key(it) == clean }) {
            return Result.failure(IllegalArgumentException("Já existe um atalho \"$clean\""))
        }
        var found = false
        val out = text.lines().filter { it.isNotEmpty() }.mapNotNull { line ->
            if (key(line) == old) {
                if (found) null else { found = true; "$clean=$path" }
            } else line
        }
        if (!found) return Result.failure(IllegalArgumentException("O atalho \"$old\" não está mais no ~/.shortcuts"))
        return Result.success(out.joinToString("\n") + "\n")
    }

    /** O `~/.shortcuts` sem as linhas de `name` (como o `fshort -d`). */
    fun removeFshort(text: String, name: String): String {
        val lines = text.lines().filter { it.isNotEmpty() && !it.startsWith("$name=") }
        return if (lines.isEmpty()) "" else lines.joinToString("\n") + "\n"
    }

    fun own(config: String): List<Shortcut> {
        val array = runCatching { JSONObject(config).optJSONArray("shortcuts") }.getOrNull() ?: return emptyList()
        return (0 until array.length()).mapNotNull { i ->
            val item = array.optJSONObject(i) ?: return@mapNotNull null
            val name = checkName(item.optString("name")).getOrNull() ?: return@mapNotNull null
            val path = item.optString("path").ifEmpty { return@mapNotNull null }
            Shortcut(name, path)
        }.distinctBy { it.name }.sortedBy { it.name }
    }

    fun fshort(text: String): List<Shortcut> = text.lineSequence().mapNotNull { line ->
        val (name, path) = line.split('=', limit = 2).takeIf { it.size == 2 } ?: return@mapNotNull null
        Shortcut(name.trim(), path.trim(), fshort = true).takeIf { it.name.isNotEmpty() && it.path.isNotEmpty() }
    }.toList()

    fun merge(own: List<Shortcut>, fshort: List<Shortcut>): List<Shortcut> =
        own + fshort.filter { f -> own.none { it.name == f.name } }

    /** O `config.json` com os atalhos novos, mantendo as outras chaves (tema etc.). */
    fun withShortcuts(config: String, own: List<Shortcut>): String {
        val json = runCatching { JSONObject(config) }.getOrElse { JSONObject() }
        json.put("shortcuts", JSONArray(own.sortedBy { it.name }.map { JSONObject().put("name", it.name).put("path", it.path) }))
        return json.toString(2) + "\n"
    }

    /** Mesmas regras do desktop: vira a primeira parte de um caminho (`nome/sub`). */
    fun checkName(raw: String): Result<String> {
        val name = raw.trim()
        val error = when {
            name.isEmpty() -> "Digite um nome para o atalho"
            name.contains('/') || name.any(Char::isWhitespace) -> "O nome não pode ter / nem espaços"
            name == "." || name == ".." || name.startsWith("~") -> "\"$name\" não pode ser nome de atalho"
            name.codePointCount(0, name.length) > 32 -> "Nome comprido demais (máximo 32 letras)"
            else -> return Result.success(name)
        }
        return Result.failure(IllegalArgumentException(error))
    }

    /** Cria, troca a pasta ou (com `old`) edita um atalho. */
    fun upsert(list: List<Shortcut>, old: String?, name: String, path: String): Result<List<Shortcut>> {
        val clean = checkName(name).getOrElse { return Result.failure(it) }
        if (old != null && clean != old && list.any { it.name == clean }) {
            return Result.failure(IllegalArgumentException("Já existe um atalho \"$clean\""))
        }
        val rest = list.filter { it.name != clean && it.name != old }
        return Result.success((rest + Shortcut(clean, path)).sortedBy { it.name })
    }

    /** Pasta digitada no atalho: `~`, `~/…` ou absoluta, gravada absoluta (o desktop não expande `~`). */
    fun expand(text: String, home: String): Result<String> {
        val t = text.trim().trimEnd('/').ifEmpty { "/" }
        return when {
            t == "~" && home.isNotEmpty() -> Result.success(home)
            t.startsWith("~/") && home.isNotEmpty() -> Result.success(home + t.substring(1))
            t.startsWith("/") -> Result.success(t)
            else -> Result.failure(IllegalArgumentException("Use uma pasta que comece com / ou ~/"))
        }
    }

    /**
     * Pasta para o `cd` a partir do campo: `~`, `~/…` e absolutas passam como estão; `nome` ou
     * `nome/sub` de um atalho viram a pasta dele; o resto é relativo à pasta padrão do host.
     * Vazio = a pasta padrão.
     */
    fun resolve(input: String, shortcuts: List<Shortcut>, defaultDir: String): String {
        val t = input.trim()
        if (t.isEmpty()) return defaultDir
        if (t == "~" || t.startsWith("~/") || t.startsWith("/")) return t
        val name = t.substringBefore('/')
        val rest = t.substringAfter('/', "")
        shortcuts.firstOrNull { it.name == name }?.let { return if (rest.isEmpty()) it.path else "${it.path}/$rest" }
        return "${defaultDir.trimEnd('/')}/$t"
    }
}
