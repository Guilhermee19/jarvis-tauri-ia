//! Rotinas observadas: o que o usuário costuma pedir, e quando.
//!
//! Isto é "aprender minhas rotinas" sem nenhum aprendizado de máquina. O log de ações
//! já grava verbo, alvo, hora e desfecho de tudo que foi executado; agrupar por
//! (ação, alvo, período do dia) e contar é a coisa toda.
//!
//! O resultado vira uma nota `rotinas-observadas.md`, reescrita pelo próprio Jarvis.
//! Ela entra na memória como qualquer outra nota — dá para ler no Obsidian, dá para
//! linkar, e dá para o modelo citar numa conversa.

use std::collections::HashMap;

use chrono::{DateTime, Local, Timelike};
use serde::{Deserialize, Serialize};

use super::Acao;

/// Abaixo disto é coincidência, não rotina. Duas vezes é acaso; três é hábito.
const MINIMO_PARA_SER_ROTINA: usize = 3;

/// Ações fracassadas não entram: o que o usuário TENTOU não é o que ele faz.
const TETO_DE_ROTINAS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Periodo {
    Madrugada,
    Manha,
    Tarde,
    Noite,
}

impl Periodo {
    /// Faixas pensadas para como as pessoas falam ("de manhã", "à noite"), não para
    /// dividir o dia em quatro partes iguais.
    fn da_hora(hora: u32) -> Self {
        match hora {
            0..=5 => Self::Madrugada,
            6..=11 => Self::Manha,
            12..=17 => Self::Tarde,
            _ => Self::Noite,
        }
    }

    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Madrugada => "de madrugada",
            Self::Manha => "de manhã",
            Self::Tarde => "à tarde",
            Self::Noite => "à noite",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotina {
    pub acao: String,
    pub alvo: String,
    pub periodo: Periodo,
    pub vezes: usize,
}

impl Rotina {
    fn como_frase(&self) -> String {
        let o_que = match self.acao.as_str() {
            "open_app" => format!("abre o {}", self.alvo),
            "open_site" => format!("abre {}", self.alvo),
            "web_search" => "pesquisa no Google".to_owned(),
            "volume_set" => format!("põe o volume em {}", self.alvo),
            "volume_up" => "aumenta o volume".to_owned(),
            "volume_down" => "abaixa o volume".to_owned(),
            "volume_mute" => "muda o mudo".to_owned(),
            "media_play_pause" => "pausa ou retoma a música".to_owned(),
            "media_next" => "pula de faixa".to_owned(),
            "media_previous" => "volta de faixa".to_owned(),
            outro => format!("{outro} {}", self.alvo),
        };

        format!(
            "- {} {} ({}x)",
            o_que,
            self.periodo.como_texto(),
            self.vezes
        )
    }
}

pub fn agregar(acoes: &[Acao]) -> Vec<Rotina> {
    let mut contagem: HashMap<(String, String, Periodo), usize> = HashMap::new();

    for acao in acoes.iter().filter(|a| a.ok) {
        let Some(quando) = DateTime::from_timestamp_millis(acao.quando) else {
            continue;
        };
        let periodo = Periodo::da_hora(quando.with_timezone(&Local).hour());

        *contagem
            .entry((acao.acao.clone(), acao.alvo.clone(), periodo))
            .or_default() += 1;
    }

    let mut rotinas: Vec<Rotina> = contagem
        .into_iter()
        .filter(|(_, vezes)| *vezes >= MINIMO_PARA_SER_ROTINA)
        .map(|((acao, alvo, periodo), vezes)| Rotina {
            acao,
            alvo,
            periodo,
            vezes,
        })
        .collect();

    // Mais frequente primeiro; empate pelo nome, para a nota não embaralhar sozinha a
    // cada regravação e poluir o diff do git.
    rotinas.sort_by(|a, b| {
        b.vezes
            .cmp(&a.vezes)
            .then_with(|| a.acao.cmp(&b.acao))
            .then_with(|| a.alvo.cmp(&b.alvo))
    });
    rotinas.truncate(TETO_DE_ROTINAS);
    rotinas
}

/// Corpo da nota `rotinas-observadas.md`. `None` quando ainda não há hábito nenhum —
/// escrever "nenhuma rotina observada" só ocuparia espaço no prompt.
pub fn como_nota(rotinas: &[Rotina]) -> Option<String> {
    if rotinas.is_empty() {
        return None;
    }

    let linhas: Vec<String> = rotinas.iter().map(Rotina::como_frase).collect();

    Some(format!(
        "O que você costuma pedir, contado a partir do log de ações. Escrito pelo Jarvis \
         — editar aqui não muda nada, porque esta nota é regravada.\n\n{}",
        linhas.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Meia-noite local, para os testes não dependerem do fuso da máquina.
    fn as_horas(hora: u32, dia: u32) -> i64 {
        use chrono::{NaiveDate, TimeZone};
        let data = NaiveDate::from_ymd_opt(2026, 8, dia)
            .and_then(|d| d.and_hms_opt(hora, 0, 0))
            .expect("data válida");

        Local
            .from_local_datetime(&data)
            .single()
            .expect("horário sem ambiguidade")
            .timestamp_millis()
    }

    fn acao(verbo: &str, alvo: &str, hora: u32, dia: u32, ok: bool) -> Acao {
        Acao {
            quando: as_horas(hora, dia),
            acao: verbo.to_owned(),
            alvo: alvo.to_owned(),
            ok,
        }
    }

    #[test]
    fn tres_vezes_vira_habito_e_duas_nao() {
        let acoes = [
            acao("open_app", "spotify", 7, 1, true),
            acao("open_app", "spotify", 7, 2, true),
            acao("open_app", "spotify", 8, 3, true),
            // Duas vezes só: acaso, não rotina.
            acao("open_app", "discord", 20, 1, true),
            acao("open_app", "discord", 20, 2, true),
        ];

        let rotinas = agregar(&acoes);

        assert_eq!(rotinas.len(), 1);
        assert_eq!(rotinas[0].alvo, "spotify");
        assert_eq!(rotinas[0].periodo, Periodo::Manha);
        assert_eq!(rotinas[0].vezes, 3);
    }

    /// O que o usuário TENTOU não é o que ele faz — senão um nome errado repetido
    /// vira "rotina" e o Jarvis passa a sugerir o próprio erro.
    #[test]
    fn acao_que_falhou_nao_conta() {
        let acoes = [
            acao("open_app", "spotifi", 7, 1, false),
            acao("open_app", "spotifi", 7, 2, false),
            acao("open_app", "spotifi", 7, 3, false),
        ];

        assert!(agregar(&acoes).is_empty());
    }

    #[test]
    fn separa_o_mesmo_app_por_periodo_do_dia() {
        let manha: Vec<Acao> = (1..=3)
            .map(|d| acao("open_app", "code", 9, d, true))
            .collect();
        let noite: Vec<Acao> = (1..=4)
            .map(|d| acao("open_app", "code", 22, d, true))
            .collect();

        let rotinas = agregar(&[manha, noite].concat());

        assert_eq!(rotinas.len(), 2);
        // Mais frequente primeiro.
        assert_eq!(rotinas[0].periodo, Periodo::Noite);
        assert_eq!(rotinas[0].vezes, 4);
    }

    #[test]
    fn sem_habito_nao_gera_nota() {
        assert!(como_nota(&[]).is_none());

        let nota = como_nota(&agregar(&[
            acao("open_app", "spotify", 7, 1, true),
            acao("open_app", "spotify", 7, 2, true),
            acao("open_app", "spotify", 7, 3, true),
        ]))
        .expect("tem rotina");

        assert!(nota.contains("abre o spotify de manhã (3x)"));
    }
}
