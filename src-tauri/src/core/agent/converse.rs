//! Conversar, e transformar o que foi conversado em conhecimento.
//!
//! Três chamadas por turno de papo, e cada divisão foi paga com uma medição:
//!
//! 1. [`responder`] — fala com o usuário, com histórico e notas relevantes.
//! 2. [`destilar_assunto`] — decide SE a troca virou conhecimento, e sobre o quê.
//! 3. [`escrever_nota`] — reescreve a nota daquele assunto, inteira.
//!
//! **Por que não uma chamada só.** A primeira versão fazia responder e aprender juntos,
//! com o schema devolvendo `{resposta, lembrar}`. Três prompts depois, o veredito foi
//! que um 3B faz um dos dois trabalhos, nunca os dois:
//!
//! | prompt                                   | conversa                                 | aprendizado |
//! | ---------------------------------------- | ---------------------------------------- | ----------- |
//! | primeiro                                 | papagaia a memória em toda resposta      | 1 de 5      |
//! | "as notas são contexto, não assunto"     | boa                                      | 0 de 5      |
//! | com exemplos de extração                 | **"adotei um gato" → "Mora em Recife."** | 3 de 5      |
//!
//! A terceira linha encerrou a discussão: os exemplos do aprendizado vazaram para
//! dentro da resposta.
//!
//! **Por que 2 e 3 são separadas.** Porque a nota precisa ser um DOCUMENTO, e para
//! reescrever um documento o modelo tem que ver o que já estava lá. Só dá para buscar
//! o texto anterior depois de saber o assunto — daí a etapa 2 devolver só o tema.
//! Anexar numa chamada só produziria a pilha de frases coladas que a nota não deve ser.
//!
//! ponytail: as três são sequenciais. Ollama serializa pedidos do mesmo modelo por
//! padrão, então `tokio::join!` só ajudaria com `OLLAMA_NUM_PARALLEL > 1` — e com 4 GB
//! de VRAM isso disputa memória com o próprio modelo. Medir antes.

use serde::Deserialize;

use super::intent::pedir;
use super::AgentError;
use crate::config::{AppSettings, Persona};
use crate::core::chat::{ChatMessage, Role};

/// Quantos turnos anteriores vão para o prompt. Vinte mensagens de conversa curta dão
/// uns 1500 tokens — folgado nos 32K do modelo, e o que sai da janela vira resumo.
pub const JANELA: usize = 20;

/// Resposta ao usuário. Texto puro: não há nada para estruturar, e um schema aqui só
/// serviria para o modelo gastar tokens escrevendo `{"resposta": ...}`.
/// Recebe `settings` inteiro em vez de `url`, `model`, `nome` e `persona` soltos: eram
/// oito parâmetros, e o clippy do repo reprova acima de sete. É o mesmo formato de
/// `pesquisar_e_responder`, que já fazia assim.
pub async fn responder(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &str,
    historico: &[ChatMessage],
    frase: &str,
) -> Result<String, AgentError> {
    let (url, model) = (&settings.ollama_url, &settings.ollama_model);

    let mut mensagens = vec![serde_json::json!({
        "role": "system",
        "content": prompt_de_conversa(&settings.assistant_name, settings.persona, memoria),
    })];

    // O log de ações (`Role::System`) fica de fora: é registro para o usuário ler, e
    // dentro do prompt viraria ruído que o modelo tentaria comentar.
    for message in historico.iter().filter(|m| m.role != Role::System) {
        mensagens.push(serde_json::json!({
            "role": if message.role == Role::User { "user" } else { "assistant" },
            "content": message.content,
        }));
    }
    mensagens.push(serde_json::json!({ "role": "user", "content": frase }));

    let corpo = serde_json::json!({
        "model": model,
        "stream": false,
        "keep_alive": super::intent::KEEP_ALIVE,
        "options": {
            // Conversa a temperatura baixa fica robótica E repetitiva: o modelo
            // reescolhe a mesma frase para o mesmo contexto, e como as respostas dele
            // voltam no histórico, ele se copia em espiral.
            "temperature": 0.7,
            // A trava de verdade contra o laço. `repeat_last_n` cobre o histórico
            // recente, então "Como está sua semana?" duas vezes seguidas custa caro.
            "repeat_penalty": 1.2,
            "repeat_last_n": 256,
            "num_predict": 300,
        },
        "messages": mensagens,
    });

    let texto = pedir(http, url, model, &corpo).await?;

    // O modelo escreve `[[nome-da-nota]]` na FALA — herdou o hábito do prompt que
    // escreve as notas, onde o link é a sintaxe certa. Aqui é conversa: o link vira
    // ruído no meio da frase. Sem nota conhecida, tudo é órfão e tudo é desembrulhado.
    Ok(tirar_links_orfaos(texto.trim(), "", &[]))
}

