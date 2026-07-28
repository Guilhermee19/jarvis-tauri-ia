# Jarvis

Assistente desktop pessoal. **v0.1 — esqueleto**: a UI de chat, a bandeja do sistema e a
persistência de configurações funcionam de verdade; a inteligência é mockada no Rust.

O objetivo desta versão não é a feature, é a base: a estrutura e o contrato entre
frontend e backend já estão desenhados para receber wake word, STT/TTS, o agente da
Anthropic com tool use, controle de mouse/teclado e memória em SQLite — sem reescrever
o que já existe.

---

## Pré-requisitos

| Ferramenta          | Versão usada         | Observação                                     |
| ------------------- | -------------------- | ---------------------------------------------- |
| Node.js             | 22.x                 |                                                |
| Rust (stable, MSVC) | 1.97                 | `rustup default stable-x86_64-pc-windows-msvc` |
| MSVC Build Tools    | 2019 (14.29) ou mais | Windows SDK junto — é o linker do Rust         |
| WebView2 Runtime    | qualquer             | Já vem no Windows 10/11 atualizado             |

Espaço em disco: reserve ~6 GB. O `src-tauri/target` sozinho passa de 3 GB no primeiro build.

## Rodando

```bash
npm install
npm run tauri dev
```

O `tauri dev` sobe o Next em `localhost:3000` e abre a janela nativa apontando para ele
(hot reload vale para o frontend; mudança em Rust recompila e reinicia o app).

Abrir `localhost:3000` direto no navegador também funciona para mexer em CSS, mas todo
`invoke()` falha — a UI mostra o erro em vez de quebrar (ver `isTauriRuntime` em
`src/lib/tauri/client.ts`).

## Build

```bash
npm run tauri build
```

Gera o instalador em `src-tauri/target/release/bundle/` (`.msi` e `.exe` no Windows).

## Qualidade

```bash
npm run lint          # ESLint (config do Next + prettier)
npm run format        # Prettier
npm run typecheck     # tsc --noEmit

cd src-tauri
cargo fmt             # rustfmt (max_width = 100)
cargo clippy --all-targets
cargo test            # contrato de serialização, estado e storage
```

`[lints.clippy] all = "deny"` está no `Cargo.toml`, então lint do Rust falha o build.

---

## Estrutura

### Frontend — `src/`

```
src/
├── app/                  rotas do App Router (layout + a única página)
├── components/
│   ├── home/             HomeScreen + JarvisCore — o HUD de fundo
│   ├── panels/           FloatingPanel + PanelLayer — as janelas internas
│   ├── chat/             ChatPanel, MessageList, MessageBubble, ChatInput
│   ├── tray-window/      TitleBar, BottomNav, HudFrame — o chrome da janela
│   ├── settings/         SettingsPanel + SettingsForm
│   └── ui/               Button, Input, icons — genéricos
├── hooks/                useBootstrap, useChat, useTrayEvents, usePointerDrag
├── lib/tauri/            wrappers tipados de invoke(), um arquivo por domínio
├── stores/               zustand: chat, settings, panels
├── types/                espelho TS dos structs Rust
└── styles/               tema Tailwind v4
```

A regra que mantém isso saudável: **componente não chama `invoke()` direto**. Ele fala com
a store, a store fala com `lib/tauri`, e só ali existe a fronteira com o Rust. Quando o
agente real entrar, nada em `components/` precisa mudar.

### Linguagem visual

HUD: quase-preto azulado, grade de fundo, um único acento ciano e texto pequeno em caixa
alta com tracking largo. Nada além de `--color-accent` introduz cor — trocar esse token
retinge o app inteiro.

As peças ficam em `styles/globals.css` (`.hud-grid`, `.hud-vignette`, `.hud-glow`,
`.hud-rotor` e as animações `animate-hud-*`, todas desligadas em `prefers-reduced-motion`),
`components/home/JarvisCore.tsx` (o núcleo em SVG) e `components/tray-window/HudFrame.tsx`
(moldura e colchetes).

Uma pegadinha que custou tempo: o `<main>` **não** pode ter `bg-base`. O fundo dele é
pintado depois dos filhos com `-z-10`, o que apagava a grade. A cor base vem do `body`.

### Janelas internas, não rotas

O app não navega entre telas. O HUD é o fundo permanente da janela e cada feature é uma
**janelinha flutuante** por cima dele: abre, fecha, minimiza, arrasta e redimensiona dentro
da janela principal. `BottomNav` funciona como barra de tarefas — clicar abre, restaura ou
minimiza o painel correspondente.

- `src/stores/panelStore.ts` — estado de cada painel (aberto, minimizado, posição, tamanho)
  e a ordem de empilhamento. `toggle` implementa a semântica de barra de tarefas: fechado
  abre, minimizado restaura, aberto minimiza.
- `src/components/panels/FloatingPanel.tsx` — o chrome da janelinha (cabeçalho arrastável,
  minimizar, fechar, canto de redimensionar).
- `src/components/panels/PanelLayer.tsx` — a área onde os painéis vivem, entre a barra de
  título e a de ícones. É ela que limita até onde dá para arrastar, e um `ResizeObserver`
  reposiciona o que ficaria fora quando a janela principal muda de tamanho.
- `src/hooks/usePointerDrag.ts` — arrasto com `setPointerCapture`, usado por mover e
  redimensionar. Sem a captura, arrastar rápido solta o painel no meio do caminho.

Para adicionar uma feature nova (voz, automação): um id em `PanelId`, um par de valores
padrão em `DEFAULTS`, uma entrada em `ITEMS` do `BottomNav` e um `<FloatingPanel>` no
`PanelLayer`. As linhas "offline" do HUD já marcam quais vêm por aí.

