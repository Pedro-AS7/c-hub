package com.pedro.chub.ui

import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/** Cores de um tema, como no desktop (`src/omarchy.rs`, `ThemeColors`). Cores em `0xRRGGBB`. */
data class ThemeColors(
    val light: Boolean,
    val accent: Int,
    val selection: Int,
    val muted: Int,
    val background: Int,
    val lighterBackground: Int,
    val foreground: Int,
    val brightForeground: Int,
    /** As 16 cores do terminal. */
    val ansi: List<Int>,
)

data class Builtin(val id: String, val name: String, val colors: ThemeColors)

/** Uma cor que o editor de tema deixa mudar (igual ao `ColorField` do desktop). */
sealed interface ColorField {
    data object Accent : ColorField
    data object Background : ColorField
    data object Foreground : ColorField
    data object Muted : ColorField
    data object Selection : ColorField
    data class Ansi(val index: Int) : ColorField

    fun get(colors: ThemeColors): Int = when (this) {
        Accent -> colors.accent
        Background -> colors.background
        Foreground -> colors.foreground
        Muted -> colors.muted
        Selection -> colors.selection
        is Ansi -> colors.ansi[index % 16]
    }

    fun set(colors: ThemeColors, value: Int): ThemeColors {
        val v = value and 0xffffff
        return when (this) {
            Accent -> colors.copy(accent = v)
            Background -> colors.copy(background = v, light = Palettes.luminance(v) > 0.5)
            Foreground -> colors.copy(foreground = v)
            Muted -> colors.copy(muted = v)
            Selection -> colors.copy(selection = v)
            is Ansi -> colors.copy(ansi = colors.ansi.toMutableList().also { it[index % 16] = v })
        }
    }

    companion object {
        /** As cores da interface, com o nome e onde aparecem, na ordem do desktop. */
        val INTERFACE = listOf(
            Triple(Accent, "destaque", "títulos, item escolhido, \"trabalhando\""),
            Triple(Background, "fundo", "fundo das telas e dos diálogos"),
            Triple(Foreground, "texto", "texto e cursor do terminal"),
            Triple(Muted, "apagado", "detalhes, ícones e bordas"),
            Triple(Selection, "seleção", "item escolhido e teclas ligadas"),
        )
        val ANSI_NAMES = listOf("preto", "vermelho", "verde", "amarelo", "azul", "magenta", "ciano", "branco")
    }
}

/**
 * Os temas do C-hub: os mesmos do desktop (`src/palettes.rs`, ids iguais), o personalizado
 * (mesmo JSON do `custom_theme` do `config.json`) e a leitura do `colors.toml` do Omarchy.
 */
object Palettes {
    const val DEFAULT = "tokyo-night"
    const val CUSTOM = "personalizado"

    val BUILTINS = listOf(
        Builtin("escuro", "Escuro", ThemeColors(false, 0x6ca0dc, 0x3a3a3a, 0x8a8a8a, 0x1e1e1e, 0x2a2a2a, 0xd4d4d4, 0xffffff, listOf(
            0x2e2e2e, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xd4d4d4,
            0x5c5c5c, 0xef8f97, 0xb5dd96, 0xf0d197, 0x8cc5f5, 0xd9a0e8, 0x7fcfd8, 0xffffff))),
        Builtin("tokyo-night", "Tokyo Night", ThemeColors(false, 0x7aa2f7, 0x283457, 0x737aa2, 0x1a1b26, 0x24283b, 0xc0caf5, 0xc0caf5, listOf(
            0x15161e, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xa9b1d6,
            0x414868, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xc0caf5))),
        Builtin("catppuccin-mocha", "Catppuccin Mocha", ThemeColors(false, 0xcba6f7, 0x45475a, 0x7f849c, 0x1e1e2e, 0x313244, 0xcdd6f4, 0xcdd6f4, listOf(
            0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de,
            0x585b70, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xa6adc8))),
        Builtin("catppuccin-latte", "Catppuccin Latte", ThemeColors(true, 0x8839ef, 0xbcc0cc, 0x7c7f93, 0xeff1f5, 0xe6e9ef, 0x4c4f69, 0x4c4f69, listOf(
            0x5c5f77, 0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179299, 0xacb0be,
            0x6c6f85, 0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179299, 0xbcc0cc))),
        Builtin("gruvbox-dark", "Gruvbox", ThemeColors(false, 0xfabd2f, 0x504945, 0x928374, 0x282828, 0x3c3836, 0xebdbb2, 0xfbf1c7, listOf(
            0x282828, 0xcc241d, 0x98971a, 0xd79921, 0x458588, 0xb16286, 0x689d6a, 0xa89984,
            0x928374, 0xfb4934, 0xb8bb26, 0xfabd2f, 0x83a598, 0xd3869b, 0x8ec07c, 0xebdbb2))),
        Builtin("nord", "Nord", ThemeColors(false, 0x88c0d0, 0x434c5e, 0x7b88a1, 0x2e3440, 0x3b4252, 0xd8dee9, 0xeceff4, listOf(
            0x3b4252, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x88c0d0, 0xe5e9f0,
            0x4c566a, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x8fbcbb, 0xeceff4))),
    )

    fun find(id: String): Builtin? = BUILTINS.firstOrNull { it.id == id }

    val default: ThemeColors get() = find(DEFAULT)!!.colors

    fun hex(color: Int): String = "#%06x".format(color and 0xffffff)

