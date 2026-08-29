//! As notas viradas grafo, para a tela desenhar o que o Jarvis sabe.
//!
//! ## O problema que este arquivo resolve
//!
//! A ideia veio do grafo do Obsidian, e lá ele funciona porque **as pessoas linkam as
//! notas à mão** enquanto escrevem. Aqui não: quando isto foi escrito havia 18 notas e
//! exatamente UM `[[link]]` real entre elas. Um grafo só de wikilinks seria dezoito pontos
//! soltos e uma linha — pareceria defeito, não mapa de conhecimento.
//!
//! Por isso as arestas têm **duas origens**, e a tela as distingue:
//!
//! - **Escritas** ([`Aresta::escrita`] = `true`): um `[[alvo]]` no corpo da nota, posto ali
//!   pelo próprio Jarvis ao escrevê-la. São as boas — significam relação de verdade.
//! - **Inferidas**: notas que falam das mesmas coisas, medido por termos em comum. Não são
//!   uma opinião sobre o assunto, são uma pista — e é o que faz o grafo já ter forma no dia
//!   em que a primeira nota é criada.
//!
//! Conforme o Jarvis escreve mais notas com links, as escritas crescem e as inferidas
//! passam a ser o fundo. O grafo melhora sozinho, sem migração.
//!
//! ## O que NÃO está aqui
//!
//! Nada de embeddings. Semelhança por termo é grosseira perto de um vetor, mas não custa
//! um modelo, não custa uma dependência, e roda em microssegundos sobre o texto que já está
//! na memória. Se um dia houver embeddings no projeto por outra razão, trocar a fonte das
//! arestas inferidas é mexer numa função só.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::nota::{slug, Nota};

/// Abaixo disto a semelhança é coincidência de vocabulário, não assunto em comum.
///
/// **Medido nas 18 notas reais** pelo `grafo_de_verdade`, em arestas por nó:
///
/// | limiar | arestas | por nó | |
/// | --- | --- | --- | --- |
/// | 0,05 | 42 | 2,3 | denso |
/// | **0,08** | **19** | **1,1** | navegável |
/// | 0,10 | 13 | 0,7 | esparso |
/// | 0,12 | 6 | 0,3 | quase nada |
/// | 0,20 | 3 | 0,2 | poeira de pontos |
///
/// O alvo é de 1 a 3 ligações por nó: menos que isso e o grafo é uma poeira de pontos que
/// não se navega, mais e vira bola de lã onde tudo se liga a tudo e nada significa.
///
/// O primeiro palpite foi 0,12, escrito antes de medir — ele dava 0,3 por nó. É o tipo de
/// número que só a ferramenta resolve.
const SEMELHANCA_MINIMA: f32 = 0.08;

/// Termos que aparecem em toda nota e não dizem nada sobre o assunto.
///
/// Lista curta de propósito: uma lista grande de stopwords portuguesas viraria manutenção,
/// e o efeito prático de tirar "que" e "para" já é quase todo o ganho. O corte por tamanho
/// (`len() > 3`) faz o resto do trabalho de graça.
const VAZIAS: [&str; 34] = [
    "para", "como", "mais", "esta", "está", "isso", "esse", "essa", "pelo", "pela", "sobre",
    "quando", "porque", "sendo", "pode", "podem", "seja", "sero", "tem", "temos", "foi",
    "eles", "elas", "dele", "dela", "nao", "sim", "com", "sem", "dos", "das", "uma", "uns",
    "voce",
];

/// Um assunto que o Jarvis conhece.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct No {
    /// O slug da nota — é a chave, e é o alvo de `[[isto]]`.
    pub id: String,
    /// O mesmo, legível: `tony-stark` vira `Tony Stark`.
    pub rotulo: String,
    /// `fato`, `aprendido`, `resumo` ou `rotina`. É por aqui que a tela filtra.
    pub tipo: String,
    /// Quanto ele sabe do assunto, de 0 a 1. Ver [`peso_de`].
    pub peso: f32,
    /// Caracteres da nota. Vai para o painel lateral, junto do peso, porque "0,73" sozinho
    /// não explica nada a quem olha.
    pub tamanho: usize,
    pub atualizado: String,
    /// Quantas outras notas apontam para esta.
    pub citacoes: usize,
}

