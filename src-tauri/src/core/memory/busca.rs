//! Que notas entram no prompt.
//!
//! O problema: a memória cresce sem limite, o contexto do modelo não. Precisa escolher.
//!
//! A escolha aqui é **palavra-chave mais um salto no grafo** — as notas que casam com o
//! que foi dito, MAIS as notas que elas citam com `[[link]]`. É o salto que paga: quem
//! pergunta "que horas eu acordo" casa com `rotina-da-manha`, e se aquela nota cita
//! `[[academia]]`, a academia vem junto sem ter sido mencionada.
//!
//! Não é embedding, e é de propósito. Embedding exigiria um segundo modelo carregado
//! (`nomic-embed-text`, ~274 MB) disputando os 4 GB de VRAM com o intérprete, mais um
//! índice para manter em sincronia com arquivos que o usuário edita no Obsidian por
//! fora. Palavra-chave erra mais, custa zero e nunca fica dessincronizada.
//!
//! ponytail: busca lexical, sem embedding. Sinônimo ("carro" não acha "veículo") passa
//! batido. Se isso incomodar de verdade, o caminho é `nomic-embed-text` via Ollama com
//! o índice reconstruído quando o `mtime` do arquivo muda.

use std::collections::HashSet;

use super::nota::Nota;

/// Palavras curtas demais ou comuns demais só adicionam ruído ao casamento.
const VAZIAS: [&str; 35] = [
    "que", "com", "por", "para", "uma", "dos", "das", "nos", "nas", "meu", "minha", "seu", "sua",
    "ele", "ela", "isso", "esse", "essa", "aqui", "ali", "mais", "menos", "muito", "quando",
    "onde", "como", "qual", "quais", "voce", "sobre", "tem", "ser", "estar", "fazer", "mim",
];

/// As notas que valem a pena mandar junto com `frase`.
///
/// Sem nenhum casamento, devolve as mais recentes — memória que só aparece quando é
/// invocada pelo nome exato não parece memória, parece busca.
pub fn relevantes(notas: &[Nota], frase: &str, teto: usize) -> Vec<Nota> {
    let mut escolhidas = casadas(notas, frase, teto);
    generosidades(notas, &mut escolhidas, teto);
    escolhidas
}

/// Só as notas que CASARAM, sem salto no grafo e sem fallback.
///
/// **É o [`relevantes`] menos as duas generosidades dele**, e as duas saem pelo mesmo
/// motivo: quem chama isto é o prompt da BUSCA, onde o modelo já está lendo trechos de
/// páginas e a instrução é "use só o que está aqui". Nesse prompt:
///
/// - o **fallback** seria veneno. Ele devolve as notas mais recentes, escolhidas por DATA,
///   e ali elas apareceriam ao lado de trechos sobre outro assunto — com cara de fonte
///   conferida. É exatamente o modo de falha que fez o [`casou`] existir, agora dentro do
///   prompt que mais depende de não inventar.
/// - o **salto no grafo** é enfeite de conversa. Ele existe para a memória parecer memória
///   ("acordo às 6" puxando `[[academia]]`), e ali a pergunta é sobre o mundo.
///
/// Devolve vazio quando nada casou, e é o chamador que decide o que fazer com isso — no
/// caso, não montar bloco nenhum e deixar o prompt idêntico ao que sempre foi.
pub fn casadas(notas: &[Nota], frase: &str, teto: usize) -> Vec<Nota> {
    if notas.is_empty() || teto == 0 {
        return Vec::new();
    }

    let mut pontuadas = pontuadas(notas, frase);

    // Mais pontos primeiro; empate desempata pelo nome, para a saída ser estável entre
    // execuções (um prompt que muda sozinho é impossível de depurar).
    pontuadas.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.nome.cmp(&b.1.nome)));

    pontuadas
        .into_iter()
        .take(teto)
        .map(|(_, nota)| nota.clone())
        .collect()
}

/// O salto no grafo e o fallback, que o [`relevantes`] tem e o [`casadas`] não.
fn generosidades(notas: &[Nota], escolhidas: &mut Vec<Nota>, teto: usize) {
    let mut vistas: HashSet<String> = escolhidas.iter().map(|nota| nota.nome.clone()).collect();

    // O salto no grafo: puxa o que as escolhidas citam. Um salto só — dois já arrastam
    // metade da memória quando as notas são bem ligadas.
    let citadas: Vec<String> = escolhidas.iter().flat_map(Nota::links).collect();
    for alvo in citadas {
        if escolhidas.len() >= teto {
            break;
        }
        if let Some(vizinha) = notas.iter().find(|nota| nota.nome == alvo) {
            if vistas.insert(vizinha.nome.clone()) {
                escolhidas.push(vizinha.clone());
            }
        }
    }

    if escolhidas.is_empty() {
        let mut recentes: Vec<&Nota> = notas.iter().collect();
        recentes.sort_by(|a, b| b.atualizado.cmp(&a.atualizado).then(a.nome.cmp(&b.nome)));
        *escolhidas = recentes.into_iter().take(teto).cloned().collect();
    }
}

