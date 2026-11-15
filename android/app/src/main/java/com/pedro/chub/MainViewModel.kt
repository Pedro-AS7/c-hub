package com.pedro.chub

import android.app.Application
import android.content.Context
import android.security.keystore.KeyPermanentlyInvalidatedException
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.pedro.chub.ssh.Connection
import com.pedro.chub.ssh.Host
import com.pedro.chub.ssh.KeyVault
import com.pedro.chub.ssh.NewChat
import com.pedro.chub.ssh.Parse
import com.pedro.chub.ssh.RemoteChat
import com.pedro.chub.ssh.SavedSession
import com.pedro.chub.ssh.Scripts
import com.pedro.chub.ssh.Secret
import com.pedro.chub.ssh.Shell
import com.pedro.chub.ssh.Shortcut
import com.pedro.chub.ssh.Shortcuts
import com.pedro.chub.ui.C
import com.pedro.chub.ui.ColorField
import com.pedro.chub.ui.Palettes
import com.pedro.chub.ui.ThemeColors
import org.json.JSONObject
import com.pedro.chub.ssh.Status
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import javax.crypto.Cipher

sealed interface Screen {
    data object Home : Screen
    data class Chat(val hostId: String, val name: String) : Screen
    data class NewChat(val hostId: String?) : Screen
    data class Sessions(val hostId: String) : Screen
    data object Settings : Screen
}

sealed interface Conn {
    data object Off : Conn
    data object Connecting : Conn
    data object On : Conn
    data class Failed(val message: String) : Conn
}

sealed interface Saved {
    data object Loading : Saved
    data class Loaded(val list: List<SavedSession>) : Saved
    data class Failed(val message: String) : Saved
}

/** Pergunta da primeira conexão (ou de uma chave de host diferente da conhecida). */
data class HostKeyAsk(val host: Host, val fingerprint: String, val type: String, val changed: Boolean) {
    val answer = CompletableDeferred<Boolean>()
}

class MainViewModel(application: Application) : AndroidViewModel(application) {
    private val context: Context get() = getApplication()
    private val prefs = context.getSharedPreferences("hosts", Context.MODE_PRIVATE)
    val vault = KeyVault(context)

    var hosts by mutableStateOf(loadHosts())
        private set
    val conn = mutableStateMapOf<String, Conn>()
    val chats = mutableStateMapOf<String, List<RemoteChat>>()
    val saved = mutableStateMapOf<String, Saved>()
    val terminals = mutableStateMapOf<String, ChatTerminal>()
    var screen by mutableStateOf<Screen>(Screen.Home)
    var notice by mutableStateOf<String?>(null)
    var hostKeyAsk by mutableStateOf<HostKeyAsk?>(null)
    var hasKey by mutableStateOf(vault.hasKey())
        private set
    var fontSize by mutableStateOf(prefs.getInt("fonte", 0))
        private set
    /** Tema escolhido: id de um tema próprio ou `personalizado` (os mesmos do desktop). */
    var themeId by mutableStateOf(prefs.getString("tema", null) ?: Palettes.DEFAULT)
        private set
    /** Tema personalizado (o JSON do `custom_theme` do desktop); `null` se nunca foi criado. */
    var customTheme by mutableStateOf(prefs.getString("tema-personalizado", null)?.let { runCatching { Palettes.fromJson(JSONObject(it)) }.getOrNull() })
        private set
    /** Atalhos de pasta de cada host, lidos dele (`Shortcuts`); `null` enquanto não carregou. */
    val shortcuts = mutableStateMapOf<String, List<Shortcut>>()
    /** Home de cada host (da conexão de estado), para gravar atalhos com caminho absoluto. */
    val homes = mutableStateMapOf<String, String>()

    /** Pede a biometria com a cifra; a Activity define (precisa de uma janela). */
    var authenticator: (suspend (Cipher, String) -> Cipher?)? = null

    private val connections = mutableMapOf<String, Connection>()
    private val polls = mutableMapOf<String, Job>()
    /** Status mostrado por chat, para o "terminou" (chave `host/nome`). */
    private val shown = mutableMapOf<String, Status>()
    private var connecting = false

    init {
        applyTheme()
    }

    fun host(id: String): Host? = hosts.firstOrNull { it.id == id }

    private fun connection(host: Host): Connection =
        connections[host.id]?.takeIf { it.host == host } ?: Connection(host).also {
            connections[host.id]?.disconnect()
            connections[host.id] = it
        }

