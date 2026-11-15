package com.pedro.chub.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.currentStateAsState
import com.pedro.chub.Conn
import com.pedro.chub.MainViewModel
import com.pedro.chub.Saved
import com.pedro.chub.Screen
import com.pedro.chub.ssh.Agent
import com.pedro.chub.ssh.Host
import com.pedro.chub.ssh.NewChat
import com.pedro.chub.ssh.Parse
import com.pedro.chub.ssh.Shortcut
import com.pedro.chub.ssh.Shortcuts
import com.pedro.chub.ssh.RemoteChat
import com.pedro.chub.ssh.Status
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun App(model: MainViewModel) {
    Box(Modifier.fillMaxSize().background(C.bg).safeDrawingPadding().imePadding()) {
        when (val screen = model.screen) {
            Screen.Home -> Home(model)
            is Screen.Chat -> ChatScreen(model, screen)
            is Screen.NewChat -> NewChatScreen(model, screen.hostId)
            is Screen.Sessions -> SessionsScreen(model, screen.hostId)
            Screen.Settings -> SettingsScreen(model)
        }
        model.notice?.let { message ->
            LaunchedEffect(message) {
                kotlinx.coroutines.delay(5000)
                if (model.notice == message) model.notice = null
            }
            Box(
                Modifier
                    .align(Alignment.BottomCenter)
                    .padding(16.dp)
                    .background(C.bg2)
                    .border(BorderStroke(1.dp, C.yellow.copy(alpha = 0.5f)), RectangleShape)
                    .clickable { model.notice = null }
                    .padding(12.dp),
            ) { Text(message, style = body.copy(color = C.yellow, fontSize = 13.sp)) }
        }
        model.hostKeyAsk?.let { ask ->
            Dialog(onDismissRequest = { ask.answer.complete(false) }) {
                Column(Modifier.background(C.bg2).border(BorderStroke(1.dp, C.accent), RectangleShape).padding(16.dp)) {
                    Text(if (ask.changed) "${I.WARNING} a chave de ${ask.host.name} MUDOU" else "# primeira conexão a ${ask.host.name}",
                        style = body.copy(fontWeight = FontWeight.Bold, color = if (ask.changed) C.red else C.fg))
                    Spacer(Modifier.size(8.dp))
                    Text("${ask.type}\n${ask.fingerprint}", style = small.copy(color = C.fg))
                    Spacer(Modifier.size(8.dp))
                    Text(
                        "Confira no host antes de aceitar: ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub",
                        style = small,
                    )
                    Spacer(Modifier.size(12.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        TextButton("aceitar", color = if (ask.changed) C.red else C.accent) { ask.answer.complete(true) }
                        TextButton("recusar") { ask.answer.complete(false) }
                    }
                }
            }
        }
    }
}

@Composable
private fun Header(title: String, onBack: (() -> Unit)? = null, actions: @Composable () -> Unit = {}) {
    Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        if (onBack != null) IconButton(I.BACK, onClick = onBack) else Spacer(Modifier.width(12.dp))
        Text(title, style = body.copy(fontWeight = FontWeight.Bold, fontSize = 16.sp), modifier = Modifier.weight(1f),
            maxLines = 1, overflow = TextOverflow.Ellipsis)
        actions()
    }
}

// ---- lista ----

@Composable
private fun Home(model: MainViewModel) {
    Column(Modifier.fillMaxSize()) {
        Header("# c-hub") {
            IconButton(I.REFRESH) { model.connect() }
            IconButton(I.GEAR) { model.screen = Screen.Settings }
        }
        if (!model.hasKey) {
            Column(Modifier.padding(16.dp)) {
                Text("Importe uma chave SSH (gerada no Termux, por exemplo) para começar.", style = body)
                Spacer(Modifier.size(12.dp))
                TextButton("${I.KEY} importar chave", color = C.accent) { model.screen = Screen.Settings }
            }
            return@Column
        }
        LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 80.dp)) {
            for (host in model.hosts) {
                item(key = "h-${host.id}") { HostHeader(model, host) }
                val list = model.chats[host.id].orEmpty()
                if (model.conn[host.id] == Conn.On && list.isEmpty()) {
                    item(key = "v-${host.id}") { Text("  nenhuma sessão aberta", style = small, modifier = Modifier.padding(start = 16.dp, bottom = 8.dp)) }
                }
                items(list, key = { "c-${host.id}-${it.name}" }) { chat -> ChatRow(model, host, chat) }
            }
        }
        Row(Modifier.fillMaxWidth().padding(12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton("${I.PLUS} nova sessão", Modifier.weight(1f), color = C.accent) { model.screen = Screen.NewChat(null) }
        }
    }
}