    /** `#rrggbb` (com ou sem `#`). */
    fun parseHex(value: String): Int? {
        val hex = value.trim().removePrefix("#")
        return if (hex.length == 6) hex.toIntOrNull(16) else null
    }

    /** Tema personalizado no formato do `custom_theme` do desktop (a opacidade fica como está). */
    fun toJson(colors: ThemeColors, opacity: Double = 1.0): JSONObject = JSONObject()
        .put("accent", hex(colors.accent))
        .put("selection", hex(colors.selection))
        .put("muted", hex(colors.muted))
        .put("background", hex(colors.background))
        .put("lighter_background", hex(colors.lighterBackground))
        .put("foreground", hex(colors.foreground))
        .put("bright_foreground", hex(colors.brightForeground))
        .put("ansi", JSONArray(colors.ansi.map(::hex)))
        .put("opacity", opacity)

    /** Tema personalizado do JSON; o que faltar ou estiver estragado fica com o padrão. */
    fun fromJson(json: JSONObject): ThemeColors {
        var c = default
        fun color(key: String) = json.optString(key).let(::parseHex)
        color("accent")?.let { c = c.copy(accent = it) }
        color("selection")?.let { c = c.copy(selection = it) }
        color("muted")?.let { c = c.copy(muted = it) }
        color("background")?.let { c = c.copy(background = it) }
        color("lighter_background")?.let { c = c.copy(lighterBackground = it) }
        color("foreground")?.let { c = c.copy(foreground = it) }
        color("bright_foreground")?.let { c = c.copy(brightForeground = it) }
        json.optJSONArray("ansi")?.let { array ->
            val ansi = c.ansi.toMutableList()
            for (i in 0 until minOf(16, array.length())) parseHex(array.optString(i))?.let { ansi[i] = it }
            c = c.copy(ansi = ansi)
        }
        return c.copy(light = luminance(c.background) > 0.5)
    }

    /** `colors.toml` do Omarchy, com o mesmo mapeamento do desktop (`ThemeColors::parse`). */
    fun parseOmarchy(text: String): ThemeColors {
        val keys = text.lineSequence().mapNotNull { line ->
            val (key, value) = line.split('=', limit = 2).takeIf { it.size == 2 } ?: return@mapNotNull null
            key.trim() to value.trim().trim('"')
        }.toMap()
        fun color(key: String) = keys[key]?.let(::parseHex)
        var c = default.copy(light = keys["mode"] == "light")
        color("accent")?.let { c = c.copy(accent = it) }
        color("selection")?.let { c = c.copy(selection = it) }
        color("muted")?.let { c = c.copy(muted = it) }
        color("background")?.let { c = c.copy(background = it) }
        color("lighter_background")?.let { c = c.copy(lighterBackground = it) }
        color("foreground")?.let { c = c.copy(foreground = it) }
        color("bright_foreground")?.let { c = c.copy(brightForeground = it) }
        // Preto é o fundo, branco é o texto, preto claro é o `muted`, branco claro o `bright_foreground`.
        val ansi = c.ansi.toMutableList()
        ansi[0] = c.background; ansi[7] = c.foreground; ansi[8] = c.muted; ansi[15] = c.brightForeground
        listOf("red", "green", "yellow", "blue", "magenta", "cyan").forEachIndexed { i, name ->
            color(name)?.let { ansi[i + 1] = it }
            ansi[i + 9] = color("bright_$name") ?: ansi[i + 1]
        }
        return c.copy(ansi = ansi)
    }

    /** Luminância relativa (WCAG). */
    fun luminance(color: Int): Double {
        fun channel(c: Int): Double {
            val v = c / 255.0
            return if (v <= 0.03928) v / 12.92 else Math.pow((v + 0.055) / 1.055, 2.4)
        }
        return 0.2126 * channel(color shr 16 and 0xff) + 0.7152 * channel(color shr 8 and 0xff) + 0.0722 * channel(color and 0xff)
    }

    /**
     * Cor do "terminou": o verde do tema, a menos que fique perto do amarelo, do vermelho ou do
     * destaque (no Starship o verde é laranja): aí o verde claro, o ciano ou o azul (como no desktop).
     */
    fun success(colors: ThemeColors): Int {
        val a = colors.ansi
        fun distinct(color: Int): Boolean {
            val hue = hue(color) ?: return false
            fun far(other: Int, minimum: Double) = hue(other)?.let { distance(hue, it) >= minimum } ?: true
            return far(a[3], 45.0) && far(a[1], 45.0) && far(colors.accent, 20.0)
        }
        return listOf(a[2], a[10], a[6], a[4]).firstOrNull(::distinct) ?: a[2]
    }

    private fun distance(a: Double, b: Double): Double = abs(a - b).let { min(it, 360 - it) }

    private fun hue(color: Int): Double? {
        val r = (color shr 16 and 0xff) / 255.0
        val g = (color shr 8 and 0xff) / 255.0
        val b = (color and 0xff) / 255.0
        val mx = max(r, max(g, b))
        val mn = min(r, min(g, b))
        val delta = mx - mn
        val lightness = (mx + mn) / 2
        val saturation = if (delta == 0.0) 0.0 else delta / (1 - abs(2 * lightness - 1))
        if (saturation < 0.25) return null
        return when (mx) {
            r -> 60 * (((g - b) / delta).mod(6.0))
            g -> 60 * ((b - r) / delta + 2)
            else -> 60 * ((r - g) / delta + 4)
        }
    }
}
