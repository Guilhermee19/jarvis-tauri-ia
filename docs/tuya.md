# Ligar a casa inteligente ao Jarvis, do zero

Este é o passo a passo completo para tirar da nuvem da Tuya o **nome** e a **chave de
controle** de cada aparelho da sua casa, uma vez só, e depois controlar tudo localmente.

Leva uns 20 minutos. A maior parte é preencher formulário no site da Tuya, e **quase todo
o tempo perdido costuma ser em três armadilhas** que este documento marca com ⚠️ — todas
falham em silêncio ou com uma mensagem que aponta para o lugar errado.

---

## O que você ganha, e o que não precisa

**Achar os aparelhos na rede não precisa de nada disto.** O painel Casa já lista o que
existe ouvindo o broadcast da rede local, sem conta, sem chave e sem internet.

O que a nuvem entrega é o que o anúncio da rede nunca traz:

- o **nome** que você deu no app ("Luz Cozinha"), sem o qual o aparelho é um id de 22
  caracteres;
- a **`local_key`**, o segredo de cada aparelho, sem a qual a porta 6668 recusa qualquer
  comando.

**É uma visita só.** Depois de importadas, as chaves ficam gravadas e o controle é local
para sempre — funciona com o roteador sem link, e continua funcionando depois que o
projeto trial da Tuya expirar, porque a chave é do aparelho e não da nuvem.

---

## Antes de começar: pareie tudo primeiro

Conclua no app **Smart Life** o pareamento de todos os aparelhos que você quer controlar.

> ⚠️ **A `local_key` muda toda vez que um aparelho é pareado de novo.** Importar antes de
> parear significa importar uma chave que já nasce velha. Se você resetar um aparelho
> depois, precisa reimportar — e o sintoma de esquecer isso é um erro de decifragem que
> não se parece nem um pouco com a causa.

Um aparelho em modo de pareamento **não está na sua rede** — ele está esperando o
SmartConfig, ou levantou um Wi-Fi próprio (`SmartLife-XXXX`). Nenhuma varredura vai
encontrá-lo até que o pareamento termine.

---

## Passo 1 — Conta no Tuya IoT Platform