/// As notas que casaram, com quantos pontos cada uma. Sem ordem e sem o fallback.
///
/// Extraída para o [`relevantes`] e o [`casou`] responderem à MESMA pergunta: um deles
/// monta o prompt e o outro decide se vale ir à internet, e nada seria mais confuso que
/// os dois discordarem sobre o que "casar" quer dizer depois que alguém mexer nas
/// [`VAZIAS`] ou no peso do nome em [`pontuar`].
fn pontuadas<'a>(notas: &'a [Nota], frase: &str) -> Vec<(usize, &'a Nota)> {
    let termos = termos_de(frase);

    notas
        .iter()
        .map(|nota| (pontuar(nota, &termos), nota))
        .filter(|(pontos, _)| *pontos > 0)
        .collect()
}

/// Alguma nota casou de VERDADE com a frase — ou o que sai de [`relevantes`] é só o
/// fallback das mais recentes?
///
/// **Existe porque o fallback apaga o sinal, e apagá-lo mentia para o modelo.** Sem
/// casamento nenhum, `relevantes` devolve as mais recentes e `Memoria::contexto` as
/// rotulava como "Conteúdo das mais relevantes agora" — oito notas escolhidas por DATA,
/// apresentadas como se tivessem a ver com a pergunta. O modelo então respondia como quem
/// tem contexto, e o que ele não achava ali completava de cabeça.
///
/// A resposta certa não era tirar o fallback (ele é de propósito, veja [`relevantes`]) e
/// sim dizer a verdade sobre o que ele é. Quem decide o que fazer com isso é o prompt.
///
/// Barato: é a mesma pontuação de string do [`relevantes`], sem ordenação, sem clone e
/// sem salto no grafo, e para no primeiro acerto. Nenhuma chamada ao modelo.
pub fn casou(notas: &[Nota], frase: &str) -> bool {
    let termos = termos_de(frase);
    notas.iter().any(|nota| pontuar(nota, &termos) > 0)
}

/// Nome vale mais que corpo: uma nota chamada `academia` é mais sobre academia do que
/// uma que menciona a palavra no meio de um parágrafo.
fn pontuar(nota: &Nota, termos: &[String]) -> usize {
    let nome = nota.nome.replace('-', " ");
    let corpo = super::normalizar(&nota.corpo);

    termos
        .iter()
        .map(|termo| {
            let mut pontos = 0;
            if nome.contains(termo.as_str()) {
                pontos += 3;
            }
            if corpo.contains(termo.as_str()) {
                pontos += 1;
            }
            pontos
        })
        .sum()
}

