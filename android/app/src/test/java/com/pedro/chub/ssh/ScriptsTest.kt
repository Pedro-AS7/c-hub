package com.pedro.chub.ssh

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ScriptsTest {
    private val notebook = Host("pc", "pc", "100.64.0.1", 22, "user", tmuxSocket = Host.DESKTOP_TMUX, defaultDir = "~/Work", omarchy = true)
    private val vps = Host("vps", "vps", "100.64.0.2", 22, "user")

    @Test
    fun aspas() {
        assertEquals("simples/caminho-1", Shell.quote("simples/caminho-1"))
        assertEquals("'com espaço'", Shell.quote("com espaço"))
        assertEquals("'it'\\''s \$HOME'", Shell.quote("it's \$HOME"))
        assertEquals("''", Shell.quote(""))
        assertEquals("\"\$HOME\"", Shell.quoteDir("~"))
        assertEquals("\"\$HOME\"'/meus projetos'", Shell.quoteDir("~/meus projetos"))
        assertEquals("/srv/app", Shell.quoteDir("/srv/app"))
    }

    @Test
    fun notebookUsaOTmuxDoDesktop() {
        assertEquals("tmux -u -L chube -f /dev/null", Scripts.tmux(notebook))
        assertEquals("tmux -u", Scripts.tmux(vps))
    }

    @Test
    fun criarSessao() {
        val name = Scripts.newName()
        assertTrue(name.matches(Regex("chube-[0-9a-f]{8}")))
        val script = Scripts.create(notebook, name, NewChat(Agent.CLAUDE, "", prompt = "oi; rm -rf /", auto = true))
        assertTrue(script, script.contains("cd \"\$HOME\"/Work 2>/dev/null || cd; exec tmux -u -L chube -f /dev/null new-session -A -s $name "))
        assertTrue(script.contains("--permission-mode auto"))
        assertTrue(script.contains("set-option -t $name prefix None"))
        assertTrue(script.contains("set-option -t $name status off"))
        // O prompt vai inteiro entre aspas, dentro das aspas do bash -lc.
        assertTrue(script.contains("oi; rm -rf /"))
        assertFalse(script.contains("-- oi; rm"))
    }

    @Test
    fun retomarECodex() {
        val resume = Scripts.agentLine(NewChat(Agent.CLAUDE, "~", resumeId = "abc", model = "haiku"))
        assertTrue(resume.endsWith("env -u ANTHROPIC_API_KEY claude --resume abc --model haiku"))
        val codex = Scripts.agentLine(NewChat(Agent.CODEX, "~"))
        assertTrue(codex.contains("env -u OPENAI_API_KEY -u CODEX_API_KEY codex -c"))
    }

    @Test
    fun entrarEEncerrar() {
        assertTrue(Scripts.attach(vps, "do celular").contains("exec tmux -u attach-session -t 'do celular' \\; "))
        assertEquals("tmux -u -L chube -f /dev/null kill-session -t chube-1", Scripts.kill(notebook, "chube-1"))
    }

    @Test
    fun estado() {
        val s = Scripts.SEP
        val lines = listOf(
            "S${s}chube-1${s}1${s}/home/l/projects/x${s}✳ Arrumar testes${s}1700000000${s}abc-1${s}idle${s}claude-opus-5-5${s}\"usage\":{\"input_tokens\":10,\"cache_read_input_tokens\":1000,\"output_tokens\":5",
            "S${s}celular${s}0${s}/home/l${s}meu-pc${s}1700000001${s}${s}${s}${s}",
            "lixo",
        )
        val list = Parse.pollBatch(lines, "/home/l")
        assertEquals(2, list.size)
        val first = list[0]
        assertEquals("chube-1", first.name)
        assertTrue(first.attached)
        assertEquals("~/projects/x", first.path)
        assertEquals("Arrumar testes", first.title)
        assertEquals(Status.READY, first.status)
        assertEquals(1010, first.contextTokens)
        assertEquals("claude-opus-5-5", first.model)
        val second = list[1]
        assertEquals(null, second.sessionId)
        assertEquals(Status.UNKNOWN, second.status)
        assertEquals("~", second.path)
        assertEquals("/home/l", Parse.home("H${s}/home/l"))
    }

    @Test
    fun statusPeloTitulo() {
        assertEquals(Status.READY, Status.fromTitle("✳ Assunto"))
        assertEquals(Status.WORKING, Status.fromTitle("⠂ Assunto"))
        assertEquals(null, Status.fromTitle("meu-pc"))
        // O arquivo do Claude Code vale mais que o título.
        assertEquals(Status.WAITING, RemoteChat("x", false, "~", "✳ a", 0, "id", "waiting", null, 0).status)
    }

    @Test
    fun sessoesSalvas() {
        val text = """
H	/home/l
F	1700000000	-home-l-x/abc.jsonl
"cwd":"/home/l/x"
"aiTitle":"Título da IA
"customTitle":"Meu título
F	1700000001	-home-l-y/def.jsonl
"cwd":"/srv/y"
"lastPrompt":"só o prompt
F	1700000002	-home-l-z/sem.jsonl
""".trimIndent()
        val list = Parse.saved(text)
        assertEquals(2, list.size)
        assertEquals(SavedSession("abc", "~/x", "Meu título", 1700000000), list[0])
        assertEquals("só o prompt", list[1].title)
        assertEquals("/srv/y", list[1].cwd)
    }
}