@Composable
private fun HostHeader(model: MainViewModel, host: Host) {
    val state = model.conn[host.id] ?: Conn.Off
    val count = model.chats[host.id]?.size ?: 0
    Column(Modifier.fillMaxWidth().padding(top = 12.dp)) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(if (host.omarchy) I.LAPTOP else I.SERVER, style = body.copy(color = C.accent))
            Spacer(Modifier.width(8.dp))
            Text("${host.name} [$count]", style = body.copy(fontWeight = FontWeight.Bold), modifier = Modifier.weight(1f))
            when (state) {
                Conn.On -> {
                    Text(I.HISTORY, style = body.copy(color = C.muted, fontSize = 18.sp),
                        modifier = Modifier.clickable { model.screen = Screen.Sessions(host.id); model.loadSaved(host.id) }.padding(8.dp))
                    Text(I.PLUS, style = body.copy(color = C.muted, fontSize = 18.sp),
                        modifier = Modifier.clickable { model.screen = Screen.NewChat(host.id) }.padding(8.dp))
                }
                Conn.Connecting -> Text("conectando…", style = small)
                Conn.Off, is Conn.Failed -> Text("${I.PLUG} conectar", style = small.copy(color = C.accent),
                    modifier = Modifier.clickable { model.connect(host.id) }.padding(8.dp))
            }
        }
        if (state is Conn.Failed) {
            Text("${I.WARNING} ${state.message}", style = small.copy(color = C.yellow), modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        }
        Box(Modifier.fillMaxWidth().padding(top = 6.dp).padding(horizontal = 16.dp).size(1.dp).background(C.border))
    }
}

