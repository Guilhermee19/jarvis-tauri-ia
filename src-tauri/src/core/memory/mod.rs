//! A memória do Jarvis: uma pasta de markdown no formato do Obsidian.
//!
//! ```text
//! memoria/
//! ├── MEMORIA.md          índice — uma linha por nota, regravado a cada mudança
//! ├── notas/*.md          O CONHECIMENTO, com frontmatter e [[links]]
//! ├── historico.jsonl     estado da UI (não é conhecimento; o Obsidian ignora)
//! ├── acoes.jsonl         log cru de ações, matéria-prima das rotinas
//! └── estado.json         até onde a conversa já virou resumo
//! ```
//!
//! **A memória NÃO é o transcrito da conversa.** É uma base de conhecimento: cada nota
//! é um documento sobre um assunto, que cresce quando o assunto volta a aparecer. O que
//! foi dito literalmente fica em `historico.jsonl`, que existe só para a UI redesenhar
//! as bolhas — reconstruir id, papel e timestamp parseando markdown seria frágil sem
//! ganhar nada. Quem transforma conversa em nota é `core::agent::converse`.
//!
//! **Por que markdown e não um banco.** Porque assim a memória tem dois donos. Você
//! abre a pasta no Obsidian, lê o que ele entendeu, corrige o que ficou errado e apaga
//! o que não quer que ele saiba — e ainda entra no git, então dá para ver a base
//! crescer no diff. Um SQLite seria um blob opaco que só o modelo lê. O `[[link]]` é o
//! que faz notas soltas virarem grafo, e é o modelo que os escreve: os prompts recebem
//! o índice e pedem que ele ligue o que citar.
//!
//! Nada aqui derruba o app. Pasta sem permissão, disco cheio ou arquivo corrompido
//! degrada para "não lembro de nada" — que é pior que lembrar, e muito melhor que
//! não abrir.

mod busca;
pub mod grafo;
mod nota;
mod rotinas;

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use chrono::Local;
use serde::{Deserialize, Serialize};

pub use nota::{slug, Nota, Tipo};
pub use rotinas::Rotina;

use crate::core::chat::ChatMessage;
use crate::core::lock;

const INDICE: &str = "MEMORIA.md";
const PASTA_NOTAS: &str = "notas";
const ARQUIVO_HISTORICO: &str = "historico.jsonl";
const ARQUIVO_ACOES: &str = "acoes.jsonl";
const ARQUIVO_MARCADOR: &str = "estado.json";

/// Nome fixo porque a nota é regravada, não acumulada.
const NOTA_DE_ROTINAS: &str = "rotinas-observadas";

/// Teto do histórico em disco. Conversa de assistente é longa e repetitiva, e o que
/// interessava já foi destilado em nota.
const LIMITE_HISTORICO: usize = 2_000;

/// Quantas notas cabem no prompt de conversa sem espremer o resto.
const NOTAS_NO_PROMPT: usize = 8;

/// Uma ação executada — matéria-prima das [`rotinas`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acao {
    /// Epoch em milissegundos.
    pub quando: i64,
    pub acao: String,
    pub alvo: String,
    pub ok: bool,
}

/// Quanto do histórico já virou resumo. Um número só, mas precisa sobreviver ao
/// fechamento do app.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
struct Marcador {
    resumidas: usize,
}

pub struct Memoria {
    raiz: PathBuf,
    historico: Mutex<Vec<ChatMessage>>,
    /// Cópia quente das notas. Recarregada do disco a cada escrita nossa — e o
    /// `recarregar` público existe para quando VOCÊ edita a pasta no Obsidian.
    notas: Mutex<Vec<Nota>>,
    resumidas: Mutex<usize>,
    /// Quantas vezes as notas mudaram por conta NOSSA — uma escrita, uma remoção.
    ///
    /// Serve para a fronteira saber se vale avisar a tela: o grafo do conhecimento se
    /// redesenha sozinho quando uma conversa vira nota, e sem este contador o aviso teria
    /// que sair a cada mensagem, inclusive nas que não aprenderam nada.
    ///
    /// Não conta o que muda no disco por fora (você editando no Obsidian). Para isso
    /// existe o botão de atualizar, e um vigia de pasta seria outro assunto.
    versao: AtomicU64,
}

