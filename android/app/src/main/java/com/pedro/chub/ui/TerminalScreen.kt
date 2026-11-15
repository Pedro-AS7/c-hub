package com.pedro.chub.ui

import android.content.Context
import android.graphics.Typeface
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.inputmethod.InputMethodManager
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.res.ResourcesCompat
import com.termux.terminal.TerminalSession
import com.termux.terminal.TextStyle
import com.termux.view.TerminalView
import com.termux.view.TerminalViewClient
import com.pedro.chub.ChatTerminal
import com.pedro.chub.R

/** Estado dos modificadores da barra de teclas (valem para a próxima tecla). */
class Modifiers {
    var ctrl by mutableStateOf(false)
    var alt by mutableStateOf(false)
}

/** Terminal do chat e a barra de teclas que o teclado do celular não tem. */
@Composable
fun TerminalPane(terminal: ChatTerminal, fontSize: Int, modifier: Modifier = Modifier) {
    val mods = remember { Modifiers() }
    val holder = remember { arrayOfNulls<TerminalView>(1) }
    Column(modifier) {
        AndroidView(
            modifier = Modifier.fillMaxWidth().weight(1f),
            factory = { context -> createView(context, terminal, mods, fontSize).also { holder[0] = it } },
            update = { view ->
                holder[0] = view
                if (view.currentSession !== terminal.session) {
                    terminal.view = view
                    view.attachSession(terminal.session)
                }
                // Só quando muda: cada setTextSize recria o renderizador e redesenha tudo.
                val size = textSizePx(view.context, fontSize)
                if (view.tag != size) {
                    view.tag = size
                    view.setTextSize(size)
                }
            },
        )
        KeyBar(terminal, mods) { holder[0] }
    }
    // Trocou o tema com o terminal aberto: recolore o que já está na tela.
    val theme = C.theme
    androidx.compose.runtime.LaunchedEffect(theme) {
        applyColors(terminal.session)
        holder[0]?.setBackgroundColor(0xff000000.toInt() or theme.background)
        holder[0]?.invalidate()
    }
    DisposableEffect(terminal) {
        onDispose { if (terminal.view === holder[0]) terminal.view = null }
    }
}

private fun textSizePx(context: Context, fontSize: Int): Int {
    // fontSize: deslocamento em sp sobre 12 (o padrão cabe ~50 colunas no S23+).
    val sp = (12 + fontSize).coerceIn(7, 24)
    return (sp * context.resources.displayMetrics.scaledDensity).toInt()
}

/** Põe as cores do tema no emulador (o esquema é global: vale para todo terminal). */
fun applyColors(session: TerminalSession?) {
    val theme = C.theme
    val defaults = com.termux.terminal.TerminalColors.COLOR_SCHEME.mDefaultColors
    for (i in 0 until 16) defaults[i] = 0xff000000.toInt() or theme.ansi[i]
    defaults[TextStyle.COLOR_INDEX_FOREGROUND] = 0xff000000.toInt() or theme.foreground
    defaults[TextStyle.COLOR_INDEX_BACKGROUND] = 0xff000000.toInt() or theme.background
    defaults[TextStyle.COLOR_INDEX_CURSOR] = 0xff000000.toInt() or theme.foreground
    session?.emulator?.mColors?.reset()
}

private fun createView(context: Context, terminal: ChatTerminal, mods: Modifiers, fontSize: Int): TerminalView {
    val view = TerminalView(context, null)
    // O tamanho antes da fonte: o setTextSize cria o renderizador, que o setTypeface usa.
    view.setTextSize(textSizePx(context, fontSize))
    view.setTypeface(ResourcesCompat.getFont(context, R.font.jetbrains_mono_nerd) ?: Typeface.MONOSPACE)
    view.tag = textSizePx(context, fontSize)
    view.setBackgroundColor(0xff000000.toInt() or C.theme.background)
    view.isFocusable = true
    view.isFocusableInTouchMode = true
    view.keepScreenOn = true
    view.setTerminalViewClient(object : TerminalViewClient {
        override fun onScale(scale: Float): Float = 1f
        override fun onSingleTapUp(e: MotionEvent) {
            view.requestFocus()
            context.getSystemService(InputMethodManager::class.java).showSoftInput(view, 0)
        }
        override fun shouldBackButtonBeMappedToEscape() = false
        override fun shouldEnforceCharBasedInput() = true
        override fun shouldUseCtrlSpaceWorkaround() = false
        override fun isTerminalViewSelected() = true
        override fun copyModeChanged(copyMode: Boolean) {}
        override fun onKeyDown(keyCode: Int, e: KeyEvent, session: TerminalSession) = false
        override fun onKeyUp(keyCode: Int, e: KeyEvent) = false
        override fun onLongPress(event: MotionEvent) = false
        override fun readControlKey(): Boolean = mods.ctrl.also { if (it) mods.ctrl = false }
        override fun readAltKey(): Boolean = mods.alt.also { if (it) mods.alt = false }
        override fun readShiftKey() = false
        override fun readFnKey() = false
        override fun onCodePoint(codePoint: Int, ctrlDown: Boolean, session: TerminalSession) = false
        override fun onEmulatorSet() {
            applyColors(terminal.session)
            view.setTerminalCursorBlinkerState(false, true)
        }
        override fun logError(tag: String, message: String) {}
        override fun logWarn(tag: String, message: String) {}
        override fun logInfo(tag: String, message: String) {}
        override fun logDebug(tag: String, message: String) {}
        override fun logVerbose(tag: String, message: String) {}
        override fun logStackTraceWithMessage(tag: String, message: String, e: Exception) {}
        override fun logStackTrace(tag: String, e: Exception) {}
    })
    terminal.view = view
    view.attachSession(terminal.session)
    view.post {
        view.requestFocus()
        context.getSystemService(InputMethodManager::class.java).showSoftInput(view, 0)
    }
    return view
}

/** Teclas que faltam no teclado do celular. Ctrl e Alt valem para a próxima tecla. */
@Composable
private fun KeyBar(terminal: ChatTerminal, mods: Modifiers, view: () -> TerminalView?) {
    fun send(text: String) = terminal.write(text)
    val keys: List<Pair<String, () -> Unit>> = listOf(
        "esc" to { send("\u001b") },
        "⇧tab" to { send("\u001b[Z") },
        "tab" to { send("\t") },
        "^C" to { send("\u0003") },
        "↑" to { send("\u001b[A") },
        "↓" to { send("\u001b[B") },
        "←" to { send("\u001b[D") },
        "→" to { send("\u001b[C") },
        "/" to { send("/") },
        "⏎" to { send("\r") },
        "colar" to { terminal.onPasteTextFromClipboard(terminal.session) },
    )
    Row(
        Modifier
            .fillMaxWidth()
            .background(C.bg2)
            .horizontalScroll(rememberScrollState())
            .height(44.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Key("ctrl", active = mods.ctrl) { mods.ctrl = !mods.ctrl }
        Key("alt", active = mods.alt) { mods.alt = !mods.alt }
        keys.forEach { (label, action) -> Key(label) { action(); view()?.requestFocus() } }
    }
}

@Composable
private fun Key(label: String, active: Boolean = false, onClick: () -> Unit) {
    Box(
        Modifier
            .clickable(onClick = onClick)
            .background(if (active) C.selection else C.bg2)
            .padding(horizontal = 12.dp, vertical = 10.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, style = body.copy(fontSize = 13.sp, color = if (active) C.accent else C.fg))
    }
}

@Composable
fun Fill(content: @Composable () -> Unit) = Box(Modifier.fillMaxSize()) { content() }