    // ---- hosts ----

    private fun loadHosts(): List<Host> =
        prefs.getString("lista", null)?.let { runCatching { Host.listFromJson(it) }.getOrNull() } ?: emptyList()

    fun saveHosts(list: List<Host>) {
        hosts = list
        prefs.edit().putString("lista", Host.listToJson(list)).apply()
        connections.keys.filter { id -> list.none { it.id == id } }.forEach { forget(it) }
    }

    private fun forget(id: String) {
        polls.remove(id)?.cancel()
        connections.remove(id)?.disconnect()
        conn.remove(id)
        chats.remove(id)
    }

    // ---- tema ----

    private fun applyTheme() {
        C.theme = when (themeId) {
            Palettes.CUSTOM -> customTheme ?: Palettes.default
            else -> Palettes.find(themeId)?.colors ?: Palettes.default
        }
    }

    fun setTheme(id: String) {
        themeId = if (id == Palettes.CUSTOM && customTheme == null) Palettes.DEFAULT else id
        prefs.edit().putString("tema", themeId).apply()
        applyTheme()
    }

    /** Muda uma cor: copia o tema em uso para o personalizado e o escolhe (como o desktop). */
    fun setColor(field: ColorField, value: Int) {
        saveCustom(field.set(C.theme, value))
    }

    private fun saveCustom(colors: ThemeColors) {
        customTheme = colors
        themeId = Palettes.CUSTOM
        prefs.edit().putString("tema-personalizado", Palettes.toJson(colors).toString())
            .putString("tema", Palettes.CUSTOM).apply()
        applyTheme()
    }

    /**
     * Usa o tema do C-hub desktop: lê o `config.json` do PC dele (`theme` e `custom_theme`); se ele
     * segue o Omarchy, as cores vêm do `colors.toml` do tema atual de lá.
     */
    fun importDesktopTheme() {
        val online = hosts.filter { conn[it.id] == Conn.On }
        val host = online.firstOrNull { it.tmuxSocket == Host.DESKTOP_TMUX } ?: online.firstOrNull { it.omarchy } ?: run {
            notice = "conecte primeiro o PC com o C-hub desktop"
            return
        }
        viewModelScope.launch(Dispatchers.IO) {
            val result = runCatching {
                val config = runCatching { JSONObject(connection(host).exec(Shortcuts.READ_CONFIG)) }.getOrElse { JSONObject() }
                when (val id = config.optString("theme").ifEmpty { "omarchy" }) {
                    Palettes.CUSTOM -> config.optJSONObject("custom_theme")?.let { Palettes.fromJson(it) to "personalizado" }
                    "omarchy" -> connection(host).exec("cat ~/.local/state/omarchy/current/theme/colors.toml 2>/dev/null; true")
                        .takeIf { it.isNotBlank() }?.let { Palettes.parseOmarchy(it) to "do Omarchy" }
                    else -> Palettes.find(id)?.let { it.colors to it.name }
                } ?: error("o tema do desktop não foi encontrado")
            }
            withContext(Dispatchers.Main) {
                result.onSuccess { (colors, name) ->
                    // Um tema próprio igual ao do desktop fica escolhido pelo id; os outros viram o personalizado.
                    val builtin = Palettes.BUILTINS.firstOrNull { it.colors == colors }
                    if (builtin != null) setTheme(builtin.id) else saveCustom(colors)
                    notice = "tema do desktop ($name) aplicado"
                }.onFailure { notice = "não consegui ler o tema: ${it.message}" }
            }
        }
    }

    // ---- atalhos de pasta ----

    fun loadShortcuts(hostId: String) {
        val host = host(hostId) ?: return
        if (conn[hostId] != Conn.On) return
        viewModelScope.launch(Dispatchers.IO) {
            val result = runCatching {
                val connection = connection(host)
                Shortcuts.merge(Shortcuts.own(connection.exec(Shortcuts.READ_CONFIG)), Shortcuts.fshort(connection.exec(Shortcuts.READ_FSHORT)))
            }
            withContext(Dispatchers.Main) {
                result.onSuccess { shortcuts[hostId] = it }.onFailure { notice = "não consegui ler os atalhos: ${it.message}" }
            }
        }
    }

