package com.pedro.chub.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.pedro.chub.R
import com.pedro.chub.ssh.Status

/**
 * Cores da interface, do tema escolhido (`Palettes`). Lidas de um estado do Compose: trocar o tema
 * redesenha tudo. Os nomes são os de antes (Tokyo Night fixo), para as telas não mudarem.
 */
object C {
    var theme by mutableStateOf(Palettes.default)

    private fun c(rgb: Int) = Color(0xff000000.toInt() or rgb)
    val bg get() = c(theme.background)
    val bg2 get() = c(theme.lighterBackground)
    val fg get() = c(theme.foreground)
    val muted get() = c(theme.muted)
    val accent get() = c(theme.accent)
    val selection get() = c(theme.selection)
    val red get() = c(theme.ansi[1])
    val green get() = c(Palettes.success(theme))
    val yellow get() = c(theme.ansi[3])
    val cyan get() = c(theme.ansi[6])
    val border get() = muted.copy(alpha = 0.35f)
}

val Mono = FontFamily(
    Font(R.font.jetbrains_mono_nerd, FontWeight.Normal),
    Font(R.font.jetbrains_mono_nerd_bold, FontWeight.Bold),
)

// Funções, não valores: a cor vem do tema da hora.
val body: TextStyle get() = TextStyle(fontFamily = Mono, fontSize = 14.sp, color = C.fg, lineHeight = 19.sp)
val small: TextStyle get() = body.copy(fontSize = 12.sp, color = C.muted, lineHeight = 16.sp)

/** Ícones Nerd Font, os mesmos do desktop. */
object I {
    const val WORKING = ""
    const val WAITING = ""
    const val READY = ""
    const val DONE = ""
    const val TMUX = ""
    const val LAPTOP = "󰌢"
    const val SERVER = "󰒋"
    const val PLUS = ""
    const val GEAR = ""
    const val HISTORY = "󰋚"
    const val REFRESH = "󰑐"
    const val BACK = ""
    const val CLOSE = "󰅖"
    const val STOP = ""
    const val WARNING = ""
    const val KEY = ""
    const val PLUG = ""
    const val KEYBOARD = ""
    const val ARROW = ""
    const val EDIT = ""
}

fun statusGlyph(status: Status): Pair<String, Color> = when (status) {
    Status.WORKING -> I.WORKING to C.accent
    Status.WAITING -> I.WAITING to C.yellow
    Status.DONE -> I.DONE to C.green
    Status.READY -> I.READY to C.muted
    Status.UNKNOWN -> I.TMUX to C.muted
}

/** Botão de texto do visual "terminal": `[ rótulo ]` com borda fina e cantos retos. */
@Composable
fun TextButton(label: String, modifier: Modifier = Modifier, color: Color = C.fg, enabled: Boolean = true, onClick: () -> Unit) {
    Box(
        modifier
            .border(BorderStroke(1.dp, (if (enabled) color else C.muted).copy(alpha = 0.4f)), RectangleShape)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Text(label, style = body.copy(color = if (enabled) color else C.muted))
    }
}

/** Botão só de ícone, com área de toque de 44 dp. */
@Composable
fun IconButton(icon: String, modifier: Modifier = Modifier, color: Color = C.fg, onClick: () -> Unit) {
    Box(modifier.clickable(onClick = onClick).padding(12.dp)) {
        Text(icon, style = body.copy(fontSize = 18.sp, color = color))
    }
}