impl Memoria {
    pub fn new(raiz: &Path) -> Self {
        let _ = std::fs::create_dir_all(raiz.join(PASTA_NOTAS));

        let marcador: Marcador = ler_json(&raiz.join(ARQUIVO_MARCADOR));

        let memoria = Self {
            historico: Mutex::new(ler_jsonl(&raiz.join(ARQUIVO_HISTORICO))),
            notas: Mutex::new(carregar_notas(&raiz.join(PASTA_NOTAS))),
            resumidas: Mutex::new(marcador.resumidas),
            raiz: raiz.to_owned(),
            versao: AtomicU64::new(0),
        };

        memoria.escrever_indice();
        memoria
    }

    // ---- histórico da conversa (estado da UI) ----------------------------

    pub fn historico(&self) -> Vec<ChatMessage> {
        lock(&self.historico).clone()
    }

    /// A cauda que vai para o prompt de conversa.
    pub fn recentes(&self, quantas: usize) -> Vec<ChatMessage> {
        let historico = lock(&self.historico);
        let comeco = historico.len().saturating_sub(quantas);
        historico[comeco..].to_vec()
    }

    /// Mensagens que já saíram da janela do prompt e ainda não entraram no resumo.
    ///
    /// Devolve no máximo `lote` por vez: sem esse teto, uma conversa longa mandaria
    /// mil mensagens de uma vez para o modelo resumir.
    pub fn pendentes_de_resumo(&self, janela: usize, lote: usize) -> Vec<ChatMessage> {
        let historico = lock(&self.historico);
        let fim = historico.len().saturating_sub(janela);
        let comeco = lock(&self.resumidas).min(fim);

        historico[comeco..fim].iter().take(lote).cloned().collect()
    }

    /// Anda o marcador. Persistido porque sem isso reabrir o app faria o resumo
    /// reprocessar tudo de novo e duplicar o conteúdo destilado.
    pub fn marcar_resumidas(&self, quantas: usize) {
        let mut resumidas = lock(&self.resumidas);
        *resumidas += quantas;

        let marcador = Marcador {
            resumidas: *resumidas,
        };
        if let Ok(bytes) = serde_json::to_vec(&marcador) {
            let _ = std::fs::write(self.raiz.join(ARQUIVO_MARCADOR), bytes);
        }
    }

    pub fn push_message(&self, message: ChatMessage) {
        anexar(&self.raiz.join(ARQUIVO_HISTORICO), &message);

        let mut historico = lock(&self.historico);
        historico.push(message);

        if historico.len() > LIMITE_HISTORICO {
            let sobra = historico.len() - LIMITE_HISTORICO;
            historico.drain(..sobra);
            reescrever_jsonl(&self.raiz.join(ARQUIVO_HISTORICO), &historico);
        }
    }

    pub fn limpar_historico(&self) {
        lock(&self.historico).clear();
        let _ = std::fs::remove_file(self.raiz.join(ARQUIVO_HISTORICO));

        // O marcador precisa voltar a zero junto: apontando para o meio de um
        // histórico que não existe mais, ele bloquearia todo resumo futuro.
        *lock(&self.resumidas) = 0;
        let _ = std::fs::remove_file(self.raiz.join(ARQUIVO_MARCADOR));
    }

    // ---- notas -----------------------------------------------------------

    pub fn notas(&self) -> Vec<Nota> {
        lock(&self.notas).clone()
    }

    /// Relê a pasta. Chamado antes de montar o contexto, porque você pode ter editado
    /// as notas no Obsidian enquanto o app estava aberto.
    pub fn recarregar(&self) {
        *lock(&self.notas) = carregar_notas(&self.raiz.join(PASTA_NOTAS));
    }

