# Roadmap — Assistente "Jarvis" (Next.js + Tauri)

## Visão geral da arquitetura

```
┌─────────────────────────────────────────────────────────────────┐
│                            Tauri App                            │
│                                                                 │
│  ┌──────────────┐        IPC / invoke         ┌──────────────┐  │
│  │  Next.js UI  │ ◄─────────────────────────► │  Rust Core   │  │ 
│  │  (frontend)  │                             │  (backend)   │  │
│  └──────────────┘                             └──────┬───────┘  │
│                                                       │         │
│              ┌────────────────────────────────────────┤         │
│              │            │            │               │        │
│         Wake Word     Áudio (mic/    Controle do    Screenshot  │
│         (Porcupine)    speaker)      SO (enigo)      (xcap)     │
└──────────────┼────────────┼──────────────┼───────────────┼──────┘
               │            │              │               │
               ▼            ▼              ▼               ▼
        ┌──────────────────────────────────────────────────────┐
        │           Camada de IA — TUDO LOCAL, hoje            │
        │  whisper-server → Ollama (tool use) → Chatterbox     │
        │  três processos nesta máquina, nenhuma API paga      │
        └──────────────────────────────────────────────────────┘
```

> **O diagrama acima já não é aspiração.** O plano original tinha "via API" nas três
> caixas; hoje as três rodam localmente, e a última a sair foi o TTS. Sobrou **uma** chave
> opcional em todo o app: a da Anthropic, e só para o Claude olhar imagem melhor que o
> modelo local — sem ela ele enxerga pelo Ollama, de graça.

**Por que essa divisão:**

- **Tauri (Rust)** é quem tem acesso ao SO de verdade: microfone contínuo, hotkeys globais, controle de mouse/teclado, screenshot, system tray, autostart. É leve (não é Electron) e perfeito pra um app que fica sempre aberto em background.
- **Next.js** só cuida da UI (janela de chat, overlay, configurações). Não precisa fazer nada pesado.
- A "IA" fica isolada em um módulo de orquestração no Rust (ou um sidecar) que decide: transcrever → mandar pro agente → agente decide se responde, pesquisa ou executa uma ação no SO.

---

## Stack recomendada

| Camada                    | Ferramenta sugerida                                            | Alternativa                                                                          |
| ------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| UI                        | Next.js (rodando dentro do Tauri)                              | —                                                                                    |
| App shell                 | Tauri v2                                                       | Electron (mais pesado, evite)                                                        |
| Wake word                 | ~~Porcupine (Picovoice)~~ **deixou de ser opção** — ver abaixo | **openWakeWord** (open source, tem modelo `hey_jarvis` pronto; em Rust via `oww-rs`) |
| STT (fala→texto)          | whisper.cpp local (rápido, privado)                            | API da OpenAI/Deepgram (mais fácil de integrar no início)                            |
| Agente de IA              | **Ollama local** (`qwen2.5vl:3b`) com JSON schema — 21 verbos   | ~~API da Anthropic~~ (virou opcional, só para visão)                                 |
| TTS (texto→fala)          | **Piper local** (padrão, 0,14 s) **+ Chatterbox** (clona a voz)  | ~~ElevenLabs~~ — saiu, era a última API paga                                          |
| Controle de mouse/teclado | crate `enigo` (Rust, equivalente ao pyautogui)                 | sidecar Python com pyautogui                                                         |
| Screenshot                | crate `xcap` ou `screenshots` (Rust)                           | `mss` via sidecar Python                                                             |
| Webcam                    | crate `nokhwa` (Rust)                                          | `opencv-python` via sidecar Python                                                   |
| Visão (entender a tela)   | Claude com input de imagem (a screenshot)                      | GPT-4V                                                                               |
| Memória/personalidade     | **Markdown em pasta**, com índice — não SQLite                  | Vector DB (ex: Chroma) se crescer muito                                              |
| Navegador embutido        | **Webview filho do Tauri** (`add_child`, feature `unstable`)    | ~~`<iframe>`~~ — medido: Google/YouTube mandam `X-Frame-Options: SAMEORIGIN`          |