fn termos_de(frase: &str) -> Vec<String> {
    super::normalizar(frase)
        .split(' ')
        .filter(|palavra| palavra.len() >= 3 && !VAZIAS.contains(palavra))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::nota::{Nota, Tipo};
    use super::*;

    /// O sinal que o fallback apaga — e a razão de os TRÊS existirem lado a lado.
    ///
    /// Amarra os comportamentos num teste só de propósito: quebra se alguém "consertar" o
    /// fallback do `relevantes` sem olhar o `casou`, ou vice-versa.
    ///
    /// O `casadas` entrou aqui pelo mesmo motivo, e não num teste separado: ele é o
    /// `relevantes` SEM o fallback, então a única coisa que garante que ele continua sendo
    /// isso é os dois serem cobrados na mesma frase, com as mesmas notas.
    #[test]
    fn casou_e_o_sinal_que_o_fallback_esconde() {
        let notas = vec![Nota::nova(
            "academia",
            Tipo::Fato,
            "Treina na Smart Fit de manhã.",
            "2026-01-01",
        )];

        assert!(
            !casou(&notas, "cotacao do bitcoin"),
            "nada ali fala de bitcoin"
        );
        assert_eq!(
            relevantes(&notas, "cotacao do bitcoin", 1).len(),
            1,
            "e mesmo assim o relevantes entrega a mais recente — é o fallback, e ele fica"
        );
        assert!(
            casadas(&notas, "cotacao do bitcoin", 1).is_empty(),
            "o casadas é o que NÃO tem fallback — é ele que o prompt da busca usa, e uma              nota escolhida por data ao lado dos trechos parece fonte conferida"
        );
    }

    /// O outro lado: quando casa de verdade, os dois entregam a mesma nota.
    #[test]
    fn casadas_entrega_o_que_casou() {
        let notas = vec![
            Nota::nova("academia", Tipo::Fato, "Treina de manhã.", "2026-01-01"),
            Nota::nova("cafe", Tipo::Fato, "Gosta de coado.", "2026-02-01"),
        ];

        let achadas = casadas(&notas, "que horas abre a academia?", 8);

        assert_eq!(achadas.len(), 1, "só uma nota fala de academia");
        assert_eq!(achadas[0].nome, "academia");
    }

    /// O salto no grafo é generosidade de CONVERSA, e não pode vazar para a busca: ali o
    /// modelo está lendo trechos de página, e uma nota que entrou de carona por citação
    /// chega sem ter casado com nada.
    #[test]
    fn casadas_nao_da_o_salto_no_grafo() {
        let notas = vec![
            Nota::nova(
                "rotina-da-manha",
                Tipo::Fato,
                "Acorda às 6 e vai para a [[academia]].",
                "2026-01-01",
            ),
            Nota::nova(
                "academia",
                Tipo::Fato,
                "Smart Fit da esquina.",
                "2026-01-01",
            ),
        ];

        assert_eq!(
            relevantes(&notas, "rotina da manha", 8).len(),
            2,
            "na conversa a academia vem junto, pelo [[link]]"
        );
        assert_eq!(
            casadas(&notas, "rotina da manha", 8).len(),
            1,
            "na busca, só o que casou"
        );
    }

    #[test]
    fn casou_pelo_nome_e_pelo_corpo() {
        let notas = vec![Nota::nova(
            "academia",
            Tipo::Fato,
            "Treina na Smart Fit de manhã.",
            "2026-01-01",
        )];

        assert!(
            casou(&notas, "que horas abre a academia?"),
            "casa pelo nome"
        );
        assert!(
            casou(&notas, "ainda treina na smart fit?"),
            "casa pelo corpo"
        );
        // Só palavras curtas ou da lista de VAZIAS: não sobra termo nenhum para casar.
        assert!(!casou(&notas, "o que é isso para mim?"));
    }

    #[test]
    fn memoria_vazia_nao_cobre_nada() {
        assert!(!casou(&[], "qualquer coisa que seja"));
    }

    fn nota(nome: &str, corpo: &str, dia: &str) -> Nota {
        Nota::nova(nome, Tipo::Fato, corpo, dia)
    }

    fn nomes(notas: &[Nota]) -> Vec<&str> {
        notas.iter().map(|n| n.nome.as_str()).collect()
    }

    /// O caso que justifica o módulo: perguntar sobre uma coisa traz a nota dela E o
    /// que ela cita, mesmo que o vizinho não tenha sido mencionado.
    #[test]
    fn puxa_o_vizinho_pelo_link() {
        let notas = [
            nota(
                "rotina-da-manha",
                "Acorda 6h30 e vai para a [[academia]].",
                "2026-08-01",
            ),
            nota("academia", "Treina na Smart Fit da esquina.", "2026-08-01"),
            nota("trabalho", "Das 9 às 18, home office.", "2026-08-01"),
        ];

        let achadas = relevantes(&notas, "que horas eu acordo de manhã?", 5);

        assert!(
            nomes(&achadas).contains(&"rotina-da-manha"),
            "não casou o óbvio"
        );
        assert!(
            nomes(&achadas).contains(&"academia"),
            "não seguiu o link — sem isso é lista, não grafo"
        );
        assert!(
            !nomes(&achadas).contains(&"trabalho"),
            "trouxe nota sem relação"
        );
    }

    /// Memória que só aparece quando invocada pelo nome exato não parece memória.
    #[test]
    fn sem_casamento_devolve_as_mais_recentes() {
        let notas = [
            nota("antiga", "Nada a ver.", "2026-01-01"),
            nota("nova", "Também não.", "2026-08-20"),
        ];

        let achadas = relevantes(&notas, "xyzzy plugh", 1);
        assert_eq!(nomes(&achadas), ["nova"]);
    }

    #[test]
    fn respeita_o_teto_e_a_memoria_vazia() {
        let notas: Vec<Nota> = (0..10)
            .map(|i| nota(&format!("n{i}"), "acordo cedo", "2026-08-01"))
            .collect();

        assert_eq!(relevantes(&notas, "acordo", 3).len(), 3);
        assert!(relevantes(&notas, "acordo", 0).is_empty());
        assert!(relevantes(&[], "acordo", 5).is_empty());
    }

    /// Palavra vazia casando com tudo faria a busca devolver a memória inteira.
    #[test]
    fn ignora_palavras_comuns_demais() {
        assert!(termos_de("o que é isso para mim com você").is_empty());
        assert_eq!(termos_de("abre o spotify"), ["abre", "spotify"]);
    }
}
