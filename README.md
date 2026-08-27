# Jarvis

Assistente desktop pessoal que **entende comandos em português e mexe no PC**: abre sites
e programas, controla volume e mídia, e pesquisa no Google. Por texto ou por voz.

Ele também **conversa e lembra**. A memória é uma pasta de markdown no formato do
Obsidian (`memoria/`), que cresce sozinha e que você pode abrir, ler e corrigir.

Tudo local. A interpretação, a conversa e a memória rodam num modelo pequeno via
**Ollama**, e a transcrição no **whisper.cpp** — nada sai da máquina, e nada custa por
chamada. Cada comando deixa um **log no chat** com o que foi ouvido, o que o modelo
entendeu, no que deu e o que entrou ou saiu da memória, porque um assistente que abre
programas e guarda fatos sobre você erra em silêncio se ninguém puder auditar o que ele
achou que foi pedido.

**Chamar pelo nome executa.** Ditar "abre o youtube" enche o campo e espera o Enviar;
dizer "**Jarvis**, abre o youtube" manda direto para o roteador. O nome é a declaração
de que a frase é para ele — sem ele, conversa perto do microfone não vira comando.

**Ele responde falando** (ElevenLabs), em qualquer caminho — o que você digita no chat
também sai em voz alta. Basta ter a key e uma voz escolhida em Diagnóstico › Voz; sem
isso ele fica calado, e nada mais muda.

**E dá para simplesmente conversar.** O botão de ondas ao lado do microfone liga o
_modo conversa_: o microfone fica aberto, o silêncio de 1,2 s marca o fim da sua frase,
ele responde e volta a ouvir sozinho — sem clique nenhum entre uma frase e a próxima.
Tudo continua indo para o chat escrito, que é onde você lê o que o Whisper entendeu e o
que ele respondeu. Ali o vocativo não é exigido: ligar o modo já é a declaração de que a
fala é para ele.