    /**
     * Cria, edita (`old`) ou apaga (`path == null`) um atalho no host. Os do C-hub ficam no
     * `config.json` (relido na hora de gravar, para não desfazer o que o desktop mudou); editar ou
     * apagar um do fshort muda o `~/.shortcuts`. `done` recebe o erro, se houver.
     */
    fun saveShortcut(hostId: String, old: String?, name: String, path: String?, done: (String?) -> Unit) {
        val host = host(hostId) ?: return
        val home = homes[hostId].orEmpty()
        val absolute = path?.let { Shortcuts.expand(it, home).getOrElse { error -> return done(error.message) } }
        val fshort = old != null && shortcuts[hostId].orEmpty().any { it.name == old && it.fshort }
        viewModelScope.launch(Dispatchers.IO) {
            val result = runCatching {
                val connection = connection(host)
                // A pasta tem que existir (falha = `erro 1`, tratado abaixo).
                if (absolute != null) connection.exec("[ -d ${Shell.quote(absolute)} ]")
                val config = connection.exec(Shortcuts.READ_CONFIG)
                var own = Shortcuts.own(config)
                var fshortText = connection.exec(Shortcuts.READ_FSHORT)
                if (fshort) {
                    // Um nome que já é atalho do C-hub ficaria escondido atrás dele.
                    if (absolute != null && name.trim() != old && own.any { it.name == name.trim() }) {
                        error("Já existe um atalho \"${name.trim()}\"")
                    }
                    fshortText = if (absolute == null) Shortcuts.removeFshort(fshortText, old!!)
                    else Shortcuts.editFshort(fshortText, old!!, name, absolute).getOrThrow()
                    connection.exec(Shortcuts.WRITE_FSHORT, input = fshortText.toByteArray())
                } else {
                    own = if (absolute == null) own.filter { it.name != name } else Shortcuts.upsert(own, old, name, absolute).getOrThrow()
                    connection.exec(Shortcuts.WRITE_CONFIG, input = Shortcuts.withShortcuts(config, own).toByteArray())
                }
                Shortcuts.merge(own, Shortcuts.fshort(fshortText))
            }
            withContext(Dispatchers.Main) {
                result.onSuccess { shortcuts[hostId] = it; done(null) }.onFailure {
                    done(if (it.message?.startsWith("erro 1") == true) "$absolute não existe em ${host.name}" else it.message)
                }
            }
        }
    }

    fun setFont(size: Int) {
        fontSize = size
        prefs.edit().putInt("fonte", size).apply()
    }

    /** Impressões digitais aceitas por host (perguntadas na primeira conexão). */
    private fun trusted(host: Host): Set<String> =
        prefs.getStringSet("conhecidas-${host.id}", null).orEmpty()

    private fun trust(host: Host, fingerprint: String, type: String): Boolean {
        val known = trusted(host)
        if (fingerprint in known) return true
        // Pergunta e espera a resposta. Estamos numa thread de conexão: o estado da tela só muda
        // na principal (escrito daqui, o Compose não redesenhava e o diálogo não aparecia). Sem
        // resposta em 2 min, recusa (o sshd já teria desistido do login).
        val ask = HostKeyAsk(host, fingerprint, type, changed = known.isNotEmpty())
        val ok = runBlocking {
            withContext(Dispatchers.Main) { hostKeyAsk = ask }
            val answer = withTimeoutOrNull(120_000) { ask.answer.await() } ?: false
            withContext(Dispatchers.Main) { if (hostKeyAsk === ask) hostKeyAsk = null }
            answer
        }
        if (ok) prefs.edit().putStringSet("conhecidas-${host.id}", known + fingerprint).apply()
        return ok
    }

    // ---- chave ----

    /** Importa a chave (texto do arquivo do Termux) e a grava cifrada; pede a biometria. */
    fun importKey(text: String, passphrase: String?) = viewModelScope.launch {
        val secret = Secret(text.trim() + "\n", passphrase?.ifEmpty { null })
        val public = withContext(Dispatchers.IO) { runCatching { KeyVault.check(secret) } }.getOrElse {
            notice = "chave inválida ou senha errada: ${it.message}"
            return@launch
        }
        val cipher = runCatching { vault.encryptCipher() }.getOrElse {
            notice = "não consegui criar a chave do Keystore: ${it.message}"
            return@launch
        }
        val unlocked = authenticator?.invoke(cipher, "Guardar a chave SSH") ?: run {
            notice = "cancelado: a chave não foi guardada"
            return@launch
        }
        vault.save(unlocked, secret, public)
        hasKey = true
        notice = "chave guardada. Apague a cópia que você exportou do Termux."
    }

