package com.pedro.chub.ssh

import com.termux.terminal.TerminalTransport
import net.schmizz.keepalive.KeepAliveProvider
import net.schmizz.keepalive.KeepAliveRunner
import net.schmizz.sshj.DefaultConfig
import net.schmizz.sshj.SSHClient
import net.schmizz.sshj.common.KeyType
import net.schmizz.sshj.connection.channel.direct.Session
import net.schmizz.sshj.transport.verification.HostKeyVerifier
import java.io.IOException
import java.io.InputStream
import java.security.PublicKey
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * Uma conexão SSH por host, com vários canais: o laço do estado, um terminal por chat aberto e os
 * comandos avulsos. Uma conexão = uma autenticação = uma biometria.
 */
class Connection(val host: Host) {
    @Volatile
    private var client: SSHClient? = null

    val isConnected: Boolean get() = client?.let { it.isConnected && it.isAuthenticated } == true

    /**
     * Conecta e autentica (bloqueia: chame fora da thread principal). `trust` decide se a chave do
     * host é aceita (impressão digital, tipo) e pode perguntar a você.
     */
    fun connect(secret: Secret, trust: (fingerprint: String, type: String) -> Boolean) {
        disconnect()
        val config = DefaultConfig().apply { keepAliveProvider = KeepAliveProvider.KEEP_ALIVE }
        val ssh = SSHClient(config)
        ssh.addHostKeyVerifier(object : HostKeyVerifier {
            override fun verify(hostname: String, port: Int, key: PublicKey): Boolean =
                trust(KeyVault.fingerprint(key), KeyType.fromKey(key).toString())

            override fun findExistingAlgorithms(hostname: String, port: Int): List<String> = emptyList()
        })
        ssh.connectTimeout = 10_000
        ssh.timeout = 0
        try {
            ssh.connect(host.address, host.port)
            // Rede caída fecha a conexão em ~1 min em vez de deixar tudo congelado.
            (ssh.connection.keepAlive as? KeepAliveRunner)?.maxAliveCount = 4
            ssh.connection.keepAlive.keepAliveInterval = 15
            ssh.authPublickey(host.user, secret.keyProvider())
        } catch (error: Exception) {
            runCatching { ssh.disconnect() }
            throw error
        }
        client = ssh
    }

    fun disconnect() {
        client?.let { runCatching { it.disconnect() } }
        client = null
    }

    private fun session(): Session = (client ?: throw IOException("sem conexão com ${host.name}")).startSession()

    /** Roda um script sem terminal e devolve a saída (bloqueia). `input` vai para a entrada dele. */
    fun exec(script: String, timeoutSeconds: Long = 20, input: ByteArray? = null): String {
        session().use { session ->
            val command = session.exec(script)
            // Entrada padrão do script (ex.: o arquivo a gravar); fechar = fim da entrada.
            if (input != null) {
                command.outputStream.write(input)
                command.outputStream.flush()
                command.outputStream.close()
            }
            val output = command.inputStream.readBytes().toString(Charsets.UTF_8)
            val error = command.errorStream.readBytes().toString(Charsets.UTF_8)
            command.join(timeoutSeconds, TimeUnit.SECONDS)
            val status = command.exitStatus
            if (status != null && status != 0) {
                throw IOException(error.lines().lastOrNull { it.isNotBlank() }?.trim() ?: "erro $status")
            }
            return output
        }
    }

    /** Roda um script de saída contínua (o laço do estado): devolve a saída para ler linha a linha. */
    fun stream(script: String): Pair<Session, InputStream> {
        val session = session()
        val command = session.exec(script)
        return session to command.inputStream
    }

    /** Transporte de um terminal: um canal com pty rodando `script` (o tmux do chat). */
    fun transport(script: String): TerminalTransport = PtyTransport(this, script)

    private class PtyTransport(private val connection: Connection, private val script: String) : TerminalTransport {
        private val io = Executors.newSingleThreadExecutor()
        private val ready = CountDownLatch(1)
        @Volatile private var session: Session? = null
        @Volatile private var command: Session.Command? = null

        override fun start(columns: Int, rows: Int, sink: TerminalTransport.Sink) {
            Thread({
                var reason: String? = null
                try {
                    val session = connection.session().also { this.session = it }
                    session.allocatePTY("xterm-256color", columns, rows, 0, 0, emptyMap())
                    val command = session.exec(script).also { this.command = it }
                    ready.countDown()
                    val input = command.inputStream
                    val buffer = ByteArray(8192)
                    while (true) {
                        val read = input.read(buffer)
                        if (read < 0) break
                        if (!sink.onOutput(buffer, 0, read)) break
                    }
                    command.join(2, TimeUnit.SECONDS)
                    if (!connection.isConnected) reason = "conexão caiu"
                } catch (error: Exception) {
                    reason = error.message ?: error.javaClass.simpleName
                } finally {
                    ready.countDown()
                    runCatching { session?.close() }
                    io.shutdown()
                    sink.onClosed(reason)
                }
            }, "pty-${connection.host.name}").start()
        }

        override fun write(data: ByteArray, offset: Int, count: Int) {
            ready.await(15, TimeUnit.SECONDS)
            val output = command?.outputStream ?: throw IOException("terminal fechado")
            output.write(data, offset, count)
            output.flush()
        }

        override fun resize(columns: Int, rows: Int) {
            // Vai pela rede: fora da thread principal.
            runCatching {
                io.execute {
                    runCatching {
                        (command as? net.schmizz.sshj.connection.channel.direct.SessionChannel)
                            ?.changeWindowDimensions(columns, rows, 0, 0)
                    }
                }
            }
        }

        override fun close() {
            Thread({ runCatching { command?.close() }; runCatching { session?.close() } }, "pty-close").start()
        }
    }
}
