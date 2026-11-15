package com.pedro.chub

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.VibrationEffect
import android.os.Vibrator
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.termux.terminal.TerminalEmulator
import com.termux.terminal.TerminalSession
import com.termux.terminal.TerminalSessionClient
import com.termux.terminal.TerminalTransport
import com.termux.view.TerminalView

/**
 * O terminal de um chat aberto: a sessão do emulador (ligada a um canal SSH com o tmux) e o que
 * a tela precisa saber dela. Fica vivo ao voltar para a lista, para reabrir na hora.
 */
class ChatTerminal(context: Context, val hostId: String, val name: String, transport: TerminalTransport) : TerminalSessionClient {
    private val app = context.applicationContext

    /** A view que mostra o terminal agora (uma só por vez). */
    var view: TerminalView? = null

    /** O canal fechou: a tela oferece reconectar. */
    var ended by mutableStateOf(false)
        private set

    val session = TerminalSession(transport, 5000, this)

    fun write(text: String) = session.write(text)

    override fun onTextChanged(changedSession: TerminalSession) {
        view?.onScreenUpdated()
    }

    override fun onTitleChanged(changedSession: TerminalSession) {}

    override fun onSessionFinished(finishedSession: TerminalSession) {
        ended = true
    }

    override fun onCopyTextToClipboard(session: TerminalSession, text: String) {
        val clipboard = app.getSystemService(ClipboardManager::class.java)
        clipboard.setPrimaryClip(ClipData.newPlainText("c-hub", text))
    }

    override fun onPasteTextFromClipboard(session: TerminalSession) {
        val clipboard = app.getSystemService(ClipboardManager::class.java)
        val text = clipboard.primaryClip?.getItemAt(0)?.coerceToText(app)?.toString() ?: return
        session.emulator?.paste(text)
    }

    override fun onBell(session: TerminalSession) {
        app.getSystemService(Vibrator::class.java)
            ?.vibrate(VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK))
    }

    override fun onColorsChanged(session: TerminalSession) {
        view?.invalidate()
    }

    override fun onTerminalCursorStateChange(state: Boolean) {}

    override fun getTerminalCursorStyle(): Int = TerminalEmulator.TERMINAL_CURSOR_STYLE_BLOCK

    override fun logError(tag: String, message: String) { Log.e(tag, message) }
    override fun logWarn(tag: String, message: String) { Log.w(tag, message) }
    override fun logInfo(tag: String, message: String) {}
    override fun logDebug(tag: String, message: String) {}
    override fun logVerbose(tag: String, message: String) {}
    override fun logStackTraceWithMessage(tag: String, message: String, e: Exception) { Log.e(tag, message, e) }
    override fun logStackTrace(tag: String, e: Exception) { Log.e(tag, "erro", e) }
}