#[derive(Deserialize)]
struct Destilacao {
    rende: bool,
    #[serde(default)]
    assunto: String,
}

/// Sobre o que foi a troca, se é que rendeu conhecimento. `None` é o caso comum —
/// cumprimento, agradecimento e desabafo não viram nota.
///
/// Só o ASSUNTO sai daqui, não o conteúdo. Quem escreve a nota é [`escrever_nota`],
/// numa segunda chamada que recebe o que já estava escrito — é isso que faz a nota
/// virar documento em vez de pilha de frases coladas.
pub async fn destilar_assunto(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    indice: &[String],
    troca: &str,
) -> Result<Option<String>, AgentError> {
    let conhecidos = if indice.is_empty() {
        "(a base ainda está vazia)".to_owned()
    } else {
        indice.join(", ")
    };

    let corpo = serde_json::json!({
        "model": model,
        "stream": false,
        "keep_alive": super::intent::KEEP_ALIVE,
        // `rende` ANTES de `assunto`: a grammar gera na ordem do schema, então ele se
        // compromete com o sim/não antes de ter a chance de inventar um tema para não
        // devolver vazio. Foi assim que a extração de fatos parou de alucinar.
        "format": {
            "type": "object",
            "properties": {
                "rende": { "type": "boolean" },
                "assunto": { "type": "string" }
            },
            "required": ["rende", "assunto"]
        },
        "options": { "temperature": 0, "num_predict": 120 },
        "messages": [
            { "role": "system", "content": format!("{PROMPT_DE_ASSUNTO}\n\nNOTAS QUE JÁ EXISTEM\n{conhecidos}") },
            { "role": "user", "content": troca },
        ],
    });

    let texto = pedir(http, url, model, &corpo).await?;
    let destilacao: Destilacao = serde_json::from_str(texto.trim())
        .map_err(|erro| AgentError::NaoEntendi(format!("{erro} — {texto}")))?;

    Ok(peneirar(destilacao))
}

/// `rende` manda, mesmo quando o assunto vem preenchido.
///
/// O modelo às vezes diz `false` e preenche o campo mesmo assim — foi medido no prompt
/// anterior, com "prefiro que você responda curto". Honrar o booleano perde uma nota;
/// honrar o campo aceita o tema que ele inventou para não devolver vazio. Base com
/// furo é melhor que base com lixo: o furo você conserta dizendo "lembra que...", que
/// passa pelo roteador e é confiável; o lixo você tem que ir catar na pasta.
fn peneirar(destilacao: Destilacao) -> Option<String> {
    if !destilacao.rende {
        return None;
    }

    let assunto = destilacao.assunto.trim();
    (!assunto.is_empty()).then(|| assunto.to_owned())
}

/// Reescreve a nota inteira sobre `assunto`, incorporando o que apareceu na conversa.
///
/// Reescrever e não anexar é o ponto: anexar produz uma lista de frases soltas na
/// ordem em que foram ditas, e o que se quer é um documento que alguém consiga ler.
/// O modelo recebe o que já estava escrito e devolve a versão nova.
pub async fn escrever_nota(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    assunto: &str,
    atual: &str,
    troca: &str,
    indice: &[String],
) -> Result<String, AgentError> {
    // Sem colchetes, sem parênteses e sem cara de nome: qualquer coisa parecida com um
    // rótulo aqui vira conteúdo da nota. Medido — "(nota nova, ainda não existe nada
    // escrito)" saiu do outro lado como `[[nota nova]]`, um link para o nada.
    let antes = if atual.trim().is_empty() {
        "ainda não há nada escrito nesta nota".to_owned()
    } else {
        atual.to_owned()
    };

    let outras = if indice.is_empty() {
        "(nenhuma)".to_owned()
    } else {
        indice.join(", ")
    };

    let corpo = serde_json::json!({
        "model": model,
        "stream": false,
        "keep_alive": super::intent::KEEP_ALIVE,
        "options": { "temperature": 0.2, "repeat_penalty": 1.15, "num_predict": 500 },
        "messages": [
            { "role": "system", "content": format!("{PROMPT_DE_NOTA}\n\nOUTRAS NOTAS QUE VOCÊ PODE CITAR COM [[nome]]\n{outras}") },
            { "role": "user", "content": format!(
                "Assunto da nota: {assunto}\n\nComo a nota está hoje:\n{antes}\n\nConversa nova:\n{troca}") },
        ],
    });

    let bruto = pedir(http, url, model, &corpo).await?;
    Ok(limpar_nota(&bruto, assunto, indice))
}