    /// O bloco de memória que entra no prompt de conversa: o índice inteiro (barato, e
    /// é o que permite o modelo escrever `[[links]]` para notas que ele não recebeu) e
    /// o corpo das notas relevantes.
    pub fn contexto(&self, frase: &str) -> String {
        let notas = lock(&self.notas);
        if notas.is_empty() {
            return String::new();
        }

        let indice: Vec<String> = notas.iter().map(Nota::linha_do_indice).collect();
        let relevantes = busca::relevantes(&notas, frase, NOTAS_NO_PROMPT);

        let corpos: Vec<String> = relevantes
            .iter()
            .map(|nota| format!("### {}\n{}", nota.nome, nota.corpo))
            .collect();

        // **O cabeçalho diz de onde as notas vieram, e isso decide a resposta.** Sem
        // casamento nenhum, `busca::relevantes` cai nas mais recentes (de propósito), e o
        // rótulo antigo — "as mais relevantes agora" — chamava de relevante o que tinha
        // sido escolhido por DATA. O modelo lia aquilo como "então é isto que eu sei sobre
        // o assunto", não achava a resposta lá dentro, e completava de cabeça.
        //
        // Agora ele sabe a diferença entre "a memória respondeu" e "a memória não tem
        // isto" — que é exatamente a bifurcação que o `prompt_de_conversa` usa para
        // decidir entre responder e mandar pesquisar.
        let cabecalho = if busca::casou(&notas, frase) {
            "Conteúdo das notas que CASARAM com o que ele disse:"
        } else {
            "NENHUMA NOTA CASOU com o que ele disse. As de baixo são só as mais recentes, \
             para você saber o que existe — NÃO são a resposta:"
        };

        format!(
            "Notas que existem na sua memória:\n{}\n\n{cabecalho}\n\n{}",
            indice.join("\n"),
            corpos.join("\n\n")
        )
    }

    /// Se a memória tem alguma nota que fale DE VERDADE sobre isto.
    ///
    /// **É a pergunta que o [`Self::contexto`] não consegue responder**, e por desenho:
    /// ele sempre devolve texto, porque o fallback do `busca::relevantes` entrega as mais
    /// recentes quando nada casa. Isso está certo para MONTAR PROMPT — memória que só
    /// aparece pelo nome exato não parece memória — e está errado para DECIDIR, que é
    /// para o que este método serve.
    ///
    /// Quem decide é o `agent::deve_estudar`: sem nota sobre o assunto, uma pergunta
    /// sobre o mundo vai para a busca antes de virar resposta.
    ///
    /// Custa um `contains` por termo por nota, sem ida ao modelo — é o que permite
    /// pendurar isto no caminho de toda mensagem sem o "bom dia" ficar mais lento.
    ///
    /// Memória vazia não cobre nada, e é o comportamento certo: numa instalação nova
    /// tudo é assunto novo, e é exatamente aí que ele mais precisa ir aprender.
    pub fn cobre(&self, frase: &str) -> bool {
        busca::casou(&lock(&self.notas), frase)
    }

    /// `apelido → alvo`, para o prompt do roteador. É o que faz "abre meu jogo" passar
    /// a funcionar depois de ensinado uma vez.
    pub fn apelidos(&self) -> BTreeMap<String, String> {
        lock(&self.notas)
            .iter()
            .filter(|nota| nota.tipo == Tipo::Apelido)
            .filter_map(|nota| {
                let (apelido, alvo) = nota.corpo.split_once('=')?;
                Some((apelido.trim().to_lowercase(), alvo.trim().to_owned()))
            })
            .collect()
    }

    /// Grava um fato. Assunto que já existe **recebe o fato no fim** em vez de ser
    /// substituído — é assim que uma nota cresce em vez de se sobrescrever.
    ///
    /// `false` quando aquilo já estava escrito, para quem chamou não anunciar duas
    /// vezes que guardou a mesma coisa.
    pub fn lembrar(&self, assunto: &str, fato: &str) -> bool {
        let fato = fato.trim();
        if fato.is_empty() {
            return false;
        }

        let nome = nota::slug(assunto);
        let mut notas = lock(&self.notas);

        match notas.iter_mut().find(|nota| nota.nome == nome) {
            Some(existente) => {
                if normalizar(&existente.corpo).contains(&normalizar(fato)) {
                    return false;
                }
                existente.corpo = format!("{}\n{fato}", existente.corpo.trim());
                existente.atualizado = hoje();
                self.gravar_nota(existente);
            }
            None => {
                let nova = Nota::nova(assunto, Tipo::Fato, fato, &hoje());
                self.gravar_nota(&nova);
                notas.push(nova);
            }
        }

        drop(notas);
        self.escrever_indice();
        true
    }

