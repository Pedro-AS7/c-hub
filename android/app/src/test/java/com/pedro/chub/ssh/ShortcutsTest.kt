package com.pedro.chub.ssh

import com.pedro.chub.ui.ColorField
import com.pedro.chub.ui.Palettes
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ShortcutsTest {
    private val config = """{"theme":"personalizado","shortcuts":[{"name":"x","path":"/home/l/x"},{"name":"a b","path":"/ruim"},{"name":"chube","path":"/home/l/projects/chube"}]}"""

    @Test
    fun lerOsDoDesktopEOsDoFshort() {
        val own = Shortcuts.own(config)
        assertEquals(listOf("chube", "x"), own.map { it.name })
        val fshort = Shortcuts.fshort("x=/outra\nw=/home/l/Work\nlixo\n")
        val all = Shortcuts.merge(own, fshort)
        // O do C-hub passa na frente do fshort de mesmo nome.
        assertEquals(listOf("chube", "x", "w"), all.map { it.name })
        assertEquals("/home/l/x", all.first { it.name == "x" }.path)
        assertTrue(all.first { it.name == "w" }.fshort)
        assertEquals(emptyList<Shortcut>(), Shortcuts.own("não é json"))
    }

    @Test
    fun gravarMantemAsOutrasChaves() {
        val list = Shortcuts.upsert(Shortcuts.own(config), null, "novo", "/home/l/novo").getOrThrow()
        val json = JSONObject(Shortcuts.withShortcuts(config, list))
        assertEquals("personalizado", json.getString("theme"))
        assertEquals(3, json.getJSONArray("shortcuts").length())
        assertEquals("[]", JSONObject(Shortcuts.withShortcuts("", emptyList())).getJSONArray("shortcuts").toString())
    }

    @Test
    fun regrasDoNome() {
        assertTrue(Shortcuts.checkName(" ok ").isSuccess)
        listOf("", "a/b", "a b", ".", "..", "~x", "x".repeat(33)).forEach { assertFalse(it, Shortcuts.checkName(it).isSuccess) }
        val list = listOf(Shortcut("a", "/a"), Shortcut("b", "/b"))
        assertFalse(Shortcuts.upsert(list, "a", "b", "/x").isSuccess)
        assertEquals(listOf("b", "c"), Shortcuts.upsert(list, "a", "c", "/a").getOrThrow().map { it.name })
        // Sem `old`, um nome existente troca a pasta.
        assertEquals("/novo", Shortcuts.upsert(list, null, "a", "/novo").getOrThrow().first { it.name == "a" }.path)
    }

    @Test
    fun pastaDoCampo() {
        val list = listOf(Shortcut("chube", "/home/l/projects/chube"))
        assertEquals("~/Work", Shortcuts.resolve("", list, "~/Work"))
        assertEquals("/home/l/projects/chube", Shortcuts.resolve("chube", list, "~"))
        assertEquals("/home/l/projects/chube/src", Shortcuts.resolve("chube/src", list, "~"))
        assertEquals("~/Work/outro", Shortcuts.resolve("outro", list, "~/Work"))
        assertEquals("~/x", Shortcuts.resolve("~/x", list, "~/Work"))
        assertEquals("/srv", Shortcuts.resolve("/srv", list, "~/Work"))
        assertEquals("/home/l/a", Shortcuts.expand("~/a/", "/home/l").getOrThrow())
        assertFalse(Shortcuts.expand("relativa", "/home/l").isSuccess)
    }

    @Test
    fun editarEExcluirNoFshort() {
        val text = "mvp=/home/u/dev\nmain=/home/u\nchb=/home/u/chube\n"
        assertEquals("mvp=/home/u/dev\ncasa=/home/u\nchb=/home/u/chube\n", Shortcuts.editFshort(text, "main", "casa", "/home/u").getOrThrow())
        assertFalse(Shortcuts.editFshort(text, "main", "chb", "/x").isSuccess)
        assertFalse(Shortcuts.editFshort(text, "sumiu", "x", "/x").isSuccess)
        assertFalse(Shortcuts.editFshort(text, "main", "ção", "/x").isSuccess)
        assertEquals("mvp=/home/u/dev\nchb=/home/u/chube\n", Shortcuts.removeFshort(text, "main"))
        assertEquals("mainx=/b\n", Shortcuts.removeFshort("main=/a\nmainx=/b\n", "main"))
        assertEquals("", Shortcuts.removeFshort("a=/a\n", "a"))
    }

    @Test
    fun temas() {
        // Mesmo JSON do custom_theme do desktop, ida e volta.
        val custom = ColorField.Accent.set(Palettes.default, 0x28b0ff)
        assertEquals(custom, Palettes.fromJson(Palettes.toJson(custom)))
        // Fundo claro vira tema claro.
        assertTrue(ColorField.Background.set(Palettes.default, 0xffffff).light)
        val omarchy = Palettes.parseOmarchy("mode = \"dark\"\naccent = \"#7aa2f7\"\nbackground = \"#1c1c1e\"\ngreen = \"#ff9f0a\"\nyellow = \"#ffc27a\"\nred = \"#ff5f4a\"\ncyan = \"#a6dcef\"\nmuted = \"#8e8e93\"")
        assertEquals(0x1c1c1e, omarchy.ansi[0])
        assertEquals(0xff9f0a, omarchy.ansi[2])
        assertEquals(0xff9f0a, omarchy.ansi[10])
        // No Starship o verde é laranja: o "terminou" usa outra cor.
        assertEquals(0xa6dcef, Palettes.success(omarchy))
        assertEquals(0x9ece6a, Palettes.success(Palettes.default))
        assertEquals(6, Palettes.BUILTINS.map { it.id }.toSet().size)
    }
}