    fun deleteKey() {
        connections.keys.toList().forEach { forget(it) }
        vault.delete()
        hasKey = false
    }

    // ---- conexão ----

    /** Conecta os hosts desligados (ou só `only`) com uma biometria só. */
    fun connect(only: String? = null) {
        if (connecting) return
        val targets = hosts.filter { (only == null || it.id == only) && conn[it.id] != Conn.On && conn[it.id] != Conn.Connecting }
        if (targets.isEmpty()) return
        if (!vault.hasKey()) {
            notice = "importe a chave SSH primeiro"
            screen = Screen.Settings
            return
        }
        connecting = true
        viewModelScope.launch {
            try {
                val cipher = try {
                    vault.decryptCipher()
                } catch (_: KeyPermanentlyInvalidatedException) {
                    notice = "a chave do Keystore foi invalidada: importe a chave SSH de novo"
                    return@launch
                } catch (error: Exception) {
                    notice = "não consegui abrir a chave: ${error.message}"
                    return@launch
                }
                val names = targets.joinToString(" e ") { it.name }
                val unlocked = authenticator?.invoke(cipher, "Conectar a $names") ?: return@launch
                val secret = vault.open(unlocked)
                targets.forEach { conn[it.id] = Conn.Connecting }
                targets.map { host ->
                    async(Dispatchers.IO) {
                        val result = runCatching { connection(host).connect(secret) { fp, type -> trust(host, fp, type) } }
                        withContext(Dispatchers.Main) {
                            result.onSuccess {
                                conn[host.id] = Conn.On
                                startPoll(host)
                            }.onFailure { conn[host.id] = Conn.Failed(describe(it)) }
                        }
                    }
                }.awaitAll()
            } finally {
                connecting = false
            }
        }
    }

    private fun describe(error: Throwable): String {
        val message = error.message ?: error.javaClass.simpleName
        return when {
            "Exhausted available authentication" in message -> "a chave não foi aceita (falta no authorized_keys?)"
            "Could not verify" in message -> "chave do host recusada"
            "timeout" in message.lowercase() || "timed out" in message -> "sem resposta (o Tailscale está ligado?)"
            "ECONNREFUSED" in message || "refused" in message -> "conexão recusada (o sshd está ligado?)"
            else -> message
        }
    }

    fun disconnect(hostId: String) {
        polls.remove(hostId)?.cancel()
        connections[hostId]?.disconnect()
        conn[hostId] = Conn.Off
    }

    /** O laço do estado de um host: uma lista nova a cada 3 s, até a conexão fechar. */
    private fun startPoll(host: Host) {
        polls.remove(host.id)?.cancel()
        polls[host.id] = viewModelScope.launch(Dispatchers.IO) {
            val connection = connection(host)
            var home = ""
            val error = runCatching {
                val (session, input) = connection.stream(Scripts.poll(host))
                session.use {
                    val reader = input.bufferedReader()
                    val batch = mutableListOf<String>()
                    while (isActive) {
                        val line = reader.readLine() ?: break
                        Parse.home(line)?.let { home = it; withContext(Dispatchers.Main) { homes[host.id] = it } }
                        if (line == "E") {
                            val list = Parse.pollBatch(batch, home)
                            batch.clear()
                            withContext(Dispatchers.Main) { applyPoll(host.id, list) }
                        } else {
                            batch += line
                        }
                    }
                }
            }.exceptionOrNull()
            withContext(Dispatchers.Main) {
                if (!isActive) return@withContext
                conn[host.id] = if (connection.isConnected) {
                    Conn.Failed("o estado parou: ${error?.message ?: "fim da leitura"}")
                } else {
                    Conn.Failed("conexão caiu")
                }
            }
        }
    }

    private fun applyPoll(hostId: String, list: List<RemoteChat>) {
        val viewing = (screen as? Screen.Chat)?.takeIf { it.hostId == hostId }?.name
        val result = list.map { chat ->
            val key = "$hostId/${chat.name}"
            val previous = shown[key]
            var status = chat.status
            // "pronto" depois de "trabalhando", fora da tela, vira "terminou" até o chat ser aberto.
            if (chat.name != viewing && status == Status.READY && (previous == Status.WORKING || previous == Status.DONE)) {
                status = Status.DONE
            }
            shown[key] = status
            chat
        }
        chats[hostId] = result.sortedByDescending { it.activity }
    }