    pub fn apelidar(&self, apelido: &str, alvo: &str) -> bool {
        let apelido = apelido.trim();
        let alvo = alvo.trim();
        if apelido.is_empty() || alvo.is_empty() {
            return false;
        }

        let corpo = format!("{} = {alvo}", apelido.to_lowercase());
        let nova = Nota::nova(apelido, Tipo::Apelido, &corpo, &hoje());

        let mut notas = lock(&self.notas);
        if let Some(pos) = notas.iter().position(|nota| nota.nome == nova.nome) {
            if notas[pos].corpo == corpo {
                return false;
            }
            notas[pos] = nova.clone();
        } else {
            notas.push(nova.clone());
        }
        drop(notas);

        self.gravar_nota(&nova);
        self.escrever_indice();
        true
    }

    /// Apaga notas cujo nome ou corpo cite `termo`. Casa por trecho porque ninguém
    /// repete o fato palavra por palavra para mandar esquecer — diz "esquece a
    /// academia". Devolve os nomes apagados, para o log poder mostrar o que saiu.
    pub fn esquecer(&self, termo: &str) -> Vec<String> {
        let alvo = normalizar(termo);
        if alvo.is_empty() {
            return Vec::new();
        }

        let mut notas = lock(&self.notas);
        let mut apagadas = Vec::new();

        notas.retain(|nota| {
            // A nota de rotinas é derivada do log: apagar só a faria voltar na próxima
            // regravação, e o usuário acharia que o "esquece" não funcionou.
            let bate = nota.nome != NOTA_DE_ROTINAS
                && (normalizar(&nota.nome.replace('-', " ")).contains(&alvo)
                    || normalizar(&nota.corpo).contains(&alvo));

            if bate {
                apagadas.push(nota.nome.clone());
                let _ = std::fs::remove_file(self.caminho_da_nota(&nota.nome));
                // Apagar também é mudança: "esquece X" tem que tirar o nó do grafo na hora.
                self.versao.fetch_add(1, Ordering::Relaxed);
            }
            !bate
        });
        drop(notas);

        if !apagadas.is_empty() {
            self.escrever_indice();
        }
        apagadas
    }

    pub fn guardar_resumo(&self, assunto: &str, texto: &str) {
        self.substituir(assunto, Tipo::Resumo, texto);
    }

    /// Os nomes das notas, para o modelo saber o que já existe — é o que faz ele
    /// REUSAR um assunto em vez de criar `trabalho`, `meu-trabalho` e `emprego`.
    pub fn nomes_das_notas(&self) -> Vec<String> {
        lock(&self.notas)
            .iter()
            .map(|nota| nota.nome.clone())
            .collect()
    }

    /// O corpo de uma nota pelo assunto (o slug é calculado aqui). Vazio quando ela
    /// ainda não existe, que é o sinal de "nota nova" para quem vai escrevê-la.
    pub fn corpo_da_nota(&self, assunto: &str) -> String {
        let nome = nota::slug(assunto);
        lock(&self.notas)
            .iter()
            .find(|nota| nota.nome == nome)
            .map(|nota| nota.corpo.clone())
            .unwrap_or_default()
    }

    /// Reescreve o corpo de uma nota que já existe, **preservando o tipo dela**.
    ///
    /// É a edição à mão, feita da janelinha do Conhecimento. Existe porque o que ele
    /// aprende sozinho às vezes está errado, e corrigir tinha que passar por achar o
    /// arquivo no disco — o que só serve para quem sabe onde a pasta mora.
    ///
    /// **O tipo não muda.** Uma nota `aprendido` corrigida continua `aprendido`: o tipo diz
    /// de ONDE o conhecimento veio, e passar a mão no texto não reescreve a origem dele. É
    /// por isso que isto não é um `escrever_conhecimento`, que carimba tudo como `fato`.
    ///
    /// Corpo vazio não apaga a nota — apagar tem botão próprio, e um `Ctrl+A Delete` sem
    /// querer não pode virar exclusão silenciosa. Devolve `false` quando a nota sumiu do
    /// disco entre a tela abrir e o salvar.
    pub fn reescrever(&self, nome: &str, corpo: &str) -> bool {
        let corpo = corpo.trim();
        if corpo.is_empty() {
            return false;
        }

        let nome = nota::slug(nome);
        let mut notas = lock(&self.notas);

        let Some(alvo) = notas.iter_mut().find(|nota| nota.nome == nome) else {
            return false;
        };

        alvo.corpo = corpo.to_owned();
        alvo.atualizado = hoje();
        let nova = alvo.clone();
        drop(notas);

        self.gravar_nota(&nova);
        self.escrever_indice();
        true
    }