/// Uma ligação entre dois assuntos.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aresta {
    pub de: String,
    pub para: String,
    /// 0 a 1. Nas escritas é sempre 1; nas inferidas é a semelhança medida.
    pub forca: f32,
    /// `true` quando o link foi ESCRITO na nota. A tela desenha essas cheias e as inferidas
    /// apagadas — misturar as duas sem distinção seria apresentar um palpite como fato.
    pub escrita: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grafo {
    pub nos: Vec<No>,
    pub arestas: Vec<Aresta>,
}

/// Monta o grafo a partir das notas.
pub fn montar(notas: &[Nota]) -> Grafo {
    let existentes: BTreeSet<&str> = notas.iter().map(|nota| nota.nome.as_str()).collect();
    let termos: Vec<(usize, BTreeSet<String>)> = notas
        .iter()
        .enumerate()
        .map(|(i, nota)| (i, termos_de(&nota.corpo)))
        .collect();

    let mut arestas = Vec::new();
    let mut citacoes: BTreeMap<&str, usize> = BTreeMap::new();

    // ---- as escritas: `[[alvo]]` no corpo -------------------------------
    for nota in notas {
        for alvo in wikilinks(&nota.corpo) {
            // **Link para nota inexistente é descartado.** O modelo inventa alvo de vez em
            // quando — o `[[nota nova]]` que apareceu numa das notas veio de um placeholder
            // entre parênteses no prompt. Desenhar um nó para cada alucinação encheria o
            // grafo de fantasmas.
            if alvo == nota.nome || !existentes.contains(alvo.as_str()) {
                continue;
            }

            *citacoes.entry(chave(&existentes, &alvo)).or_default() += 1;
            arestas.push(Aresta {
                de: nota.nome.clone(),
                para: alvo,
                forca: 1.0,
                escrita: true,
            });
        }
    }

    // ---- as inferidas: termos em comum ----------------------------------
    for (i, meus) in &termos {
        for (j, seus) in &termos {
            if j <= i {
                continue;
            }

            let semelhanca = jaccard(meus, seus);
            if semelhanca < SEMELHANCA_MINIMA {
                continue;
            }

            // Já existe link escrito entre os dois? Então a inferida é ruído: a relação já
            // está afirmada, e desenhar as duas deixaria a linha mais grossa por acidente.
            let ja_ligados = arestas.iter().any(|aresta| {
                (aresta.de == notas[*i].nome && aresta.para == notas[*j].nome)
                    || (aresta.de == notas[*j].nome && aresta.para == notas[*i].nome)
            });
            if ja_ligados {
                continue;
            }

            arestas.push(Aresta {
                de: notas[*i].nome.clone(),
                para: notas[*j].nome.clone(),
                forca: semelhanca,
                escrita: false,
            });
        }
    }

    let maior = notas.iter().map(|nota| nota.corpo.len()).max().unwrap_or(1);

    Grafo {
        nos: notas
            .iter()
            .map(|nota| {
                let recebidas = citacoes.get(nota.nome.as_str()).copied().unwrap_or(0);
                No {
                    rotulo: legivel(&nota.nome),
                    tipo: nota.tipo.como_texto().to_owned(),
                    peso: peso_de(nota.corpo.len(), maior, recebidas),
                    tamanho: nota.corpo.len(),
                    atualizado: nota.atualizado.clone(),
                    citacoes: recebidas,
                    id: nota.nome.clone(),
                }
            })
            .collect(),
        arestas,
    }
}

/// O "nível de conhecimento" de um assunto, de 0 a 1.
///
/// Duas coisas entram, e as duas são observáveis — nada aqui é opinião do modelo:
///
/// - **Quanto foi escrito** sobre o assunto, relativo à maior nota. É o sinal principal:
///   uma nota de três linhas e uma de três parágrafos não representam o mesmo domínio.
/// - **Quantas notas apontam para esta.** Um assunto citado por outros é um assunto
///   central, mesmo que a nota dele seja curta.
///
/// A raiz quadrada no tamanho é a mesma decisão dos medidores de áudio do projeto: sem ela,
/// uma nota gigante achataria todas as outras perto de zero, e o grafo viraria um sol com
/// planetas invisíveis.
fn peso_de(tamanho: usize, maior: usize, citacoes: usize) -> f32 {
    let proporcao = (tamanho as f32 / maior.max(1) as f32).sqrt();
    // Cada citação vale um empurrão que satura rápido: a terceira já quase não muda nada.
    let empurrao = 1.0 - (-(citacoes as f32) / 2.0).exp();

    (proporcao * 0.75 + empurrao * 0.25).clamp(0.05, 1.0)
}