> ⚠️ **O Porcupine não é mais grátis.** O free tier do Picovoice foi desligado em
> **30/06/2026** e as AccessKeys gratuitas foram desabilitadas — e mesmo antes disso, as
> wake words customizadas do plano grátis expiravam a cada 30 dias e tinham que ser
> regeradas. Hoje é licença enterprise paga. O substituto grátis é o **openWakeWord**,
> que tem um modelo `hey_jarvis` pré-treinado e wrapper em Rust (`oww-rs`), ao custo de
> trazer o ONNX Runtime para o build e três `.onnx` para baixar — no mesmo estilo do
> whisper.cpp. Alternativa sem dependência nenhuma: o VAD do modo conversa já detecta
> quando alguém fala, e `comandoEnderecado` (`src/lib/dictation.ts`) já reconhece o nome
> — dá para ter wake word por texto reusando o que existe, pagando uma transcrição por
> frase falada perto do microfone.
>
> **Sobre o sidecar Python:** a dica original era usá-lo para automação (pyautogui/mss).
> Isso não aconteceu — captura de tela e webcam ficaram em crates Rust (`xcap`, `nokhwa`).
> Mas o sidecar entrou por outra porta: **o TTS**. E foi forçado, não escolhido: a única
> variante do Chatterbox com export ONNX (que rodaria em Rust puro) é a Turbo, e ela fala
> só inglês. Português exige a Multilingual, que só existe em PyTorch.

---

## Roadmap por versões

### 🟢 v0.1 — Esqueleto do app (sem IA ainda)

**Objetivo:** ter o app rodando, com janela sempre disponível.

- Setup do projeto Tauri + Next.js
- Janela principal com chat simples (input de texto + histórico)
- Ícone na bandeja do sistema (system tray) com abrir/fechar/sair
- Configuração básica (onde ficam as API keys, etc.)

**Entrega:** um app que abre, tem uma UI de chat, mas ainda não pensa em nada.

---

### 🟢 v0.1.5 — Sensores e atuadores básicos (ainda sem IA)

**Objetivo:** habilitar todos os "sentidos" e a "voz" do Jarvis como capacidades isoladas e testáveis, antes de conectar qualquer inteligência a eles. Isso faz as próximas versões (que já dependem de IA) ficarem muito mais simples, porque a infraestrutura de captura já vai existir e só precisa ser plugada no agente.

- **Microfone:** captura de áudio via Tauri, com um botão de teste na UI que grava e mostra um indicador de volume/atividade (sem transcrição ainda, ou com STT básico rodando só como demo, sem ligar ao chat)
- **Fala (TTS):** integração com um serviço de TTS, com um botão "testar voz" que fala uma frase fixa e configuração de qual voz usar. _A decisão adiada aqui foi tomada duas vezes: primeiro a ElevenLabs (cloud, paga), depois o **Chatterbox local** — nem ela nem o Piper, porque nenhum dos dois clona a voz do dono. Ver "Voz local", mais abaixo._
- **Webcam:** botão para abrir/fechar a webcam, com preview ao vivo na UI, usando uma crate como `nokhwa` no Rust; captura de um frame sob demanda (ainda sem nenhum reconhecimento — isso vem depois, quando plugado ao agente com visão)
- **Visão de tela (screenshot):** botão para capturar a tela atual (crate `xcap`) e mostrar o preview na UI, confirmando que a captura funciona corretamente (multi-monitor incluso, se você tiver mais de uma tela)
- Todas essas capacidades ficam organizadas nos módulos `voice` e `automation` já criados na v0.1 (`/src-tauri/src/core/voice` para mic/TTS, `/src-tauri/src/core/automation` para webcam/screenshot), expostas como comandos Tauri independentes, sem nenhuma lógica de decisão ainda

**Entrega:** uma tela de "diagnóstico"/testes no app onde você consegue gravar áudio, ouvir ele falar, ver a webcam ligar e tirar um print da tela — tudo funcionando isoladamente, pronto para ser conectado à IA nas próximas versões.

---

### 🟢 v0.2 — Cérebro básico (chat de texto + fala)

**Objetivo:** conectar com um LLM e ter uma conversa de verdade — já aproveitando o TTS da v0.1.5 para ele responder em voz desde o início.

- ✅ Integração com um LLM — **acabou sendo o Ollama local**, não a API da Claude. Um 3B
  na máquina responde de graça e sem rede; a chave da Anthropic virou opcional e só serve
  para visão