    /// Apaga UMA nota, pelo nome exato.
    ///
    /// Irmão do [`Self::esquecer`] e diferente dele no que importa: aquele casa por termo e
    /// pode levar cinco notas junto (é o "esquece academia" falado), este leva a que está
    /// aberta na tela. Para um botão de apagar, casar por termo seria uma armadilha.
    ///
    /// A nota de rotinas passa igual, ao contrário do que acontece no `esquecer`: ali ela é
    /// protegida porque some e volta, e o usuário culparia o comando errado. Aqui ele
    /// apontou para um arquivo específico e mandou apagar — o arquivo some. Que o log de
    /// ações a reconstrua depois é o comportamento dela, não uma recusa nossa.
    pub fn apagar_nota(&self, nome: &str) -> bool {
        let nome = nota::slug(nome);
        let mut notas = lock(&self.notas);

        let Some(pos) = notas.iter().position(|nota| nota.nome == nome) else {
            return false;
        };

        notas.remove(pos);
        drop(notas);

        let _ = std::fs::remove_file(self.caminho_da_nota(&nome));
        self.versao.fetch_add(1, Ordering::Relaxed);
        self.escrever_indice();
        true
    }

    /// Grava a nota de conhecimento destilada de uma conversa, reescrevendo por
    /// inteiro. É a diferença entre um documento e uma pilha de frases coladas na
    /// ordem em que foram ditas.
    pub fn escrever_conhecimento(&self, assunto: &str, corpo: &str) {
        self.substituir(assunto, Tipo::Fato, corpo);
    }

    /// Guarda conhecimento do MUNDO trazido por uma busca — é o que faz o Jarvis não
    /// precisar pesquisar duas vezes a mesma coisa, e o que enche a pasta de assuntos
    /// além de você.
    ///
    /// Substitui em vez de acumular: busca é retrato, e o retrato novo é o que vale.
    pub fn aprender(&self, assunto: &str, texto: &str) {
        self.substituir(assunto, Tipo::Aprendido, texto);
    }

    fn substituir(&self, assunto: &str, tipo: Tipo, texto: &str) {
        let texto = texto.trim();
        if texto.is_empty() {
            return;
        }

        let nova = Nota::nova(assunto, tipo, texto, &hoje());

        let mut notas = lock(&self.notas);
        if let Some(pos) = notas.iter().position(|nota| nota.nome == nova.nome) {
            notas[pos] = nova.clone();
        } else {
            notas.push(nova.clone());
        }
        drop(notas);

        self.gravar_nota(&nova);
        self.escrever_indice();
    }

    fn caminho_da_nota(&self, nome: &str) -> PathBuf {
        self.raiz.join(PASTA_NOTAS).join(format!("{nome}.md"))
    }

    /// **O choque de todas as escritas de nota**, e por isso o lugar do contador: são cinco
    /// caminhos que gravam (aprender, escrever conhecimento, anotar fato, rotinas, resumo),
    /// e marcar cada um seria esquecer o sexto no dia em que ele aparecesse.
    fn gravar_nota(&self, nota: &Nota) {
        let caminho = self.caminho_da_nota(&nota.nome);
        if let Err(erro) = std::fs::write(&caminho, nota.para_markdown()) {
            eprintln!("[jarvis] não gravei {}: {erro}", caminho.display());
        }

        self.versao.fetch_add(1, Ordering::Relaxed);
    }

    /// Quantas vezes as notas mudaram nesta sessão. Só é útil comparada consigo mesma:
    /// mudou o número, mudou a pasta — e é isso que faz o grafo do conhecimento se
    /// redesenhar sozinho no meio da conversa.
    pub fn versao(&self) -> u64 {
        self.versao.load(Ordering::Relaxed)
    }

    /// Regravado inteiro a cada mudança. São dezenas de linhas — reconciliar seria
    /// mais código para o mesmo resultado, com chance de dessincronizar.
    fn escrever_indice(&self) {
        let notas = lock(&self.notas);
        let mut linhas: Vec<String> = notas.iter().map(Nota::linha_do_indice).collect();
        linhas.sort();

        let conteudo = format!(
            "# Memória do Jarvis\n\nÍndice gerado automaticamente — uma linha por nota em \
             `notas/`. Pode editar as notas à vontade (inclusive no Obsidian); este arquivo \
             é regravado.\n\n{}\n",
            linhas.join("\n")
        );

        if let Err(erro) = std::fs::write(self.raiz.join(INDICE), conteudo) {
            eprintln!("[jarvis] não gravei o índice: {erro}");
        }
    }

