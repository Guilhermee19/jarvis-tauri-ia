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

O que ainda não existe: wake word e o agente da Anthropic com tool use — a
`anthropicApiKey` continua nas configurações, sem consumidor.

---

## Pré-requisitos

| Ferramenta          | Versão usada         | Observação                                     |
| ------------------- | -------------------- | ---------------------------------------------- |
| Node.js             | 22.x                 |                                                |
| Rust (stable, MSVC) | 1.97                 | `rustup default stable-x86_64-pc-windows-msvc` |
| MSVC Build Tools    | 2019 (14.29) ou mais | Windows SDK junto — é o linker do Rust         |
| WebView2 Runtime    | qualquer             | Já vem no Windows 10/11 atualizado             |

Espaço em disco: reserve ~6 GB. O `src-tauri/target` sozinho passa de 3 GB no primeiro build.

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
| `start_recording`, `stop_recording`, `is_recording`, `transcribe`, `list_voices`, `speak_text`                                | `commands/voice.rs`      |
| `open_webcam`, `close_webcam`, `is_webcam_open`, `capture_webcam_frame`, `capture_screenshot`                                 | `commands/automation.rs` |

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
| sempre      | `intent::interpret`          | classifica a frase em 1 de 14 verbos              |
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

| Feature                       | Entra em                        | Chamado por                                 | Substitui                 |
| ----------------------------- | ------------------------------- | ------------------------------------------- | ------------------------- |
| Comando novo do PC            | `core/system/` + `Intent`       | `core::agent::execute`                      | —                         |
| Agente Claude + tool use      | `core/agent/`, ao lado do local | `core::agent::handle`                       | o intérprete do Ollama    |
| Wake word                     | `core/voice/wake_word.rs`       | task de background no `setup` de `lib.rs`   | —                         |
| Mouse, teclado (`enigo`)      | `core/automation/input.rs`      | `core::agent::execute`, com confirmação     | —                         |
| Busca por embedding           | `core/memory/busca.rs`          | `Memoria::contexto`                         | a busca por palavra-chave |
| Personalidade / system prompt | `core/agent/converse.rs`        | montado com o `assistant_name` das settings | —                         |

### O prompt do roteador é a peça mais frágil, e o teto é o balanço dos exemplos

Um bug real de produção: `"po enquanto nada, quero é ir pra casa pra poder jogar"` foi
classificado como `media_play_pause` e **pausou a música do usuário no meio de um
desabafo**. A causa não era a tabela de verbos — era que o prompt tinha 12 exemplos de
comando contra 2 de conversa, 6 para 1.

Rebalanceando (10 comandos, 4 perguntas do mundo, 9 conversas) e acrescentando a regra
"mencionar não é mandar", o placar foi de 15/18 com 3 falsos comandos para **23/23 com
zero**. Mesmo modelo, mesma temperatura.

A lição vale para os outros dois prompts: a extração de memória teve o mesmo
comportamento, e a correção foi a mesma. **Ao mexer em qualquer prompt aqui, conte os
exemplos de cada lado antes de reescrever as regras.**

**Adicionar um comando novo do PC são quatro pontos**, e o `cargo test` cobra o quarto: a
variante no enum `Intent`, o verbo em `ACOES`, o braço no `match` de `execute`, e a linha
na tabela do system prompt. O teste `o_schema_e_o_enum_falam_a_mesma_lingua` quebra se o
enum e o schema divergirem — é o que impede o modelo de pedir uma ação que não existe.

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
  "ollamaModel": "qwen2.5:3b",
  "memoriaPath": "",
  "braveApiKey": ""
}
```

`memoriaPath` vazio cai na pasta `memoria/` do projeto em desenvolvimento, e no
diretório de dados do usuário num app instalado (ninguém escreve em Program Files).

`ollamaModel` **vazio desliga o intérprete** e o Jarvis volta às respostas simuladas. É a
saída de emergência sem precisar de um booleano só para isso — mesmo padrão do
`ttsVoiceId`, onde vazio significa "usa o padrão".

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