Crie uma conta em [iot.tuya.com](https://iot.tuya.com). É grátis.

**É uma conta diferente da do Smart Life.** As duas vão ser ligadas no Passo 4 — e é
justamente essa ligação que faz a API enxergar os seus aparelhos.

---

## Passo 2 — Criar o Cloud Project

`Cloud` → `Project Management` → **Create Cloud Project**.

> Em versões mais antigas da plataforma este menu se chama `Cloud` → `Development`. É a
> mesma tela.

| Campo | O que preencher |
| --- | --- |
| Project Name | qualquer coisa (ex.: `CasaJarvis`) |
| Industry | `Smart Home` |
| Development Method | `Smart Home` ou `Custom` — os dois servem |
| **Data Center** | **o do seu app**, e não o seu país (veja abaixo) |

### ⚠️ Armadilha 1: o Data Center é o da CONTA DO APP

Este é o campo que mais custa tempo, porque errar nele **não dá erro de configuração** —
dá "permission deny" ou uma lista vazia, e nada na resposta aponta para cá.

A distribuição é por região da conta do Smart Life, não pelo país do projeto:

| Onde a conta do app foi criada | Data Center |
| --- | --- |
| **Américas (Brasil incluído)** | **Western America** |
| Europa, Oriente Médio, África | Central Europe |
| Índia | India |
| China | China |

Conta brasileira do Smart Life fica em **Western America**. Se você escolher outro, ao
tentar vincular a conta no Passo 4 a Tuya responde:

> *Data centers inconsistency, App account cannot be linked.*

<!-- IMAGEM: a tela "Create Cloud Project" com o campo Data Center aberto -->

### ⚠️ Armadilha 2: a cota é de 1 data center

O plano trial permite **um** data center. Se você já escolheu o errado, a lista aparece
toda cinza com esta mensagem:

> *Unable to select a new data center. You have currently selected 1 data center(s)… which
> reached the quota of IoT Core Trial Edition.*

**Não é preciso fazer upgrade.** É uma troca, não uma adição:

1. no campo **Data Center**, clique no **×** dentro da etiqueta do data center atual;
2. com o campo vazio, abra a lista de novo — o correto agora está selecionável;
3. escolha e salve.

<!-- IMAGEM: o dropdown de Data Center todo cinza, com a mensagem de cota -->

---

## Passo 3 — Assinar o IoT Core (e renovar quando expirar)

`Cloud` → `Cloud Services` → **IoT Core** → **Subscribe to Resource Pack**. É grátis.

Na aba **My Subscriptions**, confira a **Expiration Date**. O trial vale ~1 mês.

Se estiver vencido, a importação falha com:

> *IoT Core service subscription has expired.* (código `28841002`)

A solução está na mesma linha da tabela: o botão **Extend Trial Period**, ao lado da data.
Ele abre um formulário:

| Campo | O que responder |
| --- | --- |
| Extension Period | o período mais longo oferecido (normalmente 6 meses) |
| Developer Identity | `Personal` / `Individual Developer` |
| Estimated Number of Connected Devices | a **menor** faixa (1–10). Chutar alto faz o pedido parecer comercial |
| Contact Person | seu nome |
| Contact Information | o e-mail da sua conta da Tuya |
| Project Overview | veja abaixo |

O **Project Overview** é o único campo que alguém lê. Pedido vago costuma emperrar; seja
específico e sem pretensão. Em inglês:

```text
Personal home automation project, non-commercial.

I use the Tuya Cloud API only to retrieve the names and local keys of my own
devices (already paired in the Smart Life app). After that, all control happens
locally over the LAN on port 6668 — the cloud is not used for day-to-day operation.

Expected API usage: a handful of calls per month, only when I add or re-pair a
device. No resale, no third-party users, no data collection.
```

A aprovação leva de minutos a um dia. Confira também a aba **Authorized Projects**: o seu
projeto precisa estar listado ali.

<!-- IMAGEM: IoT Core > My Subscriptions, com a Expiration Date e o botão Extend Trial Period -->

---

## Passo 4 — Ligar a conta do app ao projeto

**É este passo que faz a API enxergar os seus aparelhos.** Sem ele, tudo o mais está certo
e a lista volta vazia.

`Project Management` → clique no **nome do projeto** → aba **Devices** → sub-aba **Link App
Account** → botão **Add App Account**.

Aparece um QR code. No celular: **Smart Life** → aba **Eu / Me** → ícone de **escanear** no
canto superior direito → leia o QR → confirme.

Quando perguntar o **Device Linking Method**, escolha **Automatic Link**. Com ela, todo
aparelho novo que você parear depois entra no projeto sozinho; com "Custom Link" você
volta nessa tela e marca um por um a cada lâmpada nova.

### ⚠️ Armadilha 3: o botão parecido que pede outro aplicativo

Na aba Devices existe também **Add Devices**, que abre uma janela chamada *"Add Devices
with Smart Industry App"* e pede a instalação de um aplicativo diferente.

**Não é esse.** Aquele é o fluxo industrial de cadastro de ativos.

| Botão | Para quê |
| --- | --- |
| **Add Devices** | cadastrar aparelhos direto no projeto, como ativos. Fluxo industrial, app próprio |
| **Link App Account** ✅ | dizer "esses aparelhos já são meus, no meu app pessoal" |

Só o **Link App Account** devolve a `local_key`.

Depois de vincular, a lista de aparelhos aparece com a coluna `Source` apontando para a sua
conta do Smart Life. `Device Permission: Read` é suficiente — a chave sai com permissão de
leitura, e o controle não passa pela nuvem.

<!-- IMAGEM: Devices > Link App Account com os aparelhos listados e a coluna Source -->

---

## Passo 5 — Liberar o seu IP

A Tuya bloqueia chamadas de endereços que não estejam numa lista, e a lista **nasce
vazia**. O sintoma é:

> *your ip(SEU.IP.PUB.LICO) don't have access to this API* (código `1114`)

Onde configurar: `Project Management` → o projeto → aba **Overview** → seção **Cloud
Authorization IP Allowlist** → link **Configure**.

> Em algumas versões o bloco está no fim da aba Overview; em outras, dentro da aba
> **Authorization**. Não está no botão "Edit" do cartão de autorização — aquele só edita
> nome e descrição.

No diálogo, **+ Add IP** → o endereço que a mensagem de erro mostrou.

Dois detalhes que fazem esse passo falhar de novo:

- **É o IP público do seu roteador**, o que a mensagem mostra. Não é o `192.168.x.x` deste
  PC — esse a Tuya nunca vê.
- **A lista é uma por data center.** Cadastrar no Central Europe não vale no Western
  America. Se você trocou o data center no Passo 2, a entrada antiga morreu junto.

O botão ao lado desliga a restrição inteira. Como essas credenciais só leem a lista dos
seus próprios aparelhos, desligar é uma troca defensável se o seu IP residencial muda com
frequência — mas fica a seu critério.

<!-- IMAGEM: o diálogo "Configure IP whitelist" com a aba do data center e o IP cadastrado -->

---

## Passo 6 — Configurar no Jarvis

As credenciais ficam em `Project Management` → o projeto → aba **Authorization** →
**Authorization Key**:

- **Access ID / Client ID**
- **Access Secret / Client Secret** (o olhinho revela)

No Jarvis:

1. barra inferior → ícone de **engrenagem** (Configurações)
2. quadro **Casa inteligente (Tuya)**
3. cole **Access ID** e **Access Secret**
4. **Data center**: o mesmo do Passo 2
5. **Salvar**

Depois, barra inferior → **Casa**. Não precisa clicar em mais nada: a varredura roda e a
importação sai junto. Em uns 10 segundos os cartões trocam os ids pelos nomes de verdade.

---

## Quando dá errado

A mensagem de erro do Jarvis diz **qual chamada** falhou, e isso separa causas que de
outro modo seriam idênticas. Um `1106` em `/v1.0/token` é projeto ou data center; o mesmo
`1106` em `/v1.0/devices/…` é a conta do app não vinculada.

| Código | O que diz | O que é de verdade |
| --- | --- | --- |
| `1114` | your ip don't have access | Passo 5. É o IP **público**, e a lista é por data center |
| `1106` no `/token` | permission deny | Data center errado, ou IoT Core não assinado no projeto |
| `1106` no `/devices` | permission deny | Passo 4: a conta do app não está vinculada |
| `1004` | sign invalid | Access Secret incompleto ou com espaço sobrando |
| `28841002` | subscription has expired | Passo 3: **Extend Trial Period** |
| `success` com lista vazia | — | Data center errado, ou conta do app não vinculada |

Erros do **controle** (depois da importação) são outra família:

| Sintoma | Causa provável |
| --- | --- |
| "não consegui abrir a resposta" | Chave velha: o aparelho foi pareado de novo. Reimporte |
| "não consegui falar com o aparelho" | Fora da tomada, ou o roteador trocou o IP dele. Rode a varredura |
| "recusou o aperto de mão" | Outro programa de casa inteligente conectado nele — aparelho Tuya aceita uma sessão por vez |

---

## Manutenção

Três coisas quebram sozinhas com o tempo, e nenhuma é culpa sua:

**O IP residencial muda.** Volta o `1114`. A mensagem sempre mostra o endereço do momento
— cadastre o novo, ou desligue a restrição.

**O trial expira.** Volta o `28841002`. Renove em **Extend Trial Period**. Enquanto isso,
**os aparelhos já importados continuam funcionando** — só o botão de importar para.

**Parear um aparelho de novo troca a chave dele.** Reimporte pelo painel Casa depois de
qualquer reset. O link "Reimportar nomes e chaves da nuvem" fica sempre disponível no
painel exatamente por isso.

---

## O que fica guardado, e onde

| Arquivo | Conteúdo |
| --- | --- |
| `%APPDATA%\com.jarvis.app\settings.json` | Access ID, Access Secret e data center |
| `%APPDATA%\com.jarvis.app\casa.json` | nome, `local_key`, categoria, último IP e protocolo de cada aparelho |

As duas chaves ficam em **texto puro**, como as demais credenciais do projeto. A
`local_key` só vale dentro da sua rede, e o estrago de vazá-la é alguém acender sua luz —
mas o lugar certo continua sendo o keyring do sistema, e isso está anotado como dívida no
código.