    // ---- ações e rotinas -------------------------------------------------

    pub fn registrar_acao(&self, acao: Acao) {
        anexar(&self.raiz.join(ARQUIVO_ACOES), &acao);
    }

    pub fn rotinas(&self) -> Vec<Rotina> {
        rotinas::agregar(&ler_jsonl::<Acao>(&self.raiz.join(ARQUIVO_ACOES)))
    }

    /// Reescreve `rotinas-observadas.md` a partir do log. Barato o bastante para rodar
    /// depois de cada ação executada.
    pub fn atualizar_rotinas(&self) {
        let Some(corpo) = rotinas::como_nota(&self.rotinas()) else {
            return;
        };

        let nova = Nota::nova(NOTA_DE_ROTINAS, Tipo::Rotina, &corpo, &hoje());

        let mut notas = lock(&self.notas);
        match notas.iter().position(|nota| nota.nome == nova.nome) {
            Some(pos) if notas[pos].corpo == nova.corpo => return,
            Some(pos) => notas[pos] = nova.clone(),
            None => notas.push(nova.clone()),
        }
        drop(notas);

        self.gravar_nota(&nova);
        self.escrever_indice();
    }
}

fn hoje() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Minúsculas, sem acento, só letras/números separados por um espaço. Usado para
/// comparar texto que humano e modelo escrevem de jeitos diferentes.
pub fn normalizar(texto: &str) -> String {
    let sem_acento: String = texto
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            outro => outro,
        })
        .collect();

    sem_acento
        .split(|c: char| !c.is_alphanumeric())
        .filter(|palavra| !palavra.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn carregar_notas(pasta: &Path) -> Vec<Nota> {
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return Vec::new();
    };

    let mut notas: Vec<Nota> = entradas
        .filter_map(Result::ok)
        .filter(|entrada| entrada.path().extension().is_some_and(|ext| ext == "md"))
        .filter_map(|entrada| {
            let caminho = entrada.path();
            let nome = caminho.file_stem()?.to_string_lossy().into_owned();
            let texto = std::fs::read_to_string(&caminho).ok()?;
            Some(Nota::do_markdown(&nome, &texto))
        })
        .collect();

    notas.sort_by(|a, b| a.nome.cmp(&b.nome));
    notas
}