Detalhe de implementação: os botões do cabeçalho param a propagação do `pointerdown` —
sem isso, clicar em "fechar" arrastaria o painel junto.

### Backend — `src-tauri/src/`

```
src-tauri/src/
├── main.rs               shim: chama jarvis_lib::run()
├── lib.rs                monta o app (estado, bandeja, eventos, comandos)
├── state.rs              AppState: histórico + settings
├── commands/             a fronteira do invoke() — chat.rs, settings.rs, system.rs
├── core/                 domínio, sem dependência do Tauri
│   ├── chat.rs           tipos do contrato + o mock da v0.1
│   ├── agent/            (stub) agente Claude + tool use
│   ├── voice/            (stub) wake word, STT, TTS
│   └── automation/       (stub) enigo + xcap
├── storage/              trait SettingsStore + implementação JSON
├── config/               AppSettings
├── tray.rs               ícone e menu da bandeja
└── window.rs             mostrar/esconder/minimizar + fechar-para-bandeja
```

`core/` não conhece Tauri: nenhum `#[tauri::command]`, nenhuma janela. É o que vai permitir
testar o agente e o pipeline de voz sem subir o app.

---

## Contrato frontend ↔ backend

`src-tauri/src/core/chat.rs` e `src/types/chat.ts` são espelhos. Serde serializa em
camelCase e o enum de papel em minúsculas:

```ts
interface ChatMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: number // epoch ms
}

interface ChatResponse {
  message: ChatMessage
}
```

**O histórico mora no backend** (`AppState`), não no React. O frontend é um espelho: ele
chama `get_history()` ao montar e a janela pode ser escondida e reaberta sem perder a
conversa. É essa decisão que deixa a memória persistente entrar depois só trocando o
`Vec<ChatMessage>` em memória por uma tabela SQLite atrás de `storage/`.

`ChatResponse` é um envelope de propósito: hoje carrega só a mensagem, mas existe para
receber `stopReason`, `toolCalls` e uso de tokens sem mudar a assinatura do comando.

### Comandos

| Comando                                                                      | Arquivo                |
| ---------------------------------------------------------------------------- | ---------------------- |
| `send_message`, `get_history`, `clear_history`                               | `commands/chat.rs`     |
| `get_settings`, `save_settings`                                              | `commands/settings.rs` |
| `show_window`, `hide_window`, `toggle_window`, `minimize_window`, `quit_app` | `commands/system.rs`   |

### Eventos (backend → UI)

| Evento                   | Quem emite | Quem escuta                  |
| ------------------------ | ---------- | ---------------------------- |
| `jarvis://open-settings` | `tray.rs`  | `src/hooks/useTrayEvents.ts` |

O caminho de eventos já está montado porque a voz vai depender dele: wake word e
transcrição são empurradas pelo Rust, não pedidas pela UI.

---

## Onde plugar cada feature futura

| Feature                        | Entra em                         | Chamado por                                 | Substitui                       |
| ------------------------------ | -------------------------------- | ------------------------------------------- | ------------------------------- |
| Agente Claude + tool use       | `src-tauri/src/core/agent/`      | `commands/chat.rs::send_message`            | `core::chat::mock_reply`        |
| Wake word / STT / TTS          | `src-tauri/src/core/voice/`      | task de background no `setup` de `lib.rs`   | —                               |
| Mouse, teclado (`enigo`)       | `src-tauri/src/core/automation/` | tools do agente (não é comando do frontend) | —                               |
| Screenshot (`xcap`)            | `src-tauri/src/core/automation/` | idem                                        | —                               |
| Memória persistente (SQLite)   | `src-tauri/src/storage/`         | `state.rs`                                  | o `Vec` em memória do histórico |
| Personalidade / system prompt  | `core/agent/` + `config/mod.rs`  | montado com o `assistant_name` das settings | —                               |
| UI de voz (botão de microfone) | `src/hooks/useVoiceInput.ts`     | `components/chat/ChatInput.tsx`             | os stubs de hook                |

O `send_message` já é `async` justamente porque a implementação real faz I/O de rede — a
troca do mock pelo agente não muda a assinatura nem o contrato TypeScript.

---

## Configurações

Salvas em `%APPDATA%\com.jarvis.app\settings.json`:

```json
{
  "anthropicApiKey": "",
  "assistantName": "Jarvis"
}
```

A chave fica em texto puro nesta versão — migrar para o keyring do SO quando a integração
real entrar. `AppSettings` usa `#[serde(default)]`, então campos novos não quebram arquivos
antigos.

## Comportamento de janela

- Fechar no **X** esconde para a bandeja; encerrar de verdade só pelo menu → **Sair**.
  O Jarvis é feito para ficar rodando em background.
- Clique esquerdo na bandeja alterna mostrar/esconder; botão direito abre o menu.
- A janela roda com `decorations: false` (620×700, mínimo 480×540) — a barra de título é o
  componente `tray-window/TitleBar.tsx`, e o arrasto vem do `data-tauri-drag-region`.

## Decisões de biblioteca

- **zustand** para estado global: ~1 kB, sem provider e sem boilerplate de reducer. Context
  daria mais código pelo mesmo resultado; Redux é exagero para três stores.
- **Nenhuma biblioteca de UI** (shadcn, MUI): a janela tem quatro componentes visuais. Um
  design system entra quando houver o que justificar.
- **Storage próprio em vez de `tauri-plugin-store`**: o plugin amarra o formato ao JSON dele.
  Um trait próprio deixa trocar por SQLite sem tocar nos comandos.
- **Tipos TS escritos à mão** em vez de `ts-rs`: são dois structs. Vale gerar quando o
  contrato crescer (aí o `ts-rs` entra como build-dependency do `src-tauri`).
- **Tailwind v4** com tema em CSS (`@theme`), sem `tailwind.config.js`.