/// Tira o lixo que o modelo insiste em pôr na nota.
///
/// Tudo aqui foi visto de verdade na medição, apesar de regra explícita no prompt
/// mandando o contrário: abrir repetindo o nome do assunto, ecoar o rótulo do próprio
/// prompt (`ASSUNTO DA NOTA`), cuspir a lista de outras notas como primeiras linhas, e
/// terminar copiando a pergunta que o assistente fez na conversa.
///
/// Brigar com o prompt custa rodada e nunca fecha de vez; cortar em código fecha
/// sempre. A regra fica no prompt E o corte fica aqui — cinto e suspensório, porque
/// cada um pega o que o outro deixa passar.
fn limpar_nota(bruto: &str, assunto: &str, conhecidas: &[String]) -> String {
    let alvo = crate::core::memory::normalizar(assunto);
    let mut linhas: Vec<&str> = bruto.trim().lines().collect();

    // Cerca de código em volta da nota inteira.
    if linhas
        .first()
        .is_some_and(|l| l.trim_start().starts_with("```"))
    {
        linhas.remove(0);
        if linhas.last().is_some_and(|l| l.trim() == "```") {
            linhas.pop();
        }
    }

    while let Some(primeira) = linhas.first() {
        let limpa = primeira.trim().trim_start_matches('#').trim();
        // "Escolha do banco:" também é eco — só que com dois-pontos.
        let sem_dois_pontos = limpa.trim_end_matches(':').trim();
        let normalizada = crate::core::memory::normalizar(sem_dois_pontos);

        let e_lixo = limpa.is_empty()
            || normalizada == alvo
            // Eco da lista de notas vizinhas que o prompt mostrou.
            || conhecidas
                .iter()
                .any(|nome| crate::core::memory::normalizar(nome) == normalizada)
            // Rótulo vazado: linha curta, com letras, toda em caixa alta.
            || (limpa.len() <= 40
                && limpa.chars().any(char::is_alphabetic)
                && limpa.to_uppercase() == limpa);

        if e_lixo {
            linhas.remove(0);
        } else {
            break;
        }
    }

    let sem_perguntas = tirar_perguntas_do_fim(&linhas.join("\n"));
    tirar_links_orfaos(&sem_perguntas, assunto, conhecidas)
}

/// A nota afirma; pergunta no fim é a fala do assistente que vazou.
///
/// Em laço porque o modelo emenda duas — medido em `tony-stark.md`: "Como você deseja
/// explorar mais detalhes sobre ele? O que já conhece?". Tirar só a última deixaria a
/// primeira. Perguntas no MEIO ficam: um `## Em aberto` é conteúdo legítimo.
fn tirar_perguntas_do_fim(nota: &str) -> String {
    let mut atual = nota.trim();

    while atual.ends_with('?') {
        match atual[..atual.len() - 1].rfind(['.', '!', '?', '\n']) {
            Some(fim) => atual = atual[..=fim].trim_end(),
            // Sem frase anterior, a nota inteira era pergunta.
            None => return String::new(),
        }
    }

    atual.to_owned()
}

/// Apaga `[[links]]` que não apontam para nota nenhuma.
///
/// O modelo inventa alvo, e link para o nada é pior que texto puro: no Obsidian ele
/// aparece como nota fantasma, e na busca do próprio Jarvis ele nunca casa com nada.
/// Só o link some — o texto que estava dentro dele fica.
fn tirar_links_orfaos(nota: &str, assunto: &str, conhecidas: &[String]) -> String {
    let existe = |alvo: &str| {
        let alvo = crate::core::memory::slug(alvo);
        alvo == crate::core::memory::slug(assunto)
            || conhecidas
                .iter()
                .any(|nome| crate::core::memory::slug(nome) == alvo)
    };

    let mut saida = String::with_capacity(nota.len());
    let mut resto = nota;

    while let Some(abre) = resto.find("[[") {
        let (antes, daqui) = resto.split_at(abre);
        saida.push_str(antes);

        let miolo = &daqui[2..];
        let Some(fecha) = miolo.find("]]") else {
            // Colchete sem par: copia como está e para.
            saida.push_str(daqui);
            return saida;
        };

        let dentro = &miolo[..fecha];
        // `[[alvo|texto de exibição]]` — o alvo decide, o texto é o que sobra.
        let (alvo, exibido) = match dentro.split_once('|') {
            Some((alvo, exibido)) => (alvo.trim(), exibido.trim()),
            None => (dentro.trim(), dentro.trim()),
        };

        if existe(alvo) {
            saida.push_str(&daqui[..fecha + 4]);
        } else {
            saida.push_str(exibido);
        }

        resto = &miolo[fecha + 2..];
    }

    saida.push_str(resto);
    saida.trim().to_owned()
}