fn ler_json<T: Default + for<'de> Deserialize<'de>>(caminho: &Path) -> T {
    std::fs::read(caminho)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Linha quebrada é PULADA, não fatal: um desligamento no meio de um append deixa meia
/// linha no fim do arquivo, e perder essa linha é melhor que perder o resto.
fn ler_jsonl<T: for<'de> Deserialize<'de>>(caminho: &Path) -> Vec<T> {
    let Ok(arquivo) = File::open(caminho) else {
        return Vec::new();
    };

    BufReader::new(arquivo)
        .lines()
        .map_while(Result::ok)
        .filter(|linha| !linha.trim().is_empty())
        .filter_map(|linha| serde_json::from_str(&linha).ok())
        .collect()
}

fn anexar<T: Serialize>(caminho: &Path, item: &T) {
    let Ok(linha) = serde_json::to_string(item) else {
        return;
    };
    anexar_texto(caminho, &format!("{linha}\n"));
}

fn anexar_texto(caminho: &Path, texto: &str) {
    let escrita = OpenOptions::new()
        .create(true)
        .append(true)
        .open(caminho)
        .and_then(|mut arquivo| arquivo.write_all(texto.as_bytes()));

    if let Err(erro) = escrita {
        eprintln!("[jarvis] não anexei em {}: {erro}", caminho.display());
    }
}

fn reescrever_jsonl<T: Serialize>(caminho: &Path, itens: &[T]) {
    let mut conteudo = String::new();
    for item in itens {
        if let Ok(linha) = serde_json::to_string(item) {
            conteudo.push_str(&linha);
            conteudo.push('\n');
        }
    }

    if let Err(erro) = std::fs::write(caminho, conteudo) {
        eprintln!("[jarvis] não reescrevi {}: {erro}", caminho.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::chat::Role;

    fn temporaria(nome: &str) -> (Memoria, PathBuf) {
        let raiz = std::env::temp_dir()
            .join("jarvis-testes-memoria")
            .join(nome);
        let _ = std::fs::remove_dir_all(&raiz);
        (Memoria::new(&raiz), raiz)
    }

    /// O ponto da feature: fechar o app não pode apagar o que ele aprendeu.
    #[test]
    fn sobrevive_a_reabrir_o_app() {
        let (memoria, raiz) = temporaria("persiste");
        memoria.push_message(ChatMessage::new(Role::User, "oi"));
        memoria.lembrar("rotina da manhã", "Acorda 6h30 e vai para a [[academia]].");
        memoria.apelidar("meu jogo", "steam");
        drop(memoria);

        let memoria = Memoria::new(&raiz);
        assert_eq!(memoria.historico().len(), 1);
        assert_eq!(
            memoria.apelidos().get("meu jogo").map(String::as_str),
            Some("steam")
        );

        let nota = memoria
            .notas()
            .into_iter()
            .find(|n| n.nome == "rotina-da-manha")
            .expect("a nota tem que estar no disco");
        assert_eq!(nota.links(), ["academia"]);
    }

    /// "Evoluir" é isto: o mesmo assunto acumula, em vez de a última frase apagar a
    /// anterior.
    #[test]
    fn nota_cresce_em_vez_de_se_sobrescrever() {
        let (memoria, _) = temporaria("cresce");

        assert!(memoria.lembrar("academia", "Treina na Smart Fit."));
        assert!(memoria.lembrar("academia", "Vai segunda, quarta e sexta."));
        // Repetido com outra pontuação: não duplica.
        assert!(!memoria.lembrar("academia", "treina na smart fit"));

        let notas = memoria.notas();
        let nota = notas.iter().find(|n| n.nome == "academia").expect("existe");
        assert!(nota.corpo.contains("Smart Fit"));
        assert!(nota.corpo.contains("segunda, quarta e sexta"));
        assert_eq!(notas.iter().filter(|n| n.nome == "academia").count(), 1);
    }

    /// Apagar tem que sumir com o ARQUIVO, senão a nota volta no próximo carregamento.
    #[test]
    fn esquecer_apaga_o_arquivo_do_disco() {
        let (memoria, raiz) = temporaria("esquece");
        memoria.lembrar("academia", "Treina de manhã.");
        memoria.lembrar("trabalho", "Das 9 às 18.");

        let apagadas = memoria.esquecer("academia");
        assert_eq!(apagadas, ["academia"]);
        assert!(!raiz.join(PASTA_NOTAS).join("academia.md").exists());

        let memoria = Memoria::new(&raiz);
        assert_eq!(memoria.notas().len(), 1);
        assert!(memoria.esquecer("nada com isso").is_empty());
    }

    /// O índice é o que o modelo lê para saber o que pode linkar — não pode ficar para
    /// trás do que existe na pasta.
    #[test]
    fn o_indice_acompanha_as_notas() {
        let (memoria, raiz) = temporaria("indice");
        memoria.lembrar("academia", "Treina de manhã.");

        let indice = std::fs::read_to_string(raiz.join(INDICE)).expect("índice existe");
        assert!(indice.contains("[[academia]]"));

        memoria.esquecer("academia");
        let indice = std::fs::read_to_string(raiz.join(INDICE)).expect("índice existe");
        assert!(!indice.contains("[[academia]]"));
    }

    /// Limpar a conversa na tela não pode encostar no conhecimento: são duas coisas
    /// diferentes, e apagar o que ele aprendeu tem outro caminho ("esquece X").
    #[test]
    fn limpar_o_historico_nao_apaga_as_notas() {
        let (memoria, _) = temporaria("limpa");
        memoria.push_message(ChatMessage::new(Role::User, "oi"));
        memoria.escrever_conhecimento("projeto jarvis", "Usa Tauri com Next.");

        memoria.limpar_historico();

        assert!(memoria.historico().is_empty());
        assert_eq!(memoria.notas().len(), 1);
    }

    /// A nota de rotinas é derivada do log; apagar só a faria voltar, e o usuário
    /// acharia que o "esquece" não funcionou.
    #[test]
    fn rotina_observada_vira_nota_e_nao_e_esquecivel() {
        let (memoria, _) = temporaria("rotinas");
        for dia in 1..=3 {
            memoria.registrar_acao(Acao {
                quando: Local::now().timestamp_millis() - dia * 86_400_000,
                acao: "open_app".to_owned(),
                alvo: "spotify".to_owned(),
                ok: true,
            });
        }
        memoria.atualizar_rotinas();

        let notas = memoria.notas();
        let nota = notas
            .iter()
            .find(|n| n.nome == NOTA_DE_ROTINAS)
            .expect("existe");
        assert_eq!(nota.tipo, Tipo::Rotina);
        assert!(nota.corpo.contains("spotify"));

        assert!(memoria.esquecer("spotify").is_empty());
    }

    /// Corrigir à mão não pode reclassificar a nota: o tipo diz de ONDE o conhecimento
    /// veio, e passar a mão no texto não reescreve a origem dele.
    #[test]
    fn reescrever_troca_o_texto_e_mantem_o_tipo() {
        let (memoria, raiz) = temporaria("reescreve");
        memoria.aprender(
            "bitcoin preço",
            "Bitcoin é uma criptomoeda descentralizada.",
        );

        assert!(memoria.reescrever("bitcoin-preco", "Cotação não vira nota."));

        let notas = memoria.notas();
        let nota = notas
            .iter()
            .find(|n| n.nome == "bitcoin-preco")
            .expect("existe");
        assert_eq!(nota.corpo, "Cotação não vira nota.");
        assert_eq!(nota.tipo, Tipo::Aprendido, "o tipo é a origem, não o texto");

        // E foi para o DISCO: a tela mostra a cópia em memória, e sem isto a correção
        // sumiria no próximo `recarregar`.
        let markdown = std::fs::read_to_string(raiz.join(PASTA_NOTAS).join("bitcoin-preco.md"))
            .expect("arquivo");
        assert!(markdown.contains("Cotação não vira nota."));
        assert!(markdown.contains("tipo: aprendido"));
    }

    /// Um `Ctrl+A Delete` sem querer no campo de texto não pode virar exclusão silenciosa.
    /// Apagar tem botão próprio, e ele avisa antes.
    #[test]
    fn reescrever_com_texto_vazio_nao_apaga_a_nota() {
        let (memoria, _) = temporaria("reescreve-vazio");
        memoria.aprender("stan lee", "Criador de heróis.");

        assert!(!memoria.reescrever("stan-lee", "   "));
        assert_eq!(memoria.corpo_da_nota("stan lee"), "Criador de heróis.");
    }

    #[test]
    fn reescrever_nota_que_nao_existe_avisa_em_vez_de_criar() {
        let (memoria, _) = temporaria("reescreve-ausente");

        assert!(!memoria.reescrever("nunca-existiu", "texto"));
        assert!(memoria.notas().is_empty());
    }

    /// O botão de apagar leva UMA nota — a que está aberta. O "esquece X" falado é que
    /// casa por termo e pode levar várias; confundir os dois seria uma armadilha.
    #[test]
    fn apagar_leva_so_a_nota_apontada() {
        let (memoria, raiz) = temporaria("apaga");
        memoria.aprender("bitcoin preço", "verbete errado");
        memoria.aprender("valor do bitcoin", "outro verbete errado");

        assert!(memoria.apagar_nota("bitcoin-preco"));

        let restantes: Vec<String> = memoria.notas().into_iter().map(|n| n.nome).collect();
        assert_eq!(restantes, ["valor-do-bitcoin"]);
        assert!(!raiz.join(PASTA_NOTAS).join("bitcoin-preco.md").exists());

        // Apagar de novo não é erro de programa, é um "não achei" — a tela pode ter
        // ficado aberta enquanto a nota sumia por outro caminho.
        assert!(!memoria.apagar_nota("bitcoin-preco"));
    }

    /// O grafo se redesenha por causa deste número. Uma correção que não o mexesse
    /// deixaria a tela mostrando o texto velho até alguém clicar em atualizar.
    #[test]
    fn editar_e_apagar_contam_como_mudanca() {
        let (memoria, _) = temporaria("versao");
        memoria.aprender("stan lee", "Criador de heróis.");

        let depois_de_aprender = memoria.versao();
        assert!(memoria.reescrever("stan-lee", "Editor da Marvel."));
        assert!(memoria.versao() > depois_de_aprender);

        let depois_de_editar = memoria.versao();
        assert!(memoria.apagar_nota("stan-lee"));
        assert!(memoria.versao() > depois_de_editar);
    }
}
