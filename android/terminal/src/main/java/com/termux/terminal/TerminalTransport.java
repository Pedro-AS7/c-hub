package com.termux.terminal;

/**
 * De onde vêm e para onde vão os bytes de um {@link TerminalSession}. No Termux era um processo
 * local num pty (JNI); no C-hub é um canal SSH com pty. Alteração do C-hub sobre o código do
 * Termux (Apache 2.0).
 */
public interface TerminalTransport {

    /** Recebe a saída do outro lado, numa thread qualquer. */
    interface Sink {
        /** Bytes vindos do outro lado. Retorna false se a sessão já fechou. */
        boolean onOutput(byte[] data, int offset, int count);

        /** O outro lado fechou; {@code reason} é null num fim normal. */
        void onClosed(String reason);
    }

    /** Começa com o tamanho inicial. Chamado uma vez, na thread principal: não bloqueie. */
    void start(int columns, int rows, Sink sink);

    /** Manda bytes (teclas) ao outro lado. Chamado numa thread de escrita. */
    void write(byte[] data, int offset, int count) throws java.io.IOException;

    /** Novo tamanho do terminal. */
    void resize(int columns, int rows);

    /** Encerra a conexão (o {@link Sink#onClosed} ainda é chamado). */
    void close();
}