**E ele enxerga.** "Que mouse é esse?" com o mouse na frente da câmera, "o que tem na
minha tela?" com uma janela aberta — ele decide sozinho para onde olhar, tira a imagem e
responde a pergunta. Quando a resposta não está na imagem ("quando abrem os ingressos
desse evento?"), ele identifica a coisa, **pesquisa** e responde com as fontes.

O que ainda não existe: wake word (o modo conversa ainda tem que ser ligado no botão) e
o agente com tool use — a `anthropicApiKey` já é usada, mas só para olhar.

---

## Pré-requisitos

| Ferramenta          | Versão testada       | Observação                                     |
| ------------------- | -------------------- | ---------------------------------------------- |
| Node.js             | 22.x – 24.x          | testado no 24.13                               |
| Rust (stable, MSVC) | 1.98                 | `rustup default stable-x86_64-pc-windows-msvc` |
| MSVC Build Tools    | 2019 (14.29) ou mais | Windows SDK junto — **é o linker do Rust**     |
| WebView2 Runtime    | qualquer             | Já vem no Windows 10/11 atualizado             |

Espaço em disco: reserve ~10 GB. O `src-tauri/target` sozinho passa de 3 GB no primeiro
build, e o MSVC Build Tools leva outros ~5 GB.

> **O MSVC Build Tools não é opcional e não vem com o Rust.** O `rustup` instala o
> compilador, mas quem junta os `.obj` no `.exe` é o `link.exe` da Microsoft. Sem ele
> nada em Rust compila nesta máquina — nem uma dependência transitiva. É de longe a
> causa nº 1 de `npm run tauri dev` falhar num clone novo; o diagnóstico exato está em
> [Quando dá errado](#quando-dá-errado).

---

## Instalação passo a passo

Do zero até a janela abrir. PowerShell, no Windows 10/11 x64.

### 1. Node.js

```powershell
winget install OpenJS.NodeJS.LTS
```

### 2. Rust, com o toolchain MSVC

```powershell
winget install Rustlang.Rustup
rustup default stable-x86_64-pc-windows-msvc
```

O `stable-x86_64-pc-windows-gnu` **não** serve: as dependências de COM (`windows`),
câmera (`nokhwa`) e áudio (`cpal`) esperam a ABI da MSVC.

### 3. MSVC Build Tools — o passo que todo mundo pula

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e `
  --override "--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Aceite o UAC. São ~5 GB e leva alguns minutos. O `--includeRecommended` é o que traz o
Windows SDK junto — sem ele o `link.exe` existe mas não acha `kernel32.lib`.

Alternativa pela GUI: baixe o **Build Tools for Visual Studio 2022** em
[visualstudio.microsoft.com/downloads](https://visualstudio.microsoft.com/downloads/)
(seção _Tools for Visual Studio_) e marque o workload **Desenvolvimento para desktop com
C++**.

**Feche e reabra o terminal depois** — o instalador mexe no PATH e no registro, e o
`rustc` só enxerga a mudança num processo novo.

Confira:

```powershell
rustc --print target-libdir      # tem que responder sem erro
```

### 4. WebView2

Já vem no Windows 10/11 atualizado. Para conferir:

```powershell
Test-Path "C:\Program Files (x86)\Microsoft\EdgeWebView\Application"
```

Se der `False`, instale o
[Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/).

### 5. Ollama e o modelo

```powershell
winget install Ollama.Ollama
ollama pull qwen2.5vl:3b
```

São 3,2 GB. Sem isso o app **abre e conversa**, mas em modo simulado — o intérprete fica
desligado. O porquê deste modelo específico está em [Os dois serviços locais](#os-dois-serviços-locais).

### 6. whisper.cpp — opcional, só para o comando por voz

Baixe [`whisper-blas-bin-x64.zip`](https://github.com/ggml-org/whisper.cpp/releases) e o
modelo [`ggml-small-q5_1.bin`](https://huggingface.co/ggerganov/whisper.cpp), e
descompacte os dois em `%APPDATA%\com.jarvis.app\whisper\`.

**O zip empacota tudo dentro de uma pasta `Release\`, e o app espera os arquivos
soltos** — extrair preservando a estrutura deixa o `whisper-server.exe` um nível abaixo
de onde `core::services` procura, e o erro que aparece é "não achei o Whisper", como se
o download não tivesse acontecido. Tem que ficar plano:

```
%APPDATA%\com.jarvis.app\whisper\
├── whisper-server.exe
├── ggml-small-q5_1.bin
├── whisper.dll, ggml*.dll, libopenblas.dll   ← ao lado do exe, não em subpasta
└── (o resto do zip, inofensivo)
```

Em PowerShell, achatando na extração:

```powershell
$dest = "$env:APPDATA\com.jarvis.app\whisper"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead("$HOME\Downloads\whisper-blas-bin-x64.zip")
$zip.Entries | Where-Object Name | ForEach-Object {
  [IO.Compression.ZipFileExtensions]::ExtractToFile($_, (Join-Path $dest $_.Name), $true)
}
$zip.Dispose()
Move-Item "$HOME\Downloads\ggml-small-q5_1.bin" $dest
```

O modelo tem **182 MB** — se o download vier menor, veio truncado, e o servidor sobe e
morre sem transcrever. `curl -L -C - --retry 10` retoma de onde parou.

Sem essa pasta o app sobe normalmente; só o botão de microfone devolve erro (e agora ele
**mostra** esse erro no chat, em vez de não fazer nada).

### 7. Clone, instale e rode

```powershell
git clone <url-do-repo>
cd jarvis-tauri-ia
npm install
npm run tauri dev
```

O primeiro `tauri dev` compila ~600 crates e leva **10–20 minutos**. Os seguintes são
segundos. A janela abre sozinha quando terminar.

### Os dois serviços locais

O app **sobe os dois sozinho** quando precisa, e derruba ao sair — mas os arquivos
precisam existir. Nenhum é baixado automaticamente: são centenas de megabytes, e essa é
uma decisão do dono da máquina.

**Ollama** — interpreta os comandos.

```bash
winget install Ollama.Ollama
ollama pull qwen2.5vl:3b
```

**Um modelo multimodal para tudo**, e isso não é preferência — é o que cabe. Com 4 GB de
VRAM o Ollama não segura dois modelos, e a primeira chamada depois de uma troca levou
**67 segundos**. Como o roteador é texto e o "o que é isso?" é imagem, dois modelos
significariam pagar a troca a cada duas mensagens.

O `qwen2.5vl:3b` foi escolhido medindo, em duas rodadas:

| modelo         | roteia | enxerga             | português | latência da visão |
| -------------- | ------ | ------------------- | --------- | ----------------- |
| `qwen2.5:3b`   | 13/15  | não                 | sim       | —                 |
| `moondream`    | —      | sim                 | **não**   | (força a troca)   |
| `gemma3:4b`    | —      | inventa nome de app | sim       | ~17 s             |
| `qwen2.5vl:3b` | 15/15  | sim                 | sim       | ~2–3,5 s          |

Evite modelos com modo de _thinking_ (a família qwen3, por exemplo): `think: false`
combinado com `format` faz o Ollama descartar silenciosamente a restrição de schema, e
ligar o raciocínio custa 3–4 s por comando.

Nem o `gemma3:4b` nem o `qwen2.5vl:3b` cabem inteiros nos 4 GB — os dois rodam ~55% em
CPU (`ollama ps` mostra). O qwen ainda assim responde 5× mais rápido.

**whisper.cpp** — transcreve a fala. Baixe [`whisper-blas-bin-x64.zip`](https://github.com/ggml-org/whisper.cpp/releases)
e o modelo [`ggml-small-q5_1.bin`](https://huggingface.co/ggerganov/whisper.cpp), e
descompacte os dois em `%APPDATA%\com.jarvis.app\whisper\`. O `base` é fraco demais em
português; o `large-v3-turbo` não cabe no orçamento de latência de uma CPU de 4 núcleos.

O `whisper-rs` foi descartado de propósito: compilar exigiria CMake **e** LLVM/Clang
instalados (o bindgen precisa do `libclang`), ~1,5 GB de toolchain no caminho de quem
clonar o repo — para ganhar o modelo residente, que o servidor já mantém carregado.

Latências medidas nesta máquina (i5-11300H, GTX 1650): primeira chamada ao Ollama ~90 s
(carrega o modelo na VRAM), depois ~0,4 s. Se a transcrição incomodar, a release
`whisper-cublas-*-bin-x64.zip` embute o runtime CUDA e roda na GPU sem instalar o
toolkit — é trocar os arquivos, não o código.

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

### Bissetar antes de chutar

O `tauri dev` roda duas coisas ao mesmo tempo, e o log delas se mistura. Quando quebra,
rode cada metade sozinha para saber de quem é o erro:

```powershell
npx next build            # só o frontend
cd src-tauri; cargo check # só o backend Rust
```

## Quando dá errado

### `linking with link.exe failed` — falta o MSVC Build Tools

O erro real, num clone novo sem os Build Tools:

```
error: linking with `link.exe` failed: exit code: 1
note: `link.exe` returned an unexpected error
note: you may need to install Visual Studio build tools with the "C++ build tools" workload
error: could not compile `proc-macro2` (build script) due to 1 previous error
```

Repare que quebra em `proc-macro2`, `serde_core`, `libc` — **build scripts de
dependências**, antes de chegar em qualquer linha deste projeto. Código nenhum daqui está
errado; é a toolchain que está incompleta. Resolve com o [passo 3](#3-msvc-build-tools--o-passo-que-todo-mundo-pula).

**A pegadinha que faz perder tempo:** o Git for Windows instala um
`C:\Program Files\Git\usr\bin\link.exe` — que é o `link` do GNU coreutils, para criar
hard links, e não tem nada a ver com o linker da Microsoft. Se o MSVC não estiver
instalado, o `rustc` cai nesse do PATH, e ele falha com uma mensagem que não explica
nada. Diagnóstico:

```powershell
where.exe link.exe
```

Se a **primeira** linha for a do Git e não houver nenhuma sob `Microsoft Visual Studio`,
é isso. Confirme que o MSVC de verdade está lá:

```powershell
& "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe" `
  -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
```

Sem saída = o workload C++ não foi instalado (provavelmente instalou o Build Tools sem o
`--add ...VCTools`). Não tente resolver mexendo no PATH: o `rustc` acha o linker certo
sozinho pelo registro, desde que ele exista.

### Outros

| Sintoma                                     | Causa                                   | Correção                                       |
| ------------------------------------------- | --------------------------------------- | ---------------------------------------------- |
| `error: linker link.exe not found`          | mesma coisa, outra redação do rustc     | passo 3                                        |
| `cannot open input file 'kernel32.lib'`     | MSVC sem o Windows SDK                  | reinstale com `--includeRecommended`           |
| `toolchain ...windows-gnu` no `rustup show` | toolchain errada                        | `rustup default stable-x86_64-pc-windows-msvc` |
| Porta 3000 ocupada                          | outro `next dev` de pé                  | mate o processo — a porta é fixa no `devUrl`   |
| Janela abre em branco                       | WebView2 ausente                        | passo 4                                        |
| Respostas genéricas, comando não executa    | Ollama fora do ar ou modelo não baixado | `ollama list` tem que mostrar `qwen2.5vl:3b`   |
| Microfone devolve erro                      | `whisper-server.exe` não está na pasta  | passo 6                                        |
| `cargo` reclama de clippy no build          | `[lints.clippy] all = "deny"`           | é de propósito — corrija o lint                |

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
npm run test:js       # node --test — hoje só o VAD do modo conversa

cd src-tauri
cargo fmt             # rustfmt (max_width = 100)
cargo clippy --all-targets
cargo test            # contrato, estado, storage, validação de alvo e reamostragem

# Mexe no volume do sistema de verdade, então fica fora do test normal.
# É o único jeito de provar o ComGuard — o erro que ele evita não aparece na
# compilação, aparece como microfone que para de abrir depois de mexer no volume.
cargo test fala_com_o_mixer -- --ignored --nocapture
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
├── commands/             a fronteira do invoke() — chat.rs, settings.rs, system.rs, voice.rs
├── core/                 domínio, sem dependência do Tauri
│   ├── chat.rs           tipos do contrato + o mock (rede de segurança)
│   ├── agent/            intent.rs (roteador) + converse.rs (papo e extração) + o log
│   ├── memory/           a pasta de markdown: nota.rs, busca.rs, rotinas.rs
│   ├── system/           AGE sobre o SO: target.rs (validação) + audio.rs (COM)
│   ├── services.rs       sobe e derruba o Ollama e o whisper-server
│   ├── voice/            mic.rs (cpal), stt.rs (whisper.cpp), tts.rs (ElevenLabs)
│   ├── vision/           ENTENDE a imagem: claude.rs + ollama.rs, atrás da mesma `ver()`
│   ├── casa.rs           ouve os aparelhos Tuya (Positivo, EKAZA) se anunciando na rede
│   └── automation/       PERCEBE o ambiente: webcam (nokhwa) + tela (xcap)
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

**O papel `system` carrega o log de ações.** Uma jogada do agente pode empurrar DUAS
mensagens no histórico — o log do gatilho e a resposta —, e por isso `chatStore.send()`
recarrega o histórico em vez de dar append no que o comando devolveu. Guardar o log como
`ChatMessage` (e não como campo do envelope) é o que faz ele sobreviver a fechar e
reabrir a janela, que é justamente o ponto de ter um log.

O `MessageBubble` desenha esse papel sem bolha e sem lado, porque é registro da máquina e
não fala:

```
GATILHO    abre o youtube
INTERPRETE qwen2.5:3b · 0.4 s
AÇÃO       open_site · url=https://www.youtube.com
RESULTADO  ok · 24 ms
```

A linha `GATILHO` parece redundante com a bolha do usuário logo acima — não é, quando o
comando veio por voz: ali fica **o que o Whisper ouviu**, que é a informação que falta
quando o comando errado dispara. Conversa fiada não gera log; só comando.

### Comandos

| Comando                                                                                                                       | Arquivo                  |
| ----------------------------------------------------------------------------------------------------------------------------- | ------------------------ |
| `send_message`, `get_history`, `clear_history`                                                                                | `commands/chat.rs`       |
| `get_settings`, `save_settings`                                                                                               | `commands/settings.rs`   |
| `show_window`, `hide_window`, `toggle_window`, `minimize_window`, `toggle_maximize_window`, `is_window_maximized`, `quit_app` | `commands/system.rs`     |
| `start_recording`, `stop_recording`, `is_recording`, `transcribe`, `list_voices`, `speak_text`, `stop_speaking`               | `commands/voice.rs`      |
| `open_webcam`, `close_webcam`, `is_webcam_open`, `capture_webcam_frame`, `capture_screenshot`                                 | `commands/automation.rs` |
| `discover_devices`                                                                                                            | `commands/casa.rs`       |

**Controlar o PC não adicionou nenhum comando.** Abrir site, abrir programa, volume e
mídia são chamados pelo agente dentro do `send_message` — não pelo frontend. É por isso
que a validação de entrada mora no Rust: o que chega em `core::system` veio de um modelo
de linguagem interpretando fala, e essa é a fronteira de confiança do app.

### Eventos (backend → UI)

| Evento                   | Quem emite | Quem escuta                  |
| ------------------------ | ---------- | ---------------------------- |
| `jarvis://open-settings` | `tray.rs`  | `src/hooks/useTrayEvents.ts` |

A transcrição NÃO virou evento, ao contrário do que este README previa. Ela tem começo e
fim marcados pelo clique do usuário, e a resposta é uma só — é pergunta e resposta, e um
evento teria um produtor e um consumidor já em chamada direta. O canal de eventos volta a
fazer sentido na wake word, que é o caso de verdade: o Rust empurra sem ninguém pedir.

---

## Modo conversa

O laço é `microfone aberto → silêncio → Whisper → agente → ElevenLabs → microfone
aberto`, e mora em `src/stores/sensorStore.ts`, ao lado do laço da webcam. Fica na
store, e não num hook, porque fechar o painel de chat não é dizer "pare de me ouvir".

**O VAD saiu de graça e não é um modelo.** O Rust já publicava o pico do microfone em
`jarvis://mic-level` 20×/s para desenhar o medidor; o mesmo número decide quando a frase
acabou. Nenhuma crate de VAD entrou, e `mic.rs` não mudou uma linha. A decisão pura está
em `src/lib/vad.ts` (limiar 0,02 — um degrau acima do `PICO_MINIMO` que o `stt.rs` já
trata como silêncio) e é o único pedaço com teste no frontend, em `vad.test.ts`.

O preço é o esperado de um VAD por energia: ventilador, TV e a voz de outra pessoa na
sala passam do limiar igual. O `ponytail:` no topo do arquivo aponta o upgrade.

**Uma sutileza que não é detalhe:** o laço amostra o nível num `setInterval`, e não
reagindo à mudança do valor. Em silêncio o pico chega `0` repetido, e o zustand não
notifica quem seleciona um valor igual ao anterior — reagir à mudança perderia
exatamente o caso que interessa, que é o silêncio parado.

**A fala mora no `chatStore.send`, não no laço.** Toda resposta é falada — a do modo
conversa e a do que você digitou —, porque a voz acompanha a RESPOSTA, não o caminho de
entrada. E como o `send` só volta quando ele calou, o laço da conversa ganha de graça o
sinal de quando pode reabrir o microfone: sem chave da ElevenLabs ele fica calado e o
`await` passa reto.

**Só a resposta, e só ela.** O log de ação (papel `system`) vem no `loadHistory` junto e
fica só escrito. Ninguém quer ouvir "open_site url=https://…" em voz alta — mas quer
poder ler depois.

**Sem chave, ele fica calado e sem erro.** Voz é opcional: um aviso vermelho a cada
mensagem digitada seria ruído por algo que ninguém pediu. Quem liga o modo conversa aí
sim recebe a recusa no clique, porque ali a voz é o ponto.

Enquanto ele fala, o microfone está fechado, então não existe eco para suprimir.
`stop_speaking` existe porque o `sleep_until_end` do rodio não tem cancelamento: sem
ele, desligar o modo no meio de uma resposta longa continuaria falando meia frase
adiante, e o botão de desligar estaria mentindo.

O TTS usa `eleven_flash_v2_5`, não o `eleven_multilingual_v2`. A síntese entra INTEIRA
na espera do usuário, a cada frase, e a diferença de expressividade entre os dois não
paga 1–2 s por resposta. Para narração longa, onde ninguém está esperando na frente, o
multilíngue continua sendo a escolha certa.

## Busca com resumo

Pergunta sobre o mundo abre o Google **e** responde no chat, com as fontes embaixo.
Quem responde é o modelo local, mas só com os trechos da busca na frente — perguntado
direto, um 3B inventa data, número e nome com toda a confiança.

O que eu queria era não depender de chave nenhuma. Não dá:

| fonte                        | resultado                                                |
| ---------------------------- | -------------------------------------------------------- |
| `html.duckduckgo.com`        | página de desafio anti-bot (`anomaly`, `challenge-form`) |
| `api.duckduckgo.com`         | `AbstractText` vazio em 4 de 4 consultas reais           |
| instâncias públicas de SearX | JSON desligado, 403 ou "Too Many Requests"               |
| **Wikipedia**                | **3 de 4** — falha só no que não é enciclopédico         |

Então **Wikipedia por padrão**, sem configurar nada, e um campo de chave do Brave Search
(grátis, 2000 buscas/mês) que troca a fonte por busca web de verdade. É a chave que
resolve "preço do dólar hoje" — a Wikipedia responde isso com "Opções (título)".

**Sem a chave, ele admite que não sabe.** Um teste com temperatura 0.2 devolveu uma
receita de pão de queijo inventada, com tempo de forno que não estava em trecho nenhum.
A 0, e com a regra "se pedirem passo a passo e os trechos só tiverem informação geral,
diga isso", a resposta virou _"os trechos fornecidos não contêm informações específicas
sobre como fazer pão de queijo"_. Preferir o "não sei" é o ponto.

---

## Visão

Pergunte "que mouse é esse?" com o mouse na frente da câmera, ou "o que tem na minha
tela?" com uma janela aberta. Quem decide onde olhar é o roteador (`Intent::Look` carrega
uma `fonte`), e quando ele não diz, `Fonte::Auto` resolve **pelo estado da webcam**:
ligada é uma declaração de intenção — ninguém abre a câmera e pergunta sobre a tela.

**A pergunta não é campo do `Intent`.** Ela já chega inteira no `handle` como o texto do
usuário; pedir ao 3B para extraí-la seria mais um campo para ele errar no prompt mais
frágil do app. O roteador decide só o que ninguém mais consegue decidir.

### Ele diz quando não sabe, em vez de inventar

A visão devolve `{resposta, buscar}`, não texto solto. `buscar` é a saída de emergência:
o modelo pode LER "Comic Con Experience 2026" num cartaz e ainda assim não saber quando
os ingressos abrem — isso não está na imagem. Sem um jeito de dizer isso, ele inventa uma
data. Preenchido, significa "identifiquei a coisa, o resto está fora da imagem", e o
`pesquisar_e_responder` que já existia faz o resto, com as fontes embaixo.

Vazio, ele responde direto e não gasta uma ida à internet — que é o caso do mouse.

**A autoridade inverte entre os dois casos**, e é por isso que são caminhos diferentes: em
"que mouse é esse" a imagem manda; em "quando abrem os ingressos" a resposta só existe na
busca. A função que reconciliava as duas fontes (`responder_sobre_o_que_viu`) foi
apagada — o prompt dela mandava ignorar a busca quando ela falasse de outra coisa, que é
exatamente o trecho que responde o segundo caso.

### Com chave da Anthropic, ele enxerga MUITO melhor

O `qwen2.5vl:3b` descreve bem uma cena ("um mouse preto sobre a mesa"), mas erra em
identificar QUAL mouse e em ler texto pequeno numa captura — 810 no OCRBench contra 883
do 7B. E erra sem avisar, que é o modo de falha caro. Com a `anthropicApiKey` preenchida a
imagem vai para o `claude-opus-5`; **sem chave, nada muda** — ele olha pelo Ollama, de
graça e offline, como já fazia.

Se o Claude falhar (rede fora, chave errada, recusa dos classificadores), ele **cai no
modelo local e responde assim mesmo**. Rede caindo não pode deixar o Jarvis cego, e por
isso `stop_reason: "refusal"` é checado antes de ler o `content`: numa recusa esse array
vem vazio, e pegar o primeiro bloco entraria em pânico.

Custo: ~US$ 0,01 por pergunta com imagem. Só quando a pergunta é sobre uma imagem —
conversa e comando continuam locais. Trocar por `claude-sonnet-5` custa ~40% disso com a
mesma resolução máxima, e é uma const em `core/vision/claude.rs`.

### A tela é reduzida antes de sair; a webcam não

`capture_screen` ganhou teto de largura (1568 px). Sem ele, 1080p em PNG vira 1–4 MB de
base64 no corpo da requisição, contra ~530 KB de um quadro de câmera em JPEG.

Continua PNG e não JPEG: texto de interface vira borrão nos artefatos do JPEG, e é
justamente texto que o modelo precisa ler. E a webcam continua indo em resolução cheia —
lá o que se lê é um rótulo pequeno a meio metro da lente, não um título de janela.

---

## Casa inteligente

A janelinha **Casa** escuta a rede e lista os aparelhos que encontrar. **Nada disso pede
conta, chave ou internet** — os dispositivos Tuya anunciam a si mesmos na rede local, e
este pedaço só ouve.

É janelinha flutuante (o mesmo `FloatingPanel` da conversa) e não gaveta como o
Diagnóstico, e a diferença não é de gosto: gaveta é para o que se lê e se fecha; a lista da
casa é para ficar aberta num canto enquanto você mexe em outra coisa.

**Ele conta os pacotes que não entendeu.** Sem esse número, dois silêncios com soluções
opostas dão exatamente a mesma tela vazia: _ninguém falou_ (rede errada, firewall, aparelho
fora da tomada) e _falaram num formato que não sei ler_. O primeiro se resolve no roteador,
o segundo no código.

**Positivo e EKAZA são a mesma coisa por baixo: Tuya.** As duas são rebrands da mesma
plataforma, como Nova Digital, Tramontina, Elgin e boa parte do que se vende barato aqui.
Uma leitura só enxerga todas.

Por enquanto ele só ENXERGA. Controlar depende da `local_key`, que é uma por aparelho e
só existe na nuvem da Tuya — esse é o próximo passo.

### A chave do anúncio é pública, e isso não é descuido

O broadcast vem cifrado em AES-128-ECB com uma chave fixa: o MD5 de `yGAdlopoPVldABfn`,
a mesma no mundo inteiro, usada por todo cliente Tuya que existe. Ela não protege nada e
não dá acesso a nada — serve só para ler o anúncio. Quem protege o controle de verdade é
a `local_key`.

Ela está no código como dezesseis bytes constantes em vez de ser calculada, para não
arrastar uma crate de MD5 para o projeto por causa de um valor que nunca muda.

### O controle infravermelho: o hub aparece, a TV e o ar não

Um hub de infravermelho (o "controle universal" que manda comando para ar-condicionado e
TV) é um aparelho Wi-Fi como os outros, e **ele** se anuncia na rede normalmente.

**A TV e o ar-condicionado nunca vão aparecer aqui, e isso não é defeito.** Eles não
estão na rede: são _sub-dispositivos virtuais_, que existem só como registros na nuvem da
Tuya. O hub não sabe o que é uma TV — ele sabe piscar um LED infravermelho com uma
sequência de pulsos, e **qual sequência corresponde a "desligar a TV" mora numa biblioteca
de códigos que a Tuya mantém na nuvem** e atualiza sozinha.

Isso tem uma consequência que vale saber antes de contar com ela: **mandar comando para o
ar pelo hub provavelmente vai exigir a nuvem**, ao contrário das lâmpadas e tomadas, que
funcionam 100% local depois da chave. É a limitação que a comunidade do Home Assistant
esbarra até hoje — a saída conhecida é trocar o firmware do hub por Tasmota, o que é outro
projeto.

### O que ele lista, e por que o 3.5 aparece mesmo sem funcionar

Cada aparelho vem com id, IP, **versão do protocolo** e modelo. A versão é o dado que
importa mais nesta fase: é ela que decide o que dá para usar para controlar depois.

Os aparelhos que falam **3.5** aparecem marcados como "ainda sem suporte" em vez de serem
escondidos. O quadro deles usa AES-GCM e o app não sabe ler — mas omiti-los faria você
procurar defeito no Wi-Fi por meia hora atrás de uma lâmpada que está bem ali.

### Por que não a crate `tuya-rs`

Ela faz isso e mais o protocolo de controle, mas exige `rustc 1.88` — e este projeto
declara `rust-version = "1.77.2"`. Subir o piso do repo inteiro por uma crate 0.2.1 sai
mais caro que as ~40 linhas de decodificação em `core/casa.rs`; a descoberta é a parte
simples do protocolo. Quando o CONTROLE entrar, aí vale reavaliar — aquela parte é bem
menos trivial, e reimplementar seria a decisão errada.

Entrou só a `aes`, para decifrar o anúncio.

### E a Alexa?

Fora, de propósito. A Amazon não tem API pública para mandar comando a um Echo — as
bibliotecas que fazem isso logam na sua conta com cookie e usam endpoints internos que
quebram sem aviso. E o caminho seria Jarvis → nuvem Amazon → Echo → nuvem Amazon → nuvem
Tuya → lâmpada, para um comando que cabe num pacote UDP dentro da sua própria casa.

---

## Memória

Mora em `memoria/`, no próprio projeto, e o formato é o do **Obsidian** — markdown com
frontmatter e `[[links]]`. Isso não é enfeite: é o que dá dois donos à memória. Ele
escreve quando aprende; você abre a pasta, lê o que ele entendeu de você, corrige o que
ficou torto e apaga o que não quer que ele saiba. E entra no git, então dá para ver no
diff o que ele aprendeu. `memoria/README.md` explica o layout por dentro.

**A memória não é o transcrito da conversa.** Cada nota é um documento sobre um assunto,
que é reescrito e cresce quando o assunto volta a aparecer. O que foi dito literalmente
fica em `historico.jsonl`, que serve só para a tela redesenhar as bolhas.

### Quatro chamadas ao modelo, não uma

| quando      | chamada                      | o que faz                                         |
| ----------- | ---------------------------- | ------------------------------------------------- |
| sempre      | `intent::interpret`          | classifica a frase em 1 de 18 verbos              |
| se for papo | `converse::responder`        | responde com histórico e notas relevantes         |
| se for papo | `converse::destilar_assunto` | decide SE a troca virou conhecimento, e sobre quê |
| se rendeu   | `converse::escrever_nota`    | reescreve a nota daquele assunto, inteira         |

Separar responder de aprender não foi escolha de gosto, foi medição. Numa chamada só,
com o schema devolvendo `{resposta, lembrar}`, um modelo de 3B faz um dos dois
trabalhos: ou papagueia a memória em toda resposta, ou para de aprender, ou — o caso que
encerrou a discussão — deixa os exemplos do aprendizado vazarem para dentro da resposta
("adotei um gato" respondido com "Mora em Recife."). Os números estão no topo de
`core/agent/converse.rs`.

E **destilar é separado de escrever** porque a nota precisa ser um documento: para
reescrever um documento o modelo tem que ver o que já estava lá, e só dá para buscar o
texto anterior depois de saber o assunto. Anexar numa chamada só produziria a pilha de
frases coladas na ordem em que foram ditas — exatamente o que a nota não deve ser.

### O modelo é péssimo em obedecer o formato, então o formato é imposto em código

Mesmo com a regra explícita no prompt, o 3B produziu tudo isto em produção:

| lixo                                        | de onde veio                              |
| ------------------------------------------- | ----------------------------------------- |
| nota abrindo com o próprio nome do assunto  | teimosia, apesar de "não repita o título" |
| `ASSUNTO DA NOTA` como primeira linha       | eco do rótulo do meu prompt               |
| a lista de notas vizinhas cuspida no começo | eco do bloco de contexto                  |
| `[[nota nova]]`                             | meu placeholder virou link para o nada    |
| duas perguntas do assistente coladas no fim | a nota virando ata da conversa            |

`converse::limpar_nota` corta todos, e cada um tem teste. **A regra fica no prompt E o
corte fica no código** — cinto e suspensório, porque cada um pega o que o outro deixa
passar. Brigar só com prompt custa rodada e nunca fecha de vez.

### Como ele escolhe o que lembrar na hora

Palavra-chave **mais um salto de `[[link]]`**. Perguntar "que horas eu acordo" casa com
`rotina-da-manha`; como aquela nota cita `[[academia]]`, a academia entra no prompt sem
ter sido mencionada. É o salto que transforma uma lista de notas num grafo — e vale
para os links que VOCÊ escrever na mão.

Não é embedding de propósito: um segundo modelo (`nomic-embed-text`, ~274 MB) disputaria
os 4 GB de VRAM com o intérprete, e o índice viveria dessincronizado de arquivos que
você edita por fora.

### Guardar tem dois caminhos, e um é confiável

- **Explícito** — "lembra que eu acordo 6h30". Passa pelo roteador: 11/11 nos testes.
- **Automático** — ele destila o assunto de cada troca e escreve a nota. É _best-effort_.

O balanço dos exemplos no prompt importa mais que as regras: com 6 negativos contra 4
positivos, o modelo respondeu "não rende" para tudo, inclusive para "o projeto usa Tauri
com Next" — que é quase literalmente um dos exemplos positivos. Invertendo o balanço e
acrescentando "na dúvida, responda true", ele passou a pegar o que importava.

**Ele erra na direção de anotar demais, de propósito.** Uma nota errada você apaga com
uma frase; conhecimento que nunca foi anotado some sem deixar rastro.

> ⚠️ **O que ele anota pode estar errado.** Uma nota real produzida por ele dizia que
> "Charlie Brown Jr. é conhecido por composições infantis, tocado em programas
> educacionais". Um 3B alucina sobre o mundo, e a nota registra fielmente a alucinação.
> Duas defesas: perguntas sobre o mundo são roteadas para `web_search`, que responde só
> com o que a busca trouxe; e a pasta é sua para corrigir. Não trate a base como fonte.

### O laço que faz ele melhorar com o uso

Notas do tipo `apelido` entram no prompt do **roteador**. Ensine uma vez —
"meu jogo é o steam" — e a partir daí `abre meu jogo` sai do modelo já como
`{"action":"open_app","name":"steam"}`. Nenhum peso é tocado; o que muda é o prompt.

O mesmo vale para as rotinas: toda ação executada vai para `acoes.jsonl`, e agrupar por
(ação, alvo, período do dia) vira `notas/rotinas-observadas.md`. Três vezes é hábito,
duas é acaso — e ação que falhou não conta, senão um nome errado repetido viraria
"rotina" e ele passaria a sugerir o próprio erro.

---

## Onde plugar cada feature futura

| Feature                       | Entra em                         | Chamado por                                 | Substitui                 |
| ----------------------------- | -------------------------------- | ------------------------------------------- | ------------------------- |
| Comando novo do PC            | `core/system/` + `Intent`        | `core::agent::execute`                      | —                         |
| Agente Claude + tool use      | `core/agent/`, ao lado do local  | `core::agent::handle`                       | o intérprete do Ollama    |
| Wake word                     | `core/voice/wake_word.rs`        | task de background no `setup` de `lib.rs`   | —                         |
| Outro backend de visão        | `core/vision/`, ao lado dos dois | `core::vision::ver`                         | o `if` da chave           |
| Controlar a casa por voz      | `core/casa.rs` + `Intent`        | `core::agent::handle`                       | —                         |
| Mouse, teclado (`enigo`)      | `core/automation/input.rs`       | `core::agent::execute`, com confirmação     | —                         |
| Busca por embedding           | `core/memory/busca.rs`           | `Memoria::contexto`                         | a busca por palavra-chave |
| Personalidade / system prompt | `core/agent/converse.rs`         | montado com o `assistant_name` das settings | —                         |

### O prompt do roteador é a peça mais frágil, e o teto é o balanço dos exemplos

Um bug real de produção: `"po enquanto nada, quero é ir pra casa pra poder jogar"` foi
classificado como `media_play_pause` e **pausou a música do usuário no meio de um
desabafo**. A causa não era a tabela de verbos — era que o prompt tinha 12 exemplos de
comando contra 2 de conversa, 6 para 1.

Rebalanceando (10 comandos, 4 perguntas do mundo, 9 conversas) e acrescentando a regra
"mencionar não é mandar", o placar foi de 15/18 com 3 falsos comandos para **23/23 com
zero**. Mesmo modelo, mesma temperatura.

**E ele degrada sozinho.** Cada feature nova chegou com exemplos de COMANDO e nenhum de
CONVERSA — música, webcam e visão levaram a razão de volta a **22:9**, quase na
proporção do bug. Agora existe um teste (`os_exemplos_de_comando_nao_afogam_os_de_conversa`)
que quebra quando um lado passa do dobro do outro; ele não sabe qual é o número certo,
só impede a regressão silenciosa. O balanço de hoje é 20 / 4 / 13, e as conversas
incluem de propósito frases que **citam** tela e objeto sem pedir para olhar — "tela azul
de novo, que ódio" é o falso amigo mais óbvio do `look`.

A lição vale para os outros dois prompts: a extração de memória teve o mesmo
comportamento, e a correção foi a mesma. **Ao mexer em qualquer prompt aqui, conte os
exemplos de cada lado antes de reescrever as regras.**

**Adicionar um comando novo do PC são quatro pontos**: a variante no enum `Intent`, o
verbo em `ACOES`, o braço no `match` de `execute`, e a linha na tabela do system prompt.
O `o_schema_e_o_enum_falam_a_mesma_lingua` cobra os dois primeiros — ele quebra se o enum
e o schema divergirem, que é o que impede o modelo de pedir uma ação que não existe.

**Os pontos 3 e 4 falham em silêncio**, e vale saber disso antes de confiar no verde: o
`execute` tem um braço catch-all, então um verbo esquecido lá compila e devolve string
vazia, e nada testa o texto do prompt. E existe um **quinto** ponto que só aparece quando
o verbo não é de hardware — o braço em `handle`, que é onde vivem `play_music`, `look`,
`webcam_on/off`, `remember`, `forget` e `alias`. Nenhum deles tem uma linha em `execute`.

Mouse e teclado sintéticos continuam reservados, e agora por um motivo mais concreto: as
ações de hoje não dependem de qual janela está em foco e nunca precisam de confirmação.
Clicar em coordenada depende das duas coisas, e é aí que as travas entram.

---

## Configurações

Salvas em `%APPDATA%\com.jarvis.app\settings.json`:

```json
{
  "anthropicApiKey": "",
  "assistantName": "Jarvis",
  "elevenLabsApiKey": "",
  "ttsVoiceId": "",
  "ollamaUrl": "http://localhost:11434",
  "ollamaModel": "qwen2.5vl:3b",
  "memoriaPath": "",
  "braveApiKey": "",
  "spotifyClientId": "",
  "spotifyClientSecret": ""
}
```

`memoriaPath` vazio cai na pasta `memoria/` do projeto em desenvolvimento, e no
diretório de dados do usuário num app instalado (ninguém escreve em Program Files).

`ollamaModel` **vazio desliga o intérprete** e o Jarvis volta às respostas simuladas. É a
saída de emergência sem precisar de um booleano só para isso — mesmo padrão do
`ttsVoiceId`, onde vazio significa "usa o padrão", e do `anthropicApiKey`, onde vazio
significa "olhe pelo modelo local".

As chaves ficam em texto puro nesta versão — migrar para o keyring do SO era hipotético
enquanto nenhuma delas era usada; agora duas custam dinheiro de verdade. `AppSettings` usa
`#[serde(default)]`, então campos novos não quebram arquivos antigos.

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
- **Crate `windows` em vez de `windows-sys`**: o `IAudioEndpointVolume` precisa dos
  wrappers de COM, e escrever a vtable à mão seria mais `unsafe` para o mesmo resultado.
  Custo de build ~zero — o `windows` 0.62 já vinha no `Cargo.lock` via cpal/nokhwa/tauri,
  então declarar a mesma minor reaproveita a árvore em vez de somar uma quarta cópia.
- **`ShellExecuteW` em vez de `cmd /C start`**: no Windows o `Command` reserializa os
  argumentos numa linha só e o `cmd` reinterpreta `&`, `|`, `^` e `%VAR%`. O texto vem de
  um modelo transcrevendo fala — "gatos & cachorros" bastaria para virar dois comandos.
- **`url` para validar endereço**, e não parsing à mão: já estava no lock, e uma allowlist
  de esquema em cima do `Url::parse` derruba `file:`, `javascript:` e `data:` de uma vez,
  sem lista negra para manter atualizada.
- **whisper.cpp por HTTP, não `whisper-rs`**: evita CMake + LLVM no caminho de quem clona.
  O motivo completo está nos pré-requisitos.
- **Nenhuma crate de VAD**: o medidor de volume que já existia para desenhar a barra é o
  mesmo sinal que decide quando a frase acabou. Detalhes no [Modo conversa](#modo-conversa).
- **`node --test` em vez de vitest/jest**: o Node roda TypeScript sozinho, e o frontend
  tem exatamente uma função que merece teste. Um runner com config, plugin e watcher para
  cinco `assert` seria mais infraestrutura que código testado.