- ✅ Prompt de sistema definindo a personalidade — e viraram **duas** (Jarvis e Ultron)
- ✅ Histórico persistido localmente
- ✅ A resposta é falada automaticamente, reaproveitando o TTS da v0.1.5
- ✅ Streaming da resposta na UI — a bolha cresce frase a frase, no mesmo passo da fala
  (`jarvis://reply-chunk`)

**Entrega:** você digita, ele responde com texto **e** em voz, já com uma "cara" própria.

---

### 🟢 v0.3 — Voz de entrada conectada ao chat (push-to-talk) — **feita, e passou disso**

**Objetivo:** você fala com ele dentro do fluxo real de conversa (ainda sem wake word), reaproveitando a captura de microfone da v0.1.5.

- Botão/hotkey manual para começar a gravar ("push to talk"), usando o módulo de microfone já existente
- STT (Whisper) transcrevendo o áudio pra texto
- Esse texto vira o input do agente (reaproveita o pipeline de chat da v0.2)

**Entrega:** aperta uma tecla, fala, ele entende e responde em texto e voz.

Passou do combinado: além do push-to-talk existe o **modo conversa**, um laço contínuo que
fecha o microfone enquanto ele fala e reabre quando cala. O fim de frase é detectado pelo
medidor de volume que já desenhava a barra — **sem crate de VAD nenhuma**.

---

### 🟠 v0.4 — Wake word (o "Jarvis" de verdade)

**Objetivo:** ele fica ouvindo sozinho, sem precisar apertar nada.

- Integração com Porcupine (ou openWakeWord) rodando em loop, baixo consumo de CPU
- Ao detectar a palavra-gatilho, inicia a gravação automaticamente
- Detecção de silêncio/fim de fala (VAD) para saber quando parar de gravar
- Feedback visual/sonoro de "estou ouvindo"

**Entrega:** você fala "Jarvis" (ou o nome que escolher) e ele já entra em modo escuta sozinho.

---

### 🟢 v0.5 — Agente com decisão de ação (tool use) — **feita**

**Objetivo:** ele para de só "conversar" e começa a decidir o que fazer.

- ✅ **21 verbos** num enum plano (`core/agent/intent.rs`), não um `oneOf` aninhado: o
  aninhamento vira uma grammar que um 3B erra muito mais
- ✅ O modelo escolhe o verbo, e o schema garante só a FORMA — quem valida a combinação
  verbo↔campos é o serde, no parse
- ✅ Pesquisa web de verdade: Wikipedia por padrão, Brave Search com chave. Sem a chave,
  **ele admite que não sabe** em vez de inventar
- ✅ Agir no PC saiu da caixa "ainda vazia": volume, mídia, abrir programa e abrir site

**Lição que custou uma reescrita:** um campo `acao` como enum dentro do verbo falhou em 7
de 8 frases reais contra o modelo de 3B. A saída foi **três verbos separados**
(`smart_home`, `smart_color`, `smart_bright`) — mais entradas na lista, menos campos por
entrada, e o modelo acerta.

**Entrega:** ele entende se deve só responder, pesquisar algo ou agir no PC — e faz os três.

---

### 🟡 v0.6 — Controle do computador — **metade feita, e a outra metade está reservada de propósito**

**Objetivo:** ele consegue mexer no PC.

- ✅ Abrir programa, abrir site, volume (subir/baixar/definir/mudo) e teclas de mídia
- ✅ **Log de tudo que ele executa** — cada ação vira uma linha `system` no histórico, com
  verbo, alvo e quanto tempo levou
- ⬜ `enigo`: mouse e teclado sintéticos. **Não é falta de tempo, é escolha.** As ações de
  hoje não dependem de qual janela está em foco e nenhuma precisa de confirmação. Clicar em
  coordenada depende das duas coisas — é aí que a camada de confirmação passa a ser
  obrigatória, e ela não existe ainda
- ⬜ Camada de confirmação para ações perigosas — entra junto com o `enigo`, não antes

**Entrega parcial:** ele abre programa e site sozinho. Digitar e clicar continuam fora.

---

### 🟢 v0.7 — Visão e reconhecimento (entender a tela e a webcam) — **feita, menos a parte que depende da v0.6**

**Objetivo:** ele "enxerga" de verdade — a captura crua já existe desde a v0.1.5, aqui é a parte inteligente: conectar isso ao agente.