/// O system prompt da conversa.
///
/// **Ele lista o que o app sabe e o que NÃO sabe fazer, e isso não é enfeite.** Um caso
/// real: "salve essa música nas minhas curtidas" foi corretamente classificado como
/// conversa (não existe verbo para curtir música), e o modelo respondeu _"a música foi
/// adicionada à sua lista favorita na Spotify"_ — inventando uma ação que nunca
/// aconteceu, com o log do chat provando que nada foi executado.
///
/// Dizer "fiz" sem ter feito é o pior modo de falha de um assistente que mexe no PC:
/// some com a diferença entre funcionar e não funcionar. Por isso a proibição vem ANTES
/// das regras de estilo, e a lista de limitações é explícita — sem ela o modelo não tem
/// como saber o que este app faz, e preenche a lacuna com o que um assistente genérico
/// faria.
fn prompt_de_conversa(assistant_name: &str, persona: Persona, memoria: &str) -> String {
    let tom = persona.tom();

    let bloco = if memoria.trim().is_empty() {
        "Você ainda não sabe nada sobre ele.".to_owned()
    } else {
        memoria.to_owned()
    };

    // O modelo não tem relógio, e sem esta linha ele responde "Hoje é [data atual]" —
    // literalmente, com colchetes. Aconteceu de verdade.
    let agora = chrono::Local::now().format("%A, %d de %B de %Y, %H:%M");

    format!(
        "Você é o {assistant_name}, assistente pessoal. Roda local, sem internet.

SEU JEITO
{tom}

AGORA SÃO {agora}. Use isso quando ele perguntar a data ou a hora — nunca escreva um
espaço reservado como \"[data atual]\".

VOCÊ NÃO EXECUTOU NADA
Quem executa comandos é outra parte do sistema, e ela NÃO foi acionada — se você está
respondendo, é porque o pedido dele não virou ação nenhuma.
NUNCA diga que abriu, salvou, adicionou, curtiu, tocou, pausou, mandou ou mudou alguma
coisa. Não escreva \"pronto\", \"feito\", \"adicionei\" nem \"já está lá\".
Se ele pediu uma ação e você chegou aqui, a resposta certa é dizer que não consegue fazer
aquilo — não fingir que fez.

O QUE VOCÊ CONSEGUE FAZER (se ele pedir, ELE MESMO dispara; você não)
abrir site e programa; volume e mudo; pausar, próxima e anterior; tocar uma música no
Spotify; ligar e desligar a câmera; olhar pela câmera ou para a tela e dizer o que vê;
pesquisar na internet; lembrar e esquecer coisas.

O QUE VOCÊ NÃO CONSEGUE FAZER — diga isso na cara, sem rodeio
curtir, favoritar ou salvar música; mexer em playlist; ver o que está tocando por conta
própria; mandar mensagem ou e-mail; controlar luz e tomada; clicar em coisas na tela;
abrir, mover ou apagar arquivo.
Nesses casos: uma frase dizendo que não sabe fazer, e pare. Não invente um jeito, não
prometa fazer depois, e não sugira que ele tente de novo com outras palavras.

COMO RESPONDER
- Português, direto, no máximo 2 frases.
- Responda À MENSAGEM DELE. A memória abaixo é CONTEXTO SEU, não o assunto da
  conversa. Só cite uma nota se ele perguntar ou se vier mesmo ao caso.
  NUNCA recite a memória sem motivo.
- Se ele perguntar algo que está nas notas, responda com o que está lá.
- Se não souber, diga que não sabe. Não invente.
- Escreva como quem FALA. Nada de colchetes duplos, nada de markdown, nada de citar
  nome de arquivo. Isso é conversa, não anotação.
- NÃO ofereça o que ele não pediu, e não comente o que o log mostra que ele fez. Ele
  não perguntou seu histórico de uso.
- NUNCA repita uma pergunta ou uma frase que você já disse antes nesta conversa. Se
  ele não respondeu à sua pergunta, siga o assunto dele em vez de insistir na sua.
- Se ele reclamar de você (\"você entrou em loop\", \"não pedi nada\"), reconheça em
  meia frase e mude de assunto. Não peça desculpa duas vezes seguidas.

MEMÓRIA
{bloco}"
    )
}

/// O balanço dos exemplos importa mais que as regras — foi a lição que custou três
/// rodadas de medição no prompt anterior. Com mais negativos que positivos, o modelo
/// responde `false` para tudo; aqui são 6 positivos contra 4 negativos.
const PROMPT_DE_ASSUNTO: &str =
    "Você mantém uma base de conhecimento e lê a última troca de mensagens entre o \
usuário e o assistente. Decida se ela rende uma NOTA DE CONHECIMENTO.

Primeiro responda `rende`: true ou false.

`rende` = true quando a troca traz algo que valeria reler daqui a meses: um assunto
explicado, uma decisão, como algo funciona, um projeto, uma preferência firme, um
plano, um fato sobre a vida dele.

`rende` = false SÓ nestes casos: cumprimento, agradecimento, desabafo do momento,
reclamação sobre o assistente, ou um comando executado.

NA DÚVIDA, RESPONDA TRUE. Perder conhecimento é pior que ter uma nota a mais — a nota
você apaga, o que não foi anotado some. Se a troca menciona um projeto, uma tecnologia,
uma pessoa, um lugar, um plano ou a explicação de qualquer coisa, ela RENDE.

`assunto` = o TEMA, 2 a 4 palavras, minúsculas. É o nome do arquivo, então pense
\"sobre o que é esta nota\", não \"o que foi dito\".

Se o tema já estiver na lista de notas existentes, use EXATAMENTE aquele nome — é
assim que a nota cresce em vez de virar duplicata.

Exemplos:
usuário fala que trabalha na Noclaf como dev das 9 às 18  -> true,  \"trabalho\"
usuário explica que o projeto dele usa Tauri com Next     -> true,  \"projeto jarvis\"
assistente explica o que é um buraco negro                -> true,  \"buraco negro\"
usuário conta que adotou um gato chamado Bidu             -> true,  \"gato bidu\"
usuário decide usar SQLite em vez de JSON e diz por quê   -> true,  \"escolha do banco\"
usuário diz que prefere respostas curtas                  -> true,  \"preferencias\"
\"bom dia\" / \"obrigado\" / \"e aí?\"                          -> false, \"\"
\"tô com fome\" / \"tô cansado\"                              -> false, \"\"
\"você entrou em loop\" / \"não pedi nada\"                   -> false, \"\"
\"abre o spotify\" (comando executado)                      -> false, \"\"";

/// A regra que resolve o pedido: a nota é um DOCUMENTO sobre um assunto, não o
/// registro do que foi dito. Sem o "nada de ele perguntou / eu respondi", o modelo
/// escreve ata de reunião.
const PROMPT_DE_NOTA: &str =
    "Você mantém uma base de conhecimento em markdown, no estilo do Obsidian.

Sua tarefa: reescrever UMA nota inteira, incorporando o que apareceu na conversa nova.

COMO A NOTA DEVE SER:
- Um DOCUMENTO sobre o assunto, não o registro da conversa. NUNCA escreva \"ele
  perguntou\", \"eu respondi\", \"conversamos sobre\". Escreva o conhecimento direto,
  como se fosse uma página de wiki.
- Comece DIRETO com uma frase que diz o que é o assunto. NÃO repita o nome do assunto
  como título, NÃO abra com `#` e NUNCA escreva rótulos como \"Análise:\" ou \"Resumo:\".
- A nota AFIRMA. Nada de perguntas — se o assistente perguntou algo na conversa, isso
  não entra.
- Use `## Seções` quando houver mais de um aspecto. Com pouco conteúdo, não invente
  seção — um parágrafo basta.
- Cite outras notas com [[nome-da-nota]] quando o assunto encostar nelas.
- No máximo 15 linhas. Denso, sem enrolação.

O QUE PRESERVAR:
- MANTENHA tudo que já estava escrito, a menos que a conversa nova corrija.
- Se a conversa nova não acrescentar nada ao que já está lá, devolva a nota como está.
- NUNCA invente informação que não veio nem da nota atual nem da conversa.

Devolva SÓ o markdown da nota. Sem cercas de código, sem título com #, sem comentário
seu antes ou depois.";

/// Responde a pergunta CONVERSANDO, usando só o que a busca trouxe.
///
/// Duas coisas ao mesmo tempo, e as duas importam. O "só o que a busca trouxe" existe
/// porque um 3B perguntado direto sobre o mundo inventa data, nome e número com toda a
/// confiança — com os trechos na frente ele vira resumidor, tarefa que faz bem. E o
/// "conversando" existe porque a primeira versão devolvia um relatório com bloco de
/// fontes no fim, e ninguém quer conversar com um relatório.
///
/// As fontes não somem: elas vão para a nota que o agente grava na memória.
pub async fn responder_com_busca(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    assistant_name: &str,
    consulta: &str,
    achados: &[crate::core::search::Achado],
) -> Result<String, AgentError> {
    let fontes: Vec<String> = achados
        .iter()
        .enumerate()
        .map(|(i, achado)| {
            format!(
                "[{}] {}\n{}",
                i + 1,
                achado.titulo,
                // Trecho longo de mais come o contexto sem melhorar o resumo.
                achado.trecho.chars().take(700).collect::<String>()
            )
        })
        .collect();

    let corpo = serde_json::json!({
        "model": model,
        "stream": false,
        "keep_alive": super::intent::KEEP_ALIVE,
        // Baixa, mas não zero: a 0 ele fica com cara de verbete. O que segura a
        // invenção é a regra de não sair dos trechos, não a temperatura — a 0.2 ele
        // inventou um tempo de forno que não estava em trecho nenhum, e o que
        // resolveu foi a regra sobre passo a passo, logo abaixo.
        "options": { "temperature": 0.3, "repeat_penalty": 1.15, "num_predict": 300 },
        "messages": [
            { "role": "system", "content": format!(
                "Você é o {assistant_name}, assistente pessoal. Você acabou de dar uma \
                 olhada na internet para responder o que ele perguntou.\n\n\
                 FALE COMO GENTE:\n\
                 - 1 a 3 frases, em português, no tom de quem está conversando.\n\
                 - NUNCA diga \"segundo os resultados\", \"de acordo com as fontes\", \
                   \"os trechos indicam\" nem cite links. Ele não pediu um relatório, \
                   pediu uma resposta.\n\
                 - Nada de lista com marcadores e nada de título.\n\n\
                 MAS NÃO INVENTE:\n\
                 - Use SÓ o que está nos trechos abaixo. Data, número, quantidade, tempo \
                   e nome próprio que não estiverem escritos ali NÃO entram.\n\
                 - SÓ quando ele pedir INSTRUÇÕES (uma receita, um tutorial, como \
                   instalar algo) e os trechos tiverem apenas informação geral: conte o \
                   que eles têm e diga, naturalmente, que não achou o passo a passo. \
                   NUNCA invente etapas, quantidades ou tempos.\n\
                 - Essa regra acima vale só para pedido de instruções. Para \"o que é\", \
                   \"quem foi\", \"por que\" e afins, responda normalmente com o que os \
                   trechos trazem — não é para recusar por falta de passo a passo.\n\
                 - Se os trechos não respondem, diga que não achou. Sem rodeio e sem \
                   chute.") },
            { "role": "user", "content":
                format!("PERGUNTA\n{consulta}\n\nTRECHOS\n\n{}", fontes.join("\n\n")) },
        ],
    });

    Ok(pedir(http, url, model, &corpo).await?.trim().to_owned())
}

// Aqui morava `responder_sobre_o_que_viu`, que juntava a descrição da câmera com o que
// a busca trouxe. Ela existia porque a visão SEMPRE pesquisava — com a descrição inteira
// como consulta — e alguém precisava reconciliar as duas fontes, dizendo ao modelo que a
// imagem mandava e a busca era contexto.
//
// Não pesquisamos mais às cegas: a visão devolve `buscar` só quando a resposta está fora
// da imagem, e nesse caso a autoridade é a busca, não o contrário — que é exatamente o
// caso ("quando abrem os ingressos?") em que o prompt dela mandava ignorar o trecho que
// tem a resposta. Sem esse fluxo, ela virou código morto. Está no git.

/// O que vai para a memória depois de uma busca: o texto da fonte, não a paráfrase.
///
/// A paráfrase já foi dita no chat e custaria uma segunda chamada ao modelo. O trecho
/// original é mais fiel, e o link deixa a nota conferível quando você abrir a pasta.
pub fn nota_da_busca(achados: &[crate::core::search::Achado]) -> String {
    achados
        .iter()
        .map(|achado| {
            let corpo = achado.trecho.chars().take(900).collect::<String>();
            if achado.url.is_empty() {
                format!("**{}**\n{corpo}", achado.titulo)
            } else {
                format!("**{}**\n{corpo}\n<{}>", achado.titulo, achado.url)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Destila o que saiu da janela do prompt, para a conversa antiga não sumir sem
/// deixar rastro.
pub async fn resumir(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    mensagens: &[ChatMessage],
    resumo_anterior: &str,
) -> Result<String, AgentError> {
    let transcricao: Vec<String> = mensagens
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| {
            let quem = if m.role == Role::User { "Ele" } else { "Você" };
            format!("{quem}: {}", m.content)
        })
        .collect();

    let anterior = if resumo_anterior.trim().is_empty() {
        "(ainda não há resumo anterior)".to_owned()
    } else {
        resumo_anterior.to_owned()
    };

    let corpo = serde_json::json!({
        "model": model,
        "stream": false,
        "keep_alive": super::intent::KEEP_ALIVE,
        "options": { "temperature": 0.2, "num_predict": 400 },
        "messages": [
            { "role": "system", "content":
                "Você mantém um resumo curto de conversas antigas entre você e o usuário.\n\n\
                 Junte o resumo anterior com a transcrição nova em UM texto de no máximo 10 \
                 linhas, em português, em terceira pessoa. Guarde o que ainda vai importar \
                 daqui a meses: decisões, planos, assuntos recorrentes. Descarte troca de \
                 mensagens banal. Não invente nada que não esteja escrito." },
            { "role": "user", "content":
                format!("RESUMO ANTERIOR\n{anterior}\n\nTRANSCRIÇÃO NOVA\n{}", transcricao.join("\n")) },
        ],
    });

    Ok(pedir(http, url, model, &corpo).await?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O caso medido: o modelo diz `false` e preenche o assunto mesmo assim. Se o
    /// campo vencesse, a base encheria de nota sobre tema inventado — o pior desfecho,
    /// porque lixo se acumula e sobra para o usuário limpar na mão.
    #[test]
    fn rende_falso_descarta_o_assunto_mesmo_preenchido() {
        assert_eq!(
            peneirar(Destilacao {
                rende: false,
                assunto: "trabalho".to_owned(),
            }),
            None
        );
    }

    #[test]
    fn rende_verdadeiro_passa_e_apara_o_branco() {
        assert_eq!(
            peneirar(Destilacao {
                rende: true,
                assunto: "  gato bidu  ".to_owned(),
            }),
            Some("gato bidu".to_owned())
        );

        // `true` com assunto vazio é contradição do modelo: não vira nota sem nome.
        assert_eq!(
            peneirar(Destilacao {
                rende: true,
                assunto: "   ".to_owned(),
            }),
            None
        );
    }

    /// `assunto` ausente é resposta válida do modelo — o `default` do serde é o que
    /// impede isso de virar erro de parse e sumir com a conversa inteira.
    #[test]
    fn aceita_resposta_sem_o_assunto() {
        let destilacao: Destilacao = serde_json::from_str(r#"{"rende":false}"#).expect("parseia");
        assert_eq!(peneirar(destilacao), None);
    }

    /// Os três lixos que o modelo produziu de verdade na medição: eco do assunto como
    /// título, rótulo do prompt vazado, e cerca de código em volta de tudo.
    #[test]
    fn a_nota_nao_comeca_com_eco() {
        let vizinhas = ["trabalho".to_owned(), "gato bidu".to_owned()];

        assert_eq!(
            limpar_nota(
                "projeto jarvis\n\nUsa Tauri com Next.",
                "projeto jarvis",
                &[]
            ),
            "Usa Tauri com Next."
        );
        assert_eq!(
            limpar_nota("## Projeto Jarvis\nUsa Tauri.", "projeto jarvis", &[]),
            "Usa Tauri."
        );
        assert_eq!(
            limpar_nota(
                "ASSUNTO DA NOTA\nescolha do banco\n\nMarkdown, não SQLite.",
                "escolha do banco",
                &[]
            ),
            "Markdown, não SQLite."
        );
        assert_eq!(
            limpar_nota("```markdown\nUsa Tauri.\n```", "projeto jarvis", &[]),
            "Usa Tauri."
        );
        // Eco com dois-pontos e eco da lista de notas vizinhas — os dois medidos.
        assert_eq!(
            limpar_nota("Escolha do banco:\nMarkdown.", "escolha do banco", &[]),
            "Markdown."
        );
        assert_eq!(
            limpar_nota(
                "trabalho\nprojeto jarvis\n\nUsa Tauri.",
                "projeto jarvis",
                &vizinhas
            ),
            "Usa Tauri."
        );
    }

    /// A limpeza não pode comer conteúdo legítimo.
    #[test]
    fn a_limpeza_nao_come_o_conteudo() {
        assert_eq!(
            limpar_nota(
                "Usa Tauri com Next.\n## Stack\nRust no backend.",
                "projeto jarvis",
                &[]
            ),
            "Usa Tauri com Next.\n## Stack\nRust no backend."
        );
        // Frase longa em caixa alta não é rótulo — o teto de 40 chars é o que separa.
        let gritada = "ESTE PROJETO INTEIRO FOI ESCRITO NUMA MADRUGADA SÓ, SEM DORMIR.";
        assert_eq!(limpar_nota(gritada, "projeto jarvis", &[]), gritada);
    }

    /// Medido: a nota sobre trabalho terminou com "Há alguma preocupação com o ritmo
    /// de trabalho?" — a pergunta que o assistente fez na conversa, copiada para
    /// dentro do conhecimento.
    #[test]
    fn tira_as_perguntas_que_o_assistente_deixou_no_fim() {
        assert_eq!(
            tirar_perguntas_do_fim("Dev na Noclaf, das 9 às 18. Há preocupação com o ritmo?"),
            "Dev na Noclaf, das 9 às 18."
        );
        // Duas emendadas — o caso real de `tony-stark.md`.
        assert_eq!(
            tirar_perguntas_do_fim(
                "Tony Stark é uma figura da cultura pop. Como quer explorar? O que já conhece?"
            ),
            "Tony Stark é uma figura da cultura pop."
        );
        // Pergunta no MEIO é conteúdo: um "## Em aberto" continua de pé.
        let em_aberto = "Usa Tauri.\n## Em aberto\nMigrar para SQLite? Ainda sem decisão.";
        assert_eq!(tirar_perguntas_do_fim(em_aberto), em_aberto);
        // Nota que é só pergunta não vira nota.
        assert_eq!(tirar_perguntas_do_fim("Tudo bem por aí?"), "");
    }

    /// Link para nota que não existe vira nota fantasma no Obsidian e nunca casa na
    /// busca. O caso medido foi `[[nota nova]]`, que era o texto do meu placeholder.
    #[test]
    fn apaga_link_que_nao_aponta_para_nota_nenhuma() {
        let conhecidas = ["academia".to_owned()];

        assert_eq!(
            limpar_nota("Treina na [[academia]] toda manhã.", "rotina", &conhecidas),
            "Treina na [[academia]] toda manhã."
        );
        assert_eq!(
            limpar_nota(
                "Figura da cultura pop. [[nota nova]]",
                "tony stark",
                &conhecidas
            ),
            "Figura da cultura pop. nota nova"
        );
        // `[[alvo|texto]]` órfão deixa o texto de exibição, não o alvo.
        assert_eq!(
            limpar_nota("Usa [[tauri-v2|Tauri]] no backend.", "projeto", &conhecidas),
            "Usa Tauri no backend."
        );
        // Link para a própria nota é válido — ela existe, por definição.
        assert_eq!(
            limpar_nota("Ver também [[rotina]].", "rotina", &[]),
            "Ver também [[rotina]]."
        );
    }

    /// Na CONVERSA nenhum link sobrevive: o modelo herdou do prompt de notas o hábito
    /// de escrever `[[assim]]`, e no meio de uma frase falada isso é só ruído.
    #[test]
    fn a_fala_nao_leva_colchetes() {
        assert_eq!(
            tirar_links_orfaos(
                "Talvez [[musica-charlie-brow-jr]]? Ou [[rotinas-observadas]].",
                "",
                &[]
            ),
            "Talvez musica-charlie-brow-jr? Ou rotinas-observadas."
        );
    }
}