/// Os alvos de `[[isto]]` no texto.
fn wikilinks(corpo: &str) -> Vec<String> {
    let mut achados = Vec::new();
    let mut resto = corpo;

    while let Some(inicio) = resto.find("[[") {
        let depois = &resto[inicio + 2..];
        let Some(fim) = depois.find("]]") else { break };

        let alvo = depois[..fim].trim();
        if !alvo.is_empty() {
            achados.push(slug(alvo));
        }
        resto = &depois[fim + 2..];
    }

    achados
}

/// As palavras que dizem do que a nota trata.
fn termos_de(corpo: &str) -> BTreeSet<String> {
    super::normalizar(corpo)
        .split(' ')
        .filter(|palavra| palavra.len() > 3 && !VAZIAS.contains(palavra))
        .map(str::to_owned)
        .collect()
}

/// Quanto dois conjuntos de termos se sobrepõem, de 0 a 1.
fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let comuns = a.intersection(b).count() as f32;
    let todos = a.union(b).count() as f32;

    comuns / todos
}

/// `tony-stark` vira `Tony Stark`.
fn legivel(slug: &str) -> String {
    slug.split('-')
        .map(|parte| {
            let mut letras = parte.chars();
            match letras.next() {
                Some(primeira) => primeira.to_uppercase().collect::<String>() + letras.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Devolve a fatia emprestada do conjunto, para a chave do mapa não duplicar a String.
fn chave<'a>(existentes: &BTreeSet<&'a str>, alvo: &str) -> &'a str {
    existentes
        .iter()
        .find(|nome| **nome == alvo)
        .copied()
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::Tipo;

    fn nota(nome: &str, corpo: &str) -> Nota {
        Nota::nova(nome, Tipo::Fato, corpo, "2026-08-29")
    }

    #[test]
    fn o_link_escrito_vira_aresta_cheia() {
        let notas = [
            nota("tony-stark", "Tony Stark construiu a armadura. Ver [[stan-lee]]."),
            nota("stan-lee", "Stan Lee criou o personagem."),
        ];

        let grafo = montar(&notas);
        let escrita = grafo
            .arestas
            .iter()
            .find(|aresta| aresta.escrita)
            .expect("a aresta escrita");

        assert_eq!(escrita.de, "tony-stark");
        assert_eq!(escrita.para, "stan-lee");
        assert_eq!(escrita.forca, 1.0);
    }

    /// O modelo inventa alvo de vez em quando — já aconteceu com `[[nota nova]]`, vindo de
    /// um placeholder do prompt. Um nó para cada alucinação encheria o grafo de fantasmas.
    #[test]
    fn link_para_nota_que_nao_existe_e_descartado() {
        let notas = [nota("sozinha", "Aponta para [[nota-que-nunca-existiu]].")];
        let grafo = montar(&notas);

        assert_eq!(grafo.nos.len(), 1);
        assert!(grafo.arestas.is_empty(), "o fantasma não pode virar aresta");
    }

    /// Sem isto o grafo nasceria vazio: as notas de hoje quase não se linkam.
    #[test]
    fn notas_sobre_a_mesma_coisa_se_ligam_sem_link_escrito() {
        let notas = [
            nota(
                "armadura",
                "A armadura do Homem de Ferro usa reator arc, voo supersônico e mísseis.",
            ),
            nota(
                "reator",
                "O reator arc alimenta a armadura do Homem de Ferro e permite o voo.",
            ),
            nota("pao-de-queijo", "Receita mineira com polvilho, queijo e ovo."),
        ];

        let grafo = montar(&notas);
        let inferidas: Vec<_> = grafo.arestas.iter().filter(|a| !a.escrita).collect();

        assert_eq!(inferidas.len(), 1, "só as duas do mesmo assunto se ligam");
        assert!(inferidas[0].forca >= SEMELHANCA_MINIMA);
        assert!(
            !inferidas.iter().any(|a| a.de.contains("pao") || a.para.contains("pao")),
            "receita não tem nada a ver com armadura"
        );
    }

    /// Havendo link escrito, a inferida seria a mesma relação contada duas vezes — e a
    /// linha ficaria mais grossa por acidente.
    #[test]
    fn a_inferida_nao_duplica_a_escrita() {
        let notas = [
            nota("armadura", "A armadura usa reator arc para voar. Ver [[reator]]."),
            nota("reator", "O reator arc alimenta a armadura para voar."),
        ];

        let grafo = montar(&notas);

        assert_eq!(grafo.arestas.len(), 1);
        assert!(grafo.arestas[0].escrita);
    }

    #[test]
    fn o_peso_cresce_com_o_tamanho_e_com_as_citacoes() {
        let curta = peso_de(100, 1000, 0);
        let longa = peso_de(1000, 1000, 0);
        let citada = peso_de(100, 1000, 3);

        assert!(longa > curta, "nota maior sabe mais");
        assert!(citada > curta, "assunto citado é mais central");
        assert!((0.0..=1.0).contains(&longa));
        // Nenhum nó pode sumir da tela por ser pequeno demais.
        assert!(peso_de(1, 100_000, 0) >= 0.05);
    }

    /// Monta o grafo com as notas REAIS e imprime como ele ficou, em vários limiares.
    ///
    /// É a ferramenta que calibrou o `SEMELHANCA_MINIMA`. Sem ela o número seria chute: o
    /// que importa não é a fórmula, é quantas arestas ela produz sobre as notas que existem
    /// — poucas demais e o grafo é uma poeira de pontos, muitas e vira bola de lã.
    ///
    /// ```text
    /// cargo test --lib -- --ignored --nocapture grafo_de_verdade
    /// ```
    #[test]
    #[ignore]
    fn grafo_de_verdade() {
        let raiz = std::env::var("JARVIS_MEMORIA").unwrap_or_else(|_| "../memoria".to_owned());
        let memoria = crate::core::memory::Memoria::new(std::path::Path::new(&raiz));
        let notas = memoria.notas();

        if notas.is_empty() {
            println!("nenhuma nota em {raiz} — aponte JARVIS_MEMORIA para a pasta certa");
            return;
        }

        println!("{} notas
", notas.len());

        // Quantas arestas cada limiar produziria. O alvo é um grafo navegável: mais ou
        // menos de 1 a 3 ligações por nó.
        println!("{:>8}  {:>8}  {:>10}  leitura", "limiar", "arestas", "por nó");
        for limiar in [0.05, 0.08, 0.10, 0.12, 0.15, 0.20, 0.30] {
            let quantas = contar_inferidas(&notas, limiar);
            let por_no = quantas as f32 / notas.len() as f32;
            let leitura = if por_no > 4.0 {
                "bola de lã"
            } else if por_no >= 1.0 {
                "navegável"
            } else if por_no > 0.2 {
                "esparso"
            } else {
                "poeira de pontos"
            };
            println!("{limiar:>8.2}  {quantas:>8}  {por_no:>10.1}  {leitura}");
        }

        let grafo = montar(&notas);
        let escritas = grafo.arestas.iter().filter(|a| a.escrita).count();
        println!(
            "
com o limiar atual ({SEMELHANCA_MINIMA}): {} arestas, sendo {escritas} escritas",
            grafo.arestas.len()
        );

        let mut ordenados = grafo.nos.clone();
        ordenados.sort_by(|a, b| b.peso.total_cmp(&a.peso));
        println!("
os assuntos que ele mais domina:");
        for no in ordenados.iter().take(6) {
            println!(
                "  {:<34} peso {:.2}  {:>6} chars  {} citações",
                no.rotulo, no.peso, no.tamanho, no.citacoes
            );
        }
    }

    /// Só para a ferramenta acima: conta as inferidas que um limiar produziria.
    fn contar_inferidas(notas: &[Nota], limiar: f32) -> usize {
        let termos: Vec<_> = notas.iter().map(|n| termos_de(&n.corpo)).collect();
        let mut quantas = 0;

        for (i, meus) in termos.iter().enumerate() {
            for seus in termos.iter().skip(i + 1) {
                if jaccard(meus, seus) >= limiar {
                    quantas += 1;
                }
            }
        }

        quantas
    }

    #[test]
    fn o_rotulo_fica_legivel() {
        assert_eq!(legivel("tony-stark"), "Tony Stark");
        assert_eq!(legivel("porta-da-sala"), "Porta Da Sala");
    }
}
