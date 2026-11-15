# Contribuindo com o C-hub

Obrigado pelo interesse! Issues e pull requests são bem-vindos, em português ou em inglês.

## Antes de começar

- **Bug:** abra uma issue com o que aconteceu, o que esperava, a distribuição (ou o modelo do celular e a versão do
  Android) e as versões do `claude`/`codex` e do `tmux`.
- **Mudança grande** (função nova, troca de dependência, mudança de formato): abra uma issue antes para conversar sobre
  ela. Isso evita trabalho perdido.
- **Correção pequena** (texto, bug óbvio): pode mandar o PR direto.

## Estrutura

```
src/                desktop (Rust + GTK4 + VTE)
tests/              teste de integração do desktop (filtro de cores)
res/                ícone e atalhos .desktop
android/            app Android (Kotlin + Compose)
  app/              o app
  terminal/         emulador e view de terminal do Termux (com o processo local trocado por um transporte SSH)
scripts/            scripts de configuração do PC (sshd, Tailscale, chave do celular)
docs/               guia de configuração
```

O `CLAUDE.md` da raiz e o `android/CLAUDE.md` explicam a arquitetura em detalhe: como cada parte funciona e por quê. Eles
também são as instruções para agentes de IA (Claude Code, Codex) que trabalhem no projeto.

## Ambiente

**Desktop:** os requisitos da [parte 1 do guia](docs/configuracao.md#1-desktop). Depois:

```sh
cargo build
cargo test
cargo clippy --all-targets   # se tiver o clippy
cargo run                    # roda a versão de desenvolvimento
```

Para testar sem mexer na sua lista de chats nem no C-hub que está aberto:

```sh
XDG_STATE_HOME=$(mktemp -d) dbus-run-session -- target/debug/chube
```

O `dbus-run-session` separa a instância única do GTK: sem ele, o comando abre a janela do C-hub que já está rodando.

**Android:** JDK 21 e o Android SDK (plataforma 37, build-tools 37.0.0); veja a [parte 3.3 do guia](docs/configuracao.md#33-instalar-o-app).

```sh
cd android
./gradlew :app:testDebugUnitTest :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## Regras do projeto

- **Idioma:** interface, comentários e mensagens de commit em português do Brasil; nomes no código (funções, tipos,
  variáveis) em inglês. Comentários curtos que explicam o porquê, não o quê.
- **Desktop e Android andam juntos.** Os dois criam e leem as mesmas sessões do tmux. Os nomes (`chube-<8 hex>`), as
  opções do tmux e os scripts de `src/remote.rs` e de `android/app/src/main/java/com/pedro/chub/ssh/Scripts.kt` são os
  mesmos, e o `~/.config/chube/config.json` (tema, atalhos de pasta) é lido pelos dois. Ao mudar um lado, mude o outro no
  mesmo PR, ou explique no PR por que a mudança não se aplica (ex.: algo só do GTK ou só do celular).
- **Compatibilidade:** não quebre o que já está gravado (`config.json`, `~/.local/state/chube/chats.json`, preferências
  do app). Campo novo é opcional e tem padrão.
- **Segurança:** o app guarda uma chave SSH e o desktop roda comandos em servidores. Texto que vai para o shell passa
  sempre pelas funções de aspas (`remote::quote`, `Shell.quote`), nunca por interpolação direta. Não leia arquivos de
  credenciais dos agentes (`~/.claude/sessions/*.key`, `~/.codex/auth.json`).
- **Dados pessoais:** nada de IPs, nomes de host, usuários ou chaves reais no código, nos testes ou na documentação. Use
  exemplos (`minha-vps`, `100.64.0.1`, `user`).
- **Testes:** função nova com lógica testável (parser, script gerado, regra de nome) ganha teste de unidade. O que não
  der para testar automaticamente (interface, SSH real, celular) é descrito no PR: o que foi testado à mão e o que não foi.
- **Testes que falam com o modelo** gastam o limite do seu plano: use `--model haiku` e prompts curtos.

## Pull request

1. Faça um fork e crie um branch a partir da `main`.
2. Mantenha o PR focado numa coisa só. Refatoração separada de mudança de comportamento.
3. Antes de enviar, rode `cargo test` e, se mexeu no Android, `./gradlew :app:testDebugUnitTest`. A CI roda os dois.
4. Descreva o que muda, por quê e como testou. Para mudanças visuais, uma captura ajuda. Cuidado para a captura não
   mostrar nomes de sessões, pastas ou IPs que você não quer publicar.

Ao enviar um PR, você concorda em licenciar a sua contribuição sob a [GPL-3.0-only](LICENSE), a licença do projeto.