- ✅ Verbo `look` com `fonte` (tela ou webcam), reaproveitando os módulos de captura da v0.1.5. Não viraram duas tools: é um verbo com um campo, porque quem escolhe é o mesmo modelo de 3B que já classifica tudo, e cada campo a mais é uma chance a mais de ele errar
- ✅ Envio da imagem pro Claude (input multimodal) junto com o pedido — `core/vision/claude.rs`, com o Ollama local como padrão e como fallback
- ✅ O agente usa a webcam para reconhecimento visual (identificar objetos, ler algo mostrado à câmera)
- ✅ **Extra, e é o que faz a feature valer:** quando a resposta não está na imagem, ele identifica a coisa e **pesquisa** em vez de inventar
- ❌ Usar a tela pra decidir **onde clicar** e confirmar se uma ação deu certo — isso depende do `enigo` da v0.6, que ainda não existe

**Entrega:** ele olha pela webcam ou para a tela, responde o que você perguntou, e diz "não sei" pesquisando em vez de alucinando. Clicar no que vê fica para depois da v0.6.

---

### 🟢 v0.8 — Personalidade e memória de longo prazo — **feita**

**Objetivo:** ele deixa de ser genérico e vira "o seu" assistente.

- ✅ Memória persistente em **markdown numa pasta**, com índice — e não SQLite. Arquivo de
  texto se lê, se edita e se versiona no git; um `.db` binário não
- ✅ Duas personas completas (Jarvis e Ultron): cor do app, voz e tom de conversa, trocadas
  na hora e sem reiniciar
- ✅ Rotinas observadas: ele anota padrões do que você pede
- ✅ Configuração pela UI: nome, persona, e o clipe de voz de cada uma

**Entrega:** ele lembra de coisas entre sessões e tem um jeito consistente de ser.

---

### 🟡 v1.0 — Polimento e robustez — **em curso**

**Objetivo:** deixar de ser protótipo e virar algo que você usa todo dia.

- ⬜ Autostart com o sistema operacional — não existe ainda
- ✅ Tratamento de erros: cada serviço local tem mensagem própria dizendo **o que falta e
  onde baixar**, e o app sobe normalmente sem nenhum deles
- ✅ Fronteira de confiança no Rust: o que chega em `core::system` veio de um modelo
  interpretando fala, então a validação de URL e de nome de programa mora lá, não na UI
- ✅ Painel de configurações e bancada de diagnóstico (microfone, voz, webcam, tela)
- ⬜ Whitelist de apps/comandos — hoje a trava é por categoria de ação, não por lista
- ✅ **Otimização de custo virou irrelevante**: não há mais custo por uso. Os três serviços
  são locais, e a única chave opcional que sobrou é a da visão

---

### 🟡 Casa inteligente — **fases 1 a 3 feitas** (fora da numeração original)

Não estava no roadmap; entrou por pedido. O painel **Casa** já lista os aparelhos Tuya
(Positivo, EKAZA e as outras rebrands) ouvindo a rede local, sem conta nem chave.

- ✅ Descoberta por broadcast UDP, com id, IP, modelo e **versão do protocolo**
- ✅ O 3.5 é decifrado (AES-GCM, mesma chave pública do 3.3); o que não abre ainda
  aparece na lista com o endereço, em vez de sumir
- ✅ O código de retorno de 4 bytes do quadro clássico, que fazia dois aparelhos desta
  casa serem contados como "pacote ignorado" e nunca aparecerem
- ✅ **Fase 2** — `local_key` + nome, da Cloud API da Tuya, guardados num `casa.json`.
  Trial gratuito e renovável; a chave continua válida depois que ele expira, porque é do
  aparelho e não da nuvem
- ✅ **Fase 3** — controlar. Os **três** protocolos, cada um verificado contra um
  aparelho real pelo teste `controle_real`: 3.3 (AES-ECB, CRC-32), 3.4 (sessão negociada,
  HMAC-SHA256) e 3.5 (sessão, quadro GCM). Duas armadilhas custaram caro e estão anotadas
  no código: o último passo do aperto de mão **não é respondido** (esperar por ele consome
  o timeout inteiro e parece recusa), e a chave de sessão do **3.5 é derivada em AES-GCM**,
  não no AES-ECB do 3.4 — a conta errada dá uma chave válida e errada, e o aparelho recusa
  o primeiro comando sem dizer nada.
  Junto: cor, brilho e temperatura de lâmpada (data points 20–24), ícone por categoria,
  cartão enxuto com os detalhes técnicos atrás do "i", e o verbo `smart_home` no roteador.
  Uma trava por CATEGORIA impede que aparelho sem liga-desliga ganhe botão — sem ela um
  gateway ZigBee, que expõe um booleano não documentado no DP 4, receberia comando às
  cegas.