@Composable
private fun ChatRow(model: MainViewModel, host: Host, chat: RemoteChat) {
    val status = model.shownStatus(host.id, chat)
    val (glyph, color) = statusGlyph(status)
    val open = model.terminals.containsKey("${host.id}/${chat.name}")
    Row(
        Modifier
            .fillMaxWidth()
            .clickable { model.open(host.id, chat.name) }
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(glyph, style = body.copy(color = color, fontSize = 16.sp), modifier = Modifier.width(28.dp))
        Column(Modifier.weight(1f)) {
            Text(chat.title, style = body, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val details = buildList {
                add(chat.path)
                if (status != Status.UNKNOWN) add(status.label)
                chat.model?.let { add(it.removePrefix("claude-")) }
                if (chat.contextTokens > 0) add("${chat.contextTokens / 1000}k")
                if (open) add("aberto") else if (chat.attached) add("conectado")
            }
            Text(details.joinToString(" · "), style = small, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

// ---- chat ----

@Composable
private fun ChatScreen(model: MainViewModel, screen: Screen.Chat) {
    val terminal = model.terminals["${screen.hostId}/${screen.name}"]
    val chat = model.chats[screen.hostId]?.firstOrNull { it.name == screen.name }
    val host = model.host(screen.hostId)
    var confirmKill by remember { mutableStateOf(false) }
    // Saiu da tela do chat: desconecta (o desktop volta à largura dele; o tmux continua).
    DisposableEffect(screen) { onDispose { model.release(screen.hostId, screen.name) } }
    // Voltou ao app com o chat na tela (o terminal foi solto em segundo plano): reconecta.
    val resumed = LocalLifecycleOwner.current.lifecycle.currentStateAsState().value.isAtLeast(Lifecycle.State.RESUMED)
    val online = model.conn[screen.hostId] == Conn.On
    LaunchedEffect(resumed, online, terminal == null) {
        if (resumed && online && terminal == null) model.open(screen.hostId, screen.name)
    }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().background(C.bg), verticalAlignment = Alignment.CenterVertically) {
            IconButton(I.BACK) { model.screen = Screen.Home }
            val (glyph, color) = statusGlyph(chat?.let { model.shownStatus(screen.hostId, it) } ?: Status.UNKNOWN)
            Text(glyph, style = body.copy(color = color))
            Spacer(Modifier.width(8.dp))
            Column(Modifier.weight(1f)) {
                Text(chat?.title ?: screen.name, style = body.copy(fontWeight = FontWeight.Bold), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("${host?.name}:${chat?.path ?: ""}", style = small, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            IconButton(I.STOP, color = C.muted) { confirmKill = true }
            IconButton(I.CLOSE, color = C.muted) { model.detach(screen.hostId, screen.name) }
        }
        if (terminal == null) {
            Text(if (online) "conectando…" else "${host?.name} desconectado", style = small, modifier = Modifier.padding(16.dp))
            return@Column
        }
        Box(Modifier.weight(1f)) {
            TerminalPane(terminal, model.fontSize, Modifier.fillMaxSize())
            if (terminal.ended) {
                Row(
                    Modifier.align(Alignment.TopCenter).padding(12.dp).background(C.bg2)
                        .border(BorderStroke(1.dp, C.border), RectangleShape).padding(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("desconectado ", style = small)
                    TextButton("reconectar", color = C.accent) { model.reattach(screen.hostId, screen.name) }
                }
            }
        }
    }
    if (confirmKill) {
        Dialog(onDismissRequest = { confirmKill = false }) {
            Column(Modifier.background(C.bg2).border(BorderStroke(1.dp, C.red), RectangleShape).padding(16.dp)) {
                Text("Encerrar o agente?", style = body.copy(fontWeight = FontWeight.Bold))
                Spacer(Modifier.size(8.dp))
                Text("Mata o tmux ${screen.name} em ${host?.name}. A conversa fica salva e pode ser retomada.", style = small)
                Spacer(Modifier.size(12.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton("encerrar", color = C.red) { confirmKill = false; model.kill(screen.hostId, screen.name) }
                    TextButton("cancelar") { confirmKill = false }
                }
            }
        }
    }
}

// ---- nova sessão ----

@Composable
private fun Field(value: String, onChange: (String) -> Unit, hint: String, password: Boolean = false, singleLine: Boolean = true, minLines: Int = 1) {
    Box(Modifier.fillMaxWidth().border(BorderStroke(1.dp, C.border), RectangleShape).padding(10.dp)) {
        if (value.isEmpty()) Text(hint, style = body.copy(color = C.muted))
        BasicTextField(
            value, onChange,
            modifier = Modifier.fillMaxWidth(),
            textStyle = body,
            cursorBrush = SolidColor(C.accent),
            singleLine = singleLine,
            minLines = minLines,
            visualTransformation = if (password) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
            keyboardOptions = KeyboardOptions(keyboardType = if (password) KeyboardType.Password else KeyboardType.Ascii, autoCorrectEnabled = false),
        )
    }
}

@Composable
private fun Choice(label: String, selected: Boolean, onClick: () -> Unit) {
    Box(
        Modifier
            .border(BorderStroke(1.dp, if (selected) C.fg.copy(alpha = 0.25f) else C.border), RectangleShape)
            .background(if (selected) C.fg.copy(alpha = 0.08f) else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) { Text(if (selected) "> $label" else label, style = body.copy(color = if (selected) C.accent else C.fg)) }
}

@Composable
private fun Label(text: String) = Text(text, style = small, modifier = Modifier.padding(top = 14.dp, bottom = 6.dp))

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun NewChatScreen(model: MainViewModel, initialHost: String?) {
    val connected = model.hosts.filter { model.conn[it.id] == Conn.On }
    var hostId by remember { mutableStateOf(initialHost ?: connected.firstOrNull()?.id ?: model.hosts.first().id) }
    val host = model.host(hostId) ?: model.hosts.first()
    var agent by remember { mutableStateOf(Agent.CLAUDE) }
    var auto by remember { mutableStateOf(false) }
    var dir by remember { mutableStateOf("") }
    var modelName by remember { mutableStateOf("") }
    var prompt by remember { mutableStateOf("") }
    Column(Modifier.fillMaxSize()) {
        Header("# nova sessão", onBack = { model.screen = Screen.Home })
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            Label("onde")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                model.hosts.forEach { h ->
                    val on = model.conn[h.id] == Conn.On
                    Choice("${if (h.omarchy) I.LAPTOP else I.SERVER} ${h.name}${if (on) "" else " (off)"}", h.id == hostId) {
                        hostId = h.id
                        if (!h.omarchy) auto = false
                    }
                }
            }
            Label("agente")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Choice("claude", agent == Agent.CLAUDE && !auto) { agent = Agent.CLAUDE; auto = false }
                Choice("codex", agent == Agent.CODEX && !auto) { agent = Agent.CODEX; auto = false }
                if (host.omarchy) Choice("omarchy (claude)", auto) { agent = Agent.CLAUDE; auto = true }
            }
            Label("pasta, atalho ou atalho/sub (vazio = ${host.defaultDir})")
            Field(dir, { dir = it }, host.defaultDir)
            DirShortcuts(model, host, dir) { dir = it }
            Label("modelo (opcional)")
            Field(modelName, { modelName = it }, "padrão da CLI")
            Label("primeira mensagem (opcional)")
            Field(prompt, { prompt = it }, "", singleLine = false, minLines = 3)
            if (auto) Text("modo do omarchy agent: aprova sozinho (--permission-mode auto)", style = small, modifier = Modifier.padding(top = 8.dp))
        }
        Row(Modifier.fillMaxWidth().padding(12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton("criar", Modifier.weight(1f), color = C.accent, enabled = model.conn[hostId] == Conn.On) {
                val resolved = Shortcuts.resolve(dir, model.shortcuts[hostId].orEmpty(), host.defaultDir)
                model.create(hostId, NewChat(agent, resolved, modelName.trim().ifEmpty { null }, prompt.ifBlank { null }, auto))
            }
            TextButton("cancelar") { model.screen = Screen.Home }
        }
    }
}

/**
 * Atalhos de pasta do host (os mesmos do desktop no notebook): a pasta que o campo vai usar, os
 * atalhos para tocar (põe o nome no campo), "salvar atalho" e a lista para editar e excluir.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun DirShortcuts(model: MainViewModel, host: Host, dir: String, onPick: (String) -> Unit) {
    LaunchedEffect(host.id, model.conn[host.id]) { model.loadShortcuts(host.id) }
    val list = model.shortcuts[host.id].orEmpty()
    var saving by remember { mutableStateOf(false) }
    var managing by remember { mutableStateOf(false) }
    val resolved = Shortcuts.resolve(dir, list, host.defaultDir)
    val home = model.homes[host.id].orEmpty()
    Text("${I.ARROW} ${Parse.tilde(resolved, home)}", style = small, modifier = Modifier.padding(top = 6.dp))
    if (list.isNotEmpty()) {
        FlowRow(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            list.forEach { shortcut ->
                Text(shortcut.name, style = small.copy(color = if (dir.trim() == shortcut.name) C.accent else C.fg),
                    modifier = Modifier.border(BorderStroke(1.dp, C.border), RectangleShape).clickable { onPick(shortcut.name) }
                        .padding(horizontal = 10.dp, vertical = 6.dp))
            }
        }
    }
    Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        TextButton("salvar atalho", enabled = model.conn[host.id] == Conn.On) { saving = true }
        TextButton("atalhos", enabled = model.conn[host.id] == Conn.On) { managing = true }
    }
    if (saving) {
        val suggested = resolved.trimEnd('/').substringAfterLast('/')
        ShortcutDialog(model, host, old = null, initialName = suggested, initialPath = Parse.tilde(resolved, home),
            title = "# salvar atalho", onDone = { saving = false })
    }
    if (managing) {
        var editing by remember { mutableStateOf<Shortcut?>(null) }
        var confirm by remember { mutableStateOf<String?>(null) }
        Dialog(onDismissRequest = { managing = false }) {
            Column(Modifier.background(C.bg2).border(BorderStroke(1.dp, C.border), RectangleShape).padding(16.dp)) {
                Text("# atalhos em ${host.name}", style = body.copy(fontWeight = FontWeight.Bold))
                Spacer(Modifier.size(8.dp))
                if (list.isEmpty()) Text("nenhum atalho", style = small)
                LazyColumn(Modifier.weight(1f, fill = false)) {
                    items(list, key = { (if (it.fshort) "f-" else "c-") + it.name }) { shortcut ->
                        Row(Modifier.fillMaxWidth().clickable { onPick(shortcut.name); managing = false }.padding(vertical = 8.dp),
                            verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(shortcut.name, style = body)
                                Text(Parse.tilde(shortcut.path, home) + if (shortcut.fshort) " · fshort" else "", style = small,
                                    maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                            run {
                                if (confirm == shortcut.name) {
                                    Text("excluir?", style = small.copy(color = C.red), modifier = Modifier.clickable {
                                        model.saveShortcut(host.id, shortcut.name, shortcut.name, null) { error -> error?.let { model.notice = it } }
                                        confirm = null
                                    }.padding(8.dp))
                                } else {
                                    Text(I.EDIT, style = body.copy(color = C.muted), modifier = Modifier.clickable { editing = shortcut }.padding(8.dp))
                                    Text(I.CLOSE, style = body.copy(color = C.muted), modifier = Modifier.clickable { confirm = shortcut.name }.padding(8.dp))
                                }
                            }
                        }
                    }
                }
                Spacer(Modifier.size(8.dp))
                Text("Os marcados \"fshort\" ficam no ~/.shortcuts: editar ou excluir muda também o go do terminal.", style = small)
                Spacer(Modifier.size(12.dp))
                TextButton("fechar") { managing = false }
            }
        }
        editing?.let { shortcut ->
            ShortcutDialog(model, host, old = shortcut.name, initialName = shortcut.name, initialPath = Parse.tilde(shortcut.path, home),
                title = "# editar atalho", onDone = { editing = null })
        }
    }
}

@Composable
private fun ShortcutDialog(model: MainViewModel, host: Host, old: String?, initialName: String, initialPath: String, title: String, onDone: () -> Unit) {
    var name by remember { mutableStateOf(initialName) }
    var path by remember { mutableStateOf(initialPath) }
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    val existing = model.shortcuts[host.id].orEmpty()
    // Avisos antes de gravar, como no desktop: nome já usado, pasta que já tem atalho.
    val clean = name.trim()
    val warning = when {
        old == null && existing.any { it.name == clean && !it.fshort } -> "já existe \"$clean\": salvar troca a pasta dele"
        old == null -> existing.firstOrNull { !it.fshort && Parse.tilde(it.path, model.homes[host.id].orEmpty()) == path.trim() }?.let { "essa pasta já tem o atalho \"${it.name}\"" }
        else -> null
    }
    Dialog(onDismissRequest = onDone) {
        Column(Modifier.background(C.bg2).border(BorderStroke(1.dp, C.border), RectangleShape).padding(16.dp)) {
            Text(title, style = body.copy(fontWeight = FontWeight.Bold))
            Label("nome"); Field(name, { name = it; error = null }, "proj")
            Label("pasta em ${host.name} (/… ou ~/…)"); Field(path, { path = it; error = null }, "~/projects/x")
            (error ?: warning)?.let { Text(it, style = small.copy(color = if (error != null) C.red else C.yellow), modifier = Modifier.padding(top = 8.dp)) }
            Spacer(Modifier.size(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(if (busy) "salvando…" else "salvar", color = C.accent, enabled = !busy) {
                    val check = Shortcuts.checkName(name)
                    if (check.isFailure) { error = check.exceptionOrNull()?.message; return@TextButton }
                    busy = true
                    model.saveShortcut(host.id, old, name, path) { problem ->
                        busy = false
                        if (problem == null) onDone() else error = problem
                    }
                }
                TextButton("cancelar", onClick = onDone)
            }
        }
    }
}

// ---- sessões salvas ----

@Composable
private fun SessionsScreen(model: MainViewModel, hostId: String) {
    val host = model.host(hostId) ?: return
    val format = remember { SimpleDateFormat("dd/MM HH:mm", Locale("pt", "BR")) }
    Column(Modifier.fillMaxSize()) {
        Header("# sessões em ${host.name}", onBack = { model.screen = Screen.Home }) {
            IconButton(I.REFRESH) { model.loadSaved(hostId) }
        }
        when (val state = model.saved[hostId]) {
            null, Saved.Loading -> Text("carregando…", style = small, modifier = Modifier.padding(16.dp))
            is Saved.Failed -> Text("${I.WARNING} ${state.message}", style = small.copy(color = C.yellow), modifier = Modifier.padding(16.dp))
            is Saved.Loaded -> {
                val running = model.chats[hostId].orEmpty().mapNotNull { chat -> chat.sessionId?.let { it to chat.name } }.toMap()
                if (state.list.isEmpty()) Text("nenhuma sessão salva", style = small, modifier = Modifier.padding(16.dp))
                LazyColumn(Modifier.fillMaxSize()) {
                    items(state.list, key = { it.id }) { saved ->
                        val open = running[saved.id]
                        Row(
                            Modifier.fillMaxWidth().clickable {
                                if (open != null) model.open(hostId, open)
                                else model.create(hostId, NewChat(Agent.CLAUDE, saved.cwd, resumeId = saved.id))
                            }.padding(horizontal = 16.dp, vertical = 10.dp),
                        ) {
                            Text(if (open != null) I.READY else I.HISTORY, style = body.copy(color = if (open != null) C.accent else C.muted), modifier = Modifier.width(28.dp))
                            Column(Modifier.weight(1f)) {
                                Text(saved.title, style = body, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                val when_ = format.format(Date(saved.modified * 1000))
                                Text("${saved.cwd} · $when_${if (open != null) " · aberta" else ""}", style = small, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---- ajustes ----

@Composable
private fun SettingsScreen(model: MainViewModel) {
    val context = LocalContext.current
    var keyText by remember { mutableStateOf("") }
    var passphrase by remember { mutableStateOf("") }
    var editing by remember { mutableStateOf<Host?>(null) }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        uri ?: return@rememberLauncherForActivityResult
        keyText = context.contentResolver.openInputStream(uri)?.use { it.readBytes().toString(Charsets.UTF_8) }.orEmpty()
    }
    Column(Modifier.fillMaxSize()) {
        Header("# ajustes", onBack = { model.screen = Screen.Home })
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            Label("chave ssh")
            if (model.hasKey) {
                Text("${I.KEY} guardada no Keystore (pede a biometria a cada conexão)", style = body)
                model.vault.publicKey()?.let { public ->
                    Spacer(Modifier.size(8.dp))
                    Text(public, style = small.copy(fontSize = 10.sp), maxLines = 3, overflow = TextOverflow.Ellipsis)
                    Spacer(Modifier.size(8.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        TextButton("copiar pública") {
                            context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("chave", public))
                            model.notice = "chave pública copiada"
                        }
                        TextButton("apagar", color = C.red) { model.deleteKey() }
                    }
                }
            } else {
                Text("No Termux: cat ~/.ssh/<chave> e copie, ou cp ~/.ssh/<chave> ~/storage/downloads/ e escolha o arquivo. Depois apague a cópia.", style = small)
                Spacer(Modifier.size(8.dp))
                TextButton("escolher arquivo…") { picker.launch(arrayOf("*/*")) }
                Spacer(Modifier.size(8.dp))
                Field(keyText, { keyText = it }, "-----BEGIN OPENSSH PRIVATE KEY-----", singleLine = false, minLines = 4)
                Spacer(Modifier.size(8.dp))
                Field(passphrase, { passphrase = it }, "senha da chave (se tiver)", password = true)
                Spacer(Modifier.size(8.dp))
                TextButton("importar", color = C.accent, enabled = keyText.contains("PRIVATE KEY")) {
                    model.importKey(keyText, passphrase)
                    keyText = ""
                    passphrase = ""
                }
            }

            ThemeSection(model)

            Label("fonte do terminal")
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton("−") { model.setFont(model.fontSize - 1) }
                Text("${12 + model.fontSize} sp", style = body)
                TextButton("+") { model.setFont(model.fontSize + 1) }
            }

            Label("hosts")
            model.hosts.forEach { host ->
                Row(Modifier.fillMaxWidth().clickable { editing = host }.padding(vertical = 8.dp)) {
                    Text(if (host.omarchy) I.LAPTOP else I.SERVER, style = body.copy(color = C.accent), modifier = Modifier.width(28.dp))
                    Column {
                        Text(host.name, style = body)
                        Text("${host.user}@${host.address}:${host.port}${host.tmuxSocket?.let { " · tmux -L $it" } ?: ""}", style = small)
                    }
                }
            }
            Spacer(Modifier.size(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton("${I.PLUS} PC com C-hub") { editing = Host.blank(desktop = true) }
                TextButton("${I.PLUS} servidor") { editing = Host.blank(desktop = false) }
            }
            Spacer(Modifier.size(24.dp))
        }
    }
    editing?.let { host -> HostDialog(host, onDismiss = { editing = null }, onDelete = {
        model.saveHosts(model.hosts.filter { it.id != host.id }); editing = null
    }) { saved ->
        val list = model.hosts.toMutableList()
        val index = list.indexOfFirst { it.id == saved.id }
        if (index >= 0) list[index] = saved else list += saved
        model.saveHosts(list)
        editing = null
    } }
}

/**
 * Tema: os mesmos do desktop, "usar o do desktop" (lê o `config.json` do notebook) e as cores
 * editáveis. Mudar uma cor cria o personalizado, como no painel "tema" do desktop.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun ThemeSection(model: MainViewModel) {
    var editing by remember { mutableStateOf(false) }
    Label("tema")
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Palettes.BUILTINS.forEach { theme -> Choice(theme.name, model.themeId == theme.id) { model.setTheme(theme.id) } }
        if (model.customTheme != null) Choice("personalizado", model.themeId == Palettes.CUSTOM) { model.setTheme(Palettes.CUSTOM) }
    }
    Spacer(Modifier.size(8.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        TextButton("usar o do desktop") { model.importDesktopTheme() }
        TextButton(if (editing) "fechar cores" else "personalizar…") { editing = !editing }
    }
    if (!editing) return
    Spacer(Modifier.size(8.dp))
    Text("Mudar uma cor cria o tema personalizado a partir do tema em uso.", style = small)
    ColorField.INTERFACE.forEach { (field, name, where) -> ColorRow(model, field, name, where) }
    Label("cores do terminal")
    for (i in 0 until 16) {
        val name = ColorField.ANSI_NAMES[i % 8] + if (i >= 8) " claro" else ""
        ColorRow(model, ColorField.Ansi(i), name, null)
    }
}

@Composable
private fun ColorRow(model: MainViewModel, field: ColorField, name: String, where: String?) {
    val current = field.get(C.theme)
    var text by remember(current) { mutableStateOf(Palettes.hex(current)) }
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(28.dp).background(Color(0xff000000.toInt() or current)).border(BorderStroke(1.dp, C.border), RectangleShape))
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Text(name, style = body)
            where?.let { Text(it, style = small, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        }
        Box(Modifier.width(110.dp)) {
            Field(text, { value ->
                text = value
                // Aplica assim que vira uma cor válida.
                Palettes.parseHex(value)?.takeIf { it != current }?.let { model.setColor(field, it) }
            }, "#rrggbb")
        }
    }
}

@Composable
private fun HostDialog(host: Host, onDismiss: () -> Unit, onDelete: () -> Unit, onSave: (Host) -> Unit) {
    var name by remember { mutableStateOf(host.name) }
    var address by remember { mutableStateOf(host.address) }
    var port by remember { mutableStateOf(host.port.toString()) }
    var user by remember { mutableStateOf(host.user) }
    var socket by remember { mutableStateOf(host.tmuxSocket ?: "") }
    var dir by remember { mutableStateOf(host.defaultDir) }
    var omarchy by remember { mutableStateOf(host.omarchy) }
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.background(C.bg2).border(BorderStroke(1.dp, C.border), RectangleShape).padding(16.dp).verticalScroll(rememberScrollState())) {
            Text("# host", style = body.copy(fontWeight = FontWeight.Bold))
            Label("nome"); Field(name, { name = it }, "notebook")
            Label("endereço (IP ou nome; o do Tailscale, de preferência)"); Field(address, { address = it }, "100.x.y.z")
            Label("porta"); Field(port, { port = it.filter(Char::isDigit) }, "22")
            Label("usuário"); Field(user, { user = it }, "usuário do ssh")
            Label("servidor do tmux (-L; chube = chats do C-hub desktop; vazio = padrão)"); Field(socket, { socket = it }, "")
            Label("pasta padrão"); Field(dir, { dir = it }, "~")
            Spacer(Modifier.size(8.dp))
            Choice("tem o agente do Omarchy", omarchy) { omarchy = !omarchy }
            Spacer(Modifier.size(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton("salvar", color = C.accent, enabled = name.isNotBlank() && address.isNotBlank() && user.isNotBlank()) {
                    onSave(Host(host.id, name.trim(), address.trim(), port.toIntOrNull() ?: 22, user.trim(),
                        socket.trim().ifEmpty { null }, dir.trim().ifEmpty { "~" }, omarchy))
                }
                TextButton("apagar", color = C.red, onClick = onDelete)
            }
        }
    }
}
