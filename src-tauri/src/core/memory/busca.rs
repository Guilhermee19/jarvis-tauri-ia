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
    if notas.is_empty() || teto == 0 {
        return Vec::new();
    }

    let termos = termos_de(frase);

    let mut pontuadas: Vec<(usize, &Nota)> = notas
        .iter()
        .map(|nota| (pontuar(nota, &termos), nota))
        .filter(|(pontos, _)| *pontos > 0)
        .collect();

    // Mais pontos primeiro; empate desempata pelo nome, para a saída ser estável entre
    // execuções (um prompt que muda sozinho é impossível de depurar).
    pontuadas.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.nome.cmp(&b.1.nome)));

    let mut escolhidas: Vec<Nota> = Vec::new();
    let mut vistas: HashSet<String> = HashSet::new();

    for (_, nota) in pontuadas.iter().take(teto) {
        if vistas.insert(nota.nome.clone()) {
            escolhidas.push((*nota).clone());
        }
    }

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
        escolhidas = recentes.into_iter().take(teto).cloned().collect();
    }

    escolhidas
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