- ⬜ **Fase 4** — outras marcas. Um segundo backend em `core/casa.rs`, no molde do `if` da
  chave que a visão já usa. O **Home Assistant** é o candidato: "todas as marcas" é
  literalmente o problema que ele resolve, e falar com ele é menos código que a Tuya nativa

**A Alexa ficou de fora de propósito** — a Amazon não tem API pública para mandar comando
a um Echo, e as bibliotecas que fazem isso logam na conta com cookie e quebram sem aviso.

---

### 🟢 Voz local — **feita, com dois motores** (fora da numeração original)

A última API paga saiu, e no lugar dela entraram **dois motores locais**. A escolha entre
eles é entre velocidade e identidade, e os números são medidos nesta máquina (RTX 2060,
frase de 52 caracteres):

| motor | por frase | fator | pico | voz |
| --- | --- | --- | --- | --- |
| **Piper** (padrão) | **0,14–0,21 s** | 0,04× (25× mais rápido que tempo real) | 1,000 | catálogo, 4 em pt-BR |
| **Chatterbox** | 6,6–8,1 s | ~1,4× (mais LENTO que tempo real) | 0,28 | **a do dono**, clonada |

O Piper roda em **CPU** e deixa a GPU inteira para o Ollama. O Chatterbox (Resemble AI,
licença MIT) **clona a voz do dono** a partir de um clipe de ~10 segundos — sem treino, sem
conta, sem crédito —, e é a única forma de ter a própria voz.

- ✅ Terceiro serviço local em `core/services.rs`, no mesmo ciclo dos outros dois: bate na
  porta, sobe se ninguém atender, morre junto com o app
- ✅ Uma voz clonada **por persona** — o campo já existia para os ids da ElevenLabs, e
  passou a guardar o nome do clipe sem mudar `AppSettings::voz()`
- ✅ Seletor de arquivo nativo (`tauri-plugin-dialog`, o **primeiro e único** plugin do
  projeto: não há como abrir um diálogo nativo sem ele)
- ✅ Os três portões de "tem API key?" viraram "tem clipe?" — a pergunta antiga era global,
  a nova é por persona

**Como o Piper entrou:** o Chatterbox sozinho era lento demais para conversa, e os dois
botões de aceleração dele foram medidos e **os dois pioraram** — `cfg_weight: 0.0` (9,0 s) e
`TTS_BF16=on` (8,6 s, porque a 2060 é Turing e emula bf16). O `stream: true` do servidor
também: 8,9 s até o primeiro byte, pior que os 7 s do bloco inteiro. Sem botão para girar,
a saída foi outro motor.

- ✅ Um campo escolhe o motor, e **cada motor guarda a própria voz por persona** — quatro
  campos ao todo. Não é exagero: o servidor do Piper **não recusa** um id de voz que não
  existe, usa a padrão em silêncio. Com um par só de campos, trocar de motor deixaria um
  `.mp3` no lugar do id e a fala sairia com a voz errada, sem erro nenhum
- ✅ O `PICO_TIPICO_DA_FALA` do HUD passou a seguir o motor: 1,0 no Piper (que normaliza) e
  0,28 no Chatterbox. Um número só faria o núcleo saturar num e ficar parado no outro
- ✅ Fatiar a resposta por frase e tocar a primeira enquanto a segunda é gerada. Medido:
  corta **85%** da espera numa resposta longa. Feito junto com o streaming do Ollama, e é
  ele que devolve o Chatterbox para uma conversa: os sete segundos por frase deixam de ser
  silêncio entre elas
- ⬜ Treinar a voz do dono DENTRO do Piper (~1300 frases, ~8 h de gravação). É o único
  caminho conhecido para ter voz própria E resposta instantânea

---

### 🟢 Navegador embutido — **feito** (fora da numeração original)

"abre o youtube" e "pesquisa preço do dólar" deixaram de jogar a pessoa para fora do app.