    /** Status para mostrar (com o "terminou"). */
    fun shownStatus(hostId: String, chat: RemoteChat): Status = shown["$hostId/${chat.name}"] ?: chat.status

    // ---- chats ----

    fun open(hostId: String, name: String, script: String? = null) {
        val host = host(hostId) ?: return
        val key = "$hostId/$name"
        shown[key]?.let { if (it == Status.DONE) shown[key] = Status.READY }
        // Só um terminal conectado por vez: o do chat da tela (veja `release`).
        terminals.keys.filter { it != key }.forEach { release(it.substringBefore('/'), it.substringAfter('/')) }
        val existing = terminals[key]
        if (existing == null || existing.ended) {
            if (conn[hostId] != Conn.On) {
                notice = "${host.name} não está conectado"
                connect(hostId)
                return
            }
            val transport = connection(host).transport(script ?: Scripts.attach(host, name))
            terminals[key] = ChatTerminal(context, hostId, name, transport)
        }
        screen = Screen.Chat(hostId, name)
    }

    /** Cria o tmux rodando o agente e abre o terminal dele. */
    fun create(hostId: String, chat: NewChat) {
        val host = host(hostId) ?: return
        if (chat.dir.isBlank() || conn[hostId] != Conn.On) {
            val name = Scripts.newName()
            open(hostId, name, Scripts.create(host, name, chat))
            return
        }
        // Como no desktop: pasta que não existe é recusada (o `cd` cairia na home sem avisar).
        viewModelScope.launch(Dispatchers.IO) {
            val exists = runCatching { connection(host).exec("cd ${Shell.quoteDir(chat.dir)}") }.isSuccess
            withContext(Dispatchers.Main) {
                if (!exists) {
                    notice = "${chat.dir} não existe em ${host.name}"
                    return@withContext
                }
                val name = Scripts.newName()
                open(hostId, name, Scripts.create(host, name, chat))
            }
        }
    }

    /** Fecha o terminal do chat: o tmux (e o agente) continuam no host. */
    fun detach(hostId: String, name: String) {
        terminals.remove("$hostId/$name")?.session?.finishIfRunning()
        if (screen == Screen.Chat(hostId, name)) screen = Screen.Home
    }

    /**
     * Solta o terminal do chat sem sair da tela (o app foi para segundo plano). Um tmux só tem um
     * tamanho, o de quem mexeu por último: com o celular conectado sem uso, cada toque no desktop
     * ou no celular faria a janela pular de largura. Desconectado, o desktop fica com a largura dele.
     * Ao voltar, a tela do chat reconecta (o tmux redesenha a tela; o histórico é o do tmux).
     */
    fun release(hostId: String, name: String) {
        terminals.remove("$hostId/$name")?.session?.finishIfRunning()
    }

    fun releaseAll() {
        terminals.values.forEach { it.session.finishIfRunning() }
        terminals.clear()
    }

    /** Volta a entrar no tmux depois que o canal caiu. */
    fun reattach(hostId: String, name: String) {
        terminals.remove("$hostId/$name")
        open(hostId, name)
    }

    /** Mata o tmux do chat (e o agente dentro dele). */
    fun kill(hostId: String, name: String) {
        val host = host(hostId) ?: return
        detach(hostId, name)
        chats[hostId] = chats[hostId].orEmpty().filter { it.name != name }
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { connection(host).exec(Scripts.kill(host, name)) }.onFailure {
                withContext(Dispatchers.Main) { notice = "não consegui encerrar: ${it.message}" }
            }
        }
    }

    fun loadSaved(hostId: String) {
        val host = host(hostId) ?: return
        if (conn[hostId] != Conn.On) {
            saved[hostId] = Saved.Failed("${host.name} não está conectado")
            return
        }
        saved[hostId] = Saved.Loading
        viewModelScope.launch(Dispatchers.IO) {
            val result = runCatching { Parse.saved(connection(host).exec(Scripts.SAVED, 30)) }
            withContext(Dispatchers.Main) {
                saved[hostId] = result.fold({ Saved.Loaded(it) }, { Saved.Failed(it.message ?: "erro") })
            }
        }
    }

    override fun onCleared() {
        connections.values.forEach { it.disconnect() }
    }
}
