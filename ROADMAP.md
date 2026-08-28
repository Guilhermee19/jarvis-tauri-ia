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
        │              Camada de IA (via API)                  │
        │  STT (Whisper) → Agente LLM (tool use) → TTS         │
        └──────────────────────────────────────────────────────┘
```

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
| Agente de IA              | API da Anthropic (Claude) com tool use                         | OpenAI GPT com function calling                                                      |
| TTS (texto→fala)          | ElevenLabs (qualidade alta, cloud)                             | Piper TTS (local, mais rápido e grátis)                                              |
| Controle de mouse/teclado | crate `enigo` (Rust, equivalente ao pyautogui)                 | sidecar Python com pyautogui                                                         |
| Screenshot                | crate `xcap` ou `screenshots` (Rust)                           | `mss` via sidecar Python                                                             |
| Webcam                    | crate `nokhwa` (Rust)                                          | `opencv-python` via sidecar Python                                                   |
| Visão (entender a tela)   | Claude com input de imagem (a screenshot)                      | GPT-4V                                                                               |
| Memória/personalidade     | SQLite local + system prompt customizado                       | Vector DB (ex: Chroma) se crescer muito                                              |

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
> Dica: você pode manter tudo em Rust nativo (mais performático e um único binário) **ou** usar um sidecar em Python só para a parte de automação (pyautogui/mss), já que você já conhece essas libs. O sidecar conversa com o Tauri via HTTP local ou stdin/stdout. Comece com o que for mais rápido pra você validar a ideia — dá pra trocar depois.

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
- **Fala (TTS):** integração com um serviço de TTS (ElevenLabs ou Piper — decidir nesse momento), com um botão "testar voz" que fala uma frase fixa, e configuração de qual voz usar
- **Webcam:** botão para abrir/fechar a webcam, com preview ao vivo na UI, usando uma crate como `nokhwa` no Rust; captura de um frame sob demanda (ainda sem nenhum reconhecimento — isso vem depois, quando plugado ao agente com visão)
- **Visão de tela (screenshot):** botão para capturar a tela atual (crate `xcap`) e mostrar o preview na UI, confirmando que a captura funciona corretamente (multi-monitor incluso, se você tiver mais de uma tela)
- Todas essas capacidades ficam organizadas nos módulos `voice` e `automation` já criados na v0.1 (`/src-tauri/src/core/voice` para mic/TTS, `/src-tauri/src/core/automation` para webcam/screenshot), expostas como comandos Tauri independentes, sem nenhuma lógica de decisão ainda

**Entrega:** uma tela de "diagnóstico"/testes no app onde você consegue gravar áudio, ouvir ele falar, ver a webcam ligar e tirar um print da tela — tudo funcionando isoladamente, pronto para ser conectado à IA nas próximas versões.

---

### 🟢 v0.2 — Cérebro básico (chat de texto + fala)

**Objetivo:** conectar com um LLM e ter uma conversa de verdade — já aproveitando o TTS da v0.1.5 para ele responder em voz desde o início.

- Integração com a API da Claude (texto → texto)
- Prompt de sistema definindo a personalidade inicial dele
- Histórico de conversa persistido localmente (SQLite ou arquivo local)
- Streaming da resposta na UI
- A resposta já é falada automaticamente usando o módulo de TTS criado na v0.1.5 (reaproveitando, não recriando)

**Entrega:** você digita, ele responde com texto **e** em voz, já com uma "cara" própria.

---

### 🟡 v0.3 — Voz de entrada conectada ao chat (push-to-talk)

**Objetivo:** você fala com ele dentro do fluxo real de conversa (ainda sem wake word), reaproveitando a captura de microfone da v0.1.5.

- Botão/hotkey manual para começar a gravar ("push to talk"), usando o módulo de microfone já existente
- STT (Whisper) transcrevendo o áudio pra texto
- Esse texto vira o input do agente (reaproveita o pipeline de chat da v0.2)

**Entrega:** aperta uma tecla, fala, ele entende e responde em texto e voz.

---

### 🟠 v0.4 — Wake word (o "Jarvis" de verdade)

**Objetivo:** ele fica ouvindo sozinho, sem precisar apertar nada.

- Integração com Porcupine (ou openWakeWord) rodando em loop, baixo consumo de CPU
- Ao detectar a palavra-gatilho, inicia a gravação automaticamente
- Detecção de silêncio/fim de fala (VAD) para saber quando parar de gravar
- Feedback visual/sonoro de "estou ouvindo"

**Entrega:** você fala "Jarvis" (ou o nome que escolher) e ele já entra em modo escuta sozinho.

---

### 🟠 v0.5 — Agente com decisão de ação (tool use)

**Objetivo:** ele para de só "conversar" e começa a decidir o que fazer.

- Estrutura de tools/functions no agente: `pesquisar_na_web`, `responder_direto`, `executar_acao_no_pc` (ainda vazia)
- O LLM decide, a partir do pedido, qual tool usar
- Implementação real da tool de pesquisa web (search API)

**Entrega:** ele entende se deve só responder, pesquisar algo, ou (no futuro) agir no PC — e já pesquisa de verdade.

---

### 🔴 v0.6 — Controle do computador

**Objetivo:** ele consegue mexer no PC.

- Implementação da tool `executar_acao_no_pc` usando `enigo` (ou sidecar com pyautogui)
- Ações básicas: abrir programas, digitar texto, clicar em posições, atalhos de teclado
- Camada de **confirmação/segurança**: antes de executar ações "perigosas" (fechar programas, deletar algo), ele pergunta antes
- Log de tudo que ele executa (auditoria)

**Entrega:** ele consegue, por exemplo, abrir o navegador e digitar algo, sozinho.

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

### 🟣 v0.8 — Personalidade e memória de longo prazo

**Objetivo:** ele deixa de ser genérico e vira "o seu" assistente.

- Sistema de memória persistente (fatos sobre você, preferências, contexto de projetos)
- Prompt de personalidade mais elaborado (tom de voz, jeito de falar, humor)
- Ajuste fino de como ele resume/lembra conversas antigas (para não estourar contexto)
- Configuração de personalidade pela própria UI (ajustar tom, nome, etc.)

**Entrega:** ele lembra de coisas entre sessões e tem um jeito consistente de ser.

---

### ⚫ v1.0 — Polimento e robustez

**Objetivo:** deixar de ser protótipo e virar algo que você usa todo dia.

- Autostart com o sistema operacional
- Tratamento de erros (API fora do ar, sem internet, mic falhando)
- Permissões e sandboxing das ações no PC (whitelist de apps/comandos permitidos)
- Painel de configurações completo (trocar de LLM, TTS, wake word, atalhos)
- Otimização de custo (cache, escolher quando usar modelo mais barato vs mais caro)

**Entrega:** app estável, configurável, seguro, pronto pro dia a dia.

---

### 🟡 Casa inteligente — **fase 1 feita** (fora da numeração original)

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

### 🚀 Depois do v1.0 (ideias de expansão)

- Plugins/skills customizáveis (ex: integração com Spotify, Google Calendar, Home Assistant)
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