- ✅ Abas dentro de uma janelinha, com barra de endereço e histórico
- ✅ Cada aba é um **webview filho** (`Window::add_child`), não um `<iframe>` — foi medido
  que Google, YouTube e DuckDuckGo respondem `X-Frame-Options: SAMEORIGIN` e ficariam em
  branco, e "abre o youtube" é o exemplo canônico do roteador
- ✅ O agente não abre a aba direto: devolve um `AcaoDeUi` e quem age é a tela, porque
  `core/` não conhece o Tauri
- ✅ Saída para o navegador do sistema no botão ↗ — senha salva, extensão e impressão ainda
  precisam dele

**A armadilha que custou um travamento:** um `#[tauri::command]` **sem `async` roda na
thread principal**, dentro do callback do WebView2. Criar um webview dali faz o wry abrir
um message loop aninhado (`wait_with_pump`) dentro de um handler do WebView2, e o app
congela sem volta. Todo comando que cria ou destrói aba é `#[tauri::command(async)]`, e
nenhum `Mutex` do módulo é segurado durante uma chamada ao Tauri.

---

### 🟡 Latência do turno — **medida, duas correções feitas** (fora da numeração original)

Das ~7 etapas de um turno de voz, **só uma tinha número**. O `turno_de_verdade`
(`core/agent/mod.rs`) cronometra todas, no molde do `fala_de_verdade`.

- ✅ **Whisper com os núcleos físicos.** Ele subia com `-t 4` fixo, do laptop de 4 núcleos
  onde o projeto nasceu. Medido: 4 → 2,29 s, **8 → 1,66 s**, 16 → 2,38 s. Passar dos
  núcleos físicos perde o ganho inteiro
- ✅ **Anotar deixou de vir antes de responder.** Destilar o assunto e reescrever a nota são
  duas chamadas ao Ollama que não mudam a resposta, e valiam 33% do trabalho do turno. Hoje
  rodam em `spawn` depois de a fala já ter começado
- ✅ **Streaming do Ollama, fatiado por frase.** Era o que sobrava na frente do usuário:
  1,34 s de `responder` que só terminavam no último token. Hoje aquela chamada — a única
  das sete — vai com `"stream": true`, `converse::fim_de_frase` corta a resposta em frases,
  e a fila de `commands/voice.rs` sintetiza uma à frente enquanto a atual toca. A espera
  passou de "a resposta inteira" para "a primeira frase dela"
- ⬜ Build CUDA do Whisper — troca de arquivos, sem código

**O TTS nunca foi o gargalo.** A 0,15 s ele é o elo mais rápido; trocar de motor (foi
cogitado o MeloTTS, que além disso não fala português) otimizaria 3% do turno. Medir antes
foi o que evitou gastar a leva no lugar errado.

---

### 🚀 Depois do v1.0 (ideias de expansão)

- ✅ ~~Spotify~~ — já feito: tocar faixa nomeada, com o widget de "tocando agora"
- Plugins/skills customizáveis (Google Calendar, Home Assistant)
- Execução de tarefas em background/agendadas ("todo dia às 9h me resuma meus e-mails")
- Modo "colaborativo": ele narra o que está vendo/fazendo em tempo real enquanto executa ações longas
- Suporte a múltiplos perfis de voz/persona

---

## Ordem de prioridade se quiser simplificar ainda mais

Se quiser um caminho ainda mais enxuto pra validar rápido:

1. v0.1 + v0.1.5 (esqueleto + sensores básicos) — deixe mic, TTS, webcam e screenshot funcionando isoladamente antes de qualquer IA
2. v0.2 + v0.3 (chat de texto/voz funcionando via push-to-talk) — **valide a personalidade e o LLM primeiro**
3. v0.4 (wake word) só depois que o resto do pipeline de voz já estiver 100%
4. v0.5 → v0.7 (agente + controle + visão) é a parte mais trabalhosa — pode levar mais tempo que todo o resto junto

Isso evita você travar cedo tentando resolver wake word + controle de PC ao mesmo tempo, que são as partes tecnicamente mais chatas.

> **Como o caminho realmente foi:** a ordem acima se cumpriu até a v0.3. Daí em diante o
> projeto seguiu por pedido, não por numeração — casa inteligente, painel de desempenho,
> navegador embutido e voz local entraram na frente da wake word (v0.4), que continua sendo
> a única peça do plano original ainda intocada. E a v0.6 parou na metade **de propósito**:
> mouse e teclado sintéticos só entram junto com a camada de confirmação.
