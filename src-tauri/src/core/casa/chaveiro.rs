//! Onde as chaves dos aparelhos ficam depois que a nuvem as entregou.
//!
//! Um `casa.json` ao lado do `settings.json`, no molde do
//! [`crate::storage::json_store`]: arquivo ausente não é erro, BOM é tolerado, e a
//! gravação é um `to_string_pretty` — o arquivo é para ser legível por quem abrir.
//!
//! **Não vira campo de configuração.** Configuração é o que você escolhe; isto é dado
//! que o app descobriu. Misturar os dois faria uma lista de aparelhos aparecer na tela
//! de preferências.
//!
//! ## Duas fontes, uma ficha
//!
//! Cada aparelho é escrito por dois lados que não se sobrepõem: a **nuvem** dá nome,
//! chave, modelo e categoria; a **rede local** dá endereço, versão do protocolo e a hora
//! em que ele apareceu. Nenhum dos dois apaga o campo do outro — foi por isso que o
//! `guardar` deixou de substituir o arquivo inteiro. Um aparelho que a rede anuncia mas
//! que nunca foi importado tem ficha aqui do mesmo jeito, só que sem nome e sem chave.
//!
//! ## Sobre guardar segredo em texto puro
//!
//! A `local_key` fica legível no arquivo, como as chaves da Anthropic e do Spotify já
//! ficam no `settings.json`. É a política atual do projeto, e vale a mesma ressalva
//! escrita lá: o lugar certo é o keyring do sistema. A diferença é que esta chave só
//! serve dentro da sua rede, e o estrago de vazá-la é alguém acender sua luz.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::core::lock;
use crate::core::memory::normalizar;
use crate::storage::StorageError;

/// A ficha de um aparelho, com o que as duas fontes sabem dele.
///
/// `Default` existe para a nuvem poder preencher só a metade dela — os campos da rede
/// entram depois, no [`Chaveiro::vistos`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Conhecido {
    pub id: String,
    /// "Luz Cozinha", da nuvem. Vazio = ainda não importado, ou saiu da conta.
    pub nome: String,
    /// O segredo de controle, da nuvem. Muda a cada pareamento.
    pub local_key: String,
    pub produto: String,
    /// "dj" (luz), "cz" (tomada), "kg" (interruptor)…
    pub categoria: String,
    /// Se a nuvem o via no momento da importação. É um retrato velho; quem sabe do
    /// agora é o [`Self::visto_em`].
    pub online: bool,
    /// Onde a varredura o encontrou por último.
    pub ultimo_ip: String,
    /// O protocolo que a varredura ouviu ("3.3", "3.4", "3.5"). A nuvem não sabe disto:
    /// ela conhece o modelo, não o dialeto que o firmware fala.
    pub versao: String,
    /// Quando a rede o anunciou pela última vez, em ms. `0` = nunca.
    pub visto_em: i64,
}

const ARQUIVO: &str = "casa.json";

/// Os aparelhos que já sabemos abrir, e o disco onde eles moram.
///
/// Registrado com `app.manage` como capacidade própria, e não dentro do `AppState`:
/// é a regra do `lib.rs` — as configurações são de um dono, o que o app aprendeu é de
/// outro.
pub struct Chaveiro {
    path: PathBuf,
    /// Chaveado por id, que é como o anúncio da rede e a nuvem se referem ao mesmo
    /// aparelho. `BTreeMap` para o arquivo sair em ordem estável entre gravações — um
    /// `casa.json` que embaralha sozinho é ilegível no `git diff` de quem versiona a
    /// pasta de config.
    aparelhos: Mutex<BTreeMap<String, Conhecido>>,
}

impl Chaveiro {
    /// Lê o que houver no disco. Falha de leitura **não** impede o app de subir: sem
    /// chaveiro a Casa volta a ser só a lista da rede, que é um app pior mas inteiro.
    pub fn new(config_dir: &Path) -> Self {
        let path = config_dir.join(ARQUIVO);
        let aparelhos = carregar(&path).unwrap_or_else(|erro| {
            eprintln!("[jarvis] não consegui ler o {ARQUIVO} ({erro}); seguindo sem chaves");
            BTreeMap::new()
        });

        Self {
            path,
            aparelhos: Mutex::new(aparelhos),
        }
    }

    /// Grava o que a nuvem acabou de dizer, por cima do que ela tinha dito antes.
    ///
    /// **A nuvem manda nos campos da nuvem, e só neles.** Endereço, versão e hora do
    /// último anúncio são da rede local e sobrevivem intactos a uma importação — a
    /// alternativa, substituir o arquivo inteiro, apagaria justamente o que faz "apaga a
    /// luz da cozinha" funcionar sem varredura.
    ///
    /// Aparelho que saiu da conta perde nome e chave mas **mantém a ficha**: ele pode
    /// continuar anunciando na rede, e sumir da lista faria parecer defeito. O que não
    /// pode em hipótese alguma é a chave velha sobreviver — parear de novo troca a
    /// chave, e a antiga daria um erro de decifragem que não se parece com a causa.
    pub fn guardar(&self, da_nuvem: Vec<Conhecido>) -> Result<usize, StorageError> {
        let mut mapa = lock(&self.aparelhos);
        let vindos: BTreeSet<&str> = da_nuvem.iter().map(|novo| novo.id.as_str()).collect();

        for (id, ficha) in mapa.iter_mut() {
            if vindos.contains(id.as_str()) {
                continue;
            }
            ficha.nome.clear();
            ficha.local_key.clear();
        }

        let quantos = da_nuvem.len();
        for novo in da_nuvem {
            let ficha = mapa.entry(novo.id.clone()).or_insert_with(|| Conhecido {
                id: novo.id.clone(),
                ..Conhecido::default()
            });

            ficha.nome = novo.nome;
            ficha.local_key = novo.local_key;
            ficha.produto = novo.produto;
            ficha.categoria = novo.categoria;
            ficha.online = novo.online;
        }

        gravar(&self.path, &mapa)?;

        Ok(quantos)
    }

    /// O que sabemos sobre um aparelho, pelo id do anúncio.
    pub fn de(&self, id: &str) -> Option<Conhecido> {
        lock(&self.aparelhos).get(id).cloned()
    }

    pub fn todos(&self) -> Vec<Conhecido> {
        lock(&self.aparelhos).values().cloned().collect()
    }

    pub fn vazio(&self) -> bool {
        lock(&self.aparelhos).is_empty()
    }

    /// Anota o que a varredura acabou de ver, criando ficha para quem ainda não tinha.
    ///
    /// Duas coisas dependem disto. Uma é "apaga a luz da cozinha" não precisar pagar 10
    /// segundos de escuta antes do comando. A outra é o painel ter o que mostrar no
    /// instante em que abre — a lista some da tela a cada reinício se ninguém anotar
    /// quem já apareceu.
    ///
    /// **Cria ficha para aparelho que a nuvem não conhece.** Ele fica sem nome e sem
    /// chave, e ainda assim aparece na lista: existir na rede já é motivo suficiente.
    pub fn vistos(&self, vistos: &[(&str, &str, &str)]) {
        if vistos.is_empty() {
            return;
        }

        let mut mapa = lock(&self.aparelhos);
        let agora = Utc::now().timestamp_millis();

        for (id, ip, versao) in vistos {
            let ficha = mapa.entry((*id).to_owned()).or_insert_with(|| Conhecido {
                id: (*id).to_owned(),
                ..Conhecido::default()
            });

            ficha.ultimo_ip = (*ip).to_owned();
            ficha.versao = (*versao).to_owned();
            ficha.visto_em = agora;
        }

        // Grava toda vez, e não só quando o endereço muda: a hora do último anúncio muda
        // sempre, e é ela que diz se um aparelho ainda estava lá antes de o app fechar.
        // É um arquivo de poucos KB, escrito enquanto o painel está aberto.
        //
        // Falha de gravação não derruba a varredura: o que se perde é a memória entre
        // sessões, não a lista na tela.
        if let Err(erro) = gravar(&self.path, &mapa) {
            eprintln!("[jarvis] não consegui anotar a varredura no {ARQUIVO}: {erro}");
        }
    }

    /// Procura um aparelho pelo nome dito em voz alta.
    ///
    /// "apaga a luz da cozinha" tem que achar a "Luz Cozinha" sem que ninguém cadastre
    /// sinônimo: a regra é que TODAS as palavras do nome do aparelho apareçam na frase.
    /// O caminho inverso — palavras da frase presentes no nome — casaria "a luz" com
    /// qualquer luz da casa.
    pub fn achar_por_nome(&self, dito: &str) -> Busca {
        let frase = normalizar(dito);
        if frase.is_empty() {
            return Busca::Nenhum;
        }

        let palavras: Vec<&str> = frase.split(' ').collect();
        let mut candidatos: Vec<(usize, Conhecido)> = Vec::new();

        for aparelho in lock(&self.aparelhos).values() {
            let nome = normalizar(&aparelho.nome);
            if nome.is_empty() {
                continue;
            }

            let do_nome: Vec<&str> = nome.split(' ').collect();
            if do_nome
                .iter()
                .all(|palavra| palavras.contains(palavra))
            {
                candidatos.push((do_nome.len(), aparelho.clone()));
            }
        }

        // Nome mais específico ganha: com "Luz" e "Luz Cozinha" cadastrados, "apaga a
        // luz da cozinha" casa com os dois, e o certo é o de duas palavras.
        let Some(melhor) = candidatos.iter().map(|(quantas, _)| *quantas).max() else {
            return Busca::Nenhum;
        };
        candidatos.retain(|(quantas, _)| *quantas == melhor);

        match candidatos.len() {
            1 => Busca::Um(Box::new(candidatos.remove(0).1)),
            // Empate de verdade: perguntar é melhor que apagar a luz errada de alguém.
            _ => Busca::Varios(
                candidatos
                    .into_iter()
                    .map(|(_, aparelho)| aparelho.nome)
                    .collect(),
            ),
        }
    }
}

/// O resultado de procurar um aparelho por nome.
///
/// Três casos e não um `Option`: "não achei" e "achei três" pedem respostas diferentes
/// do assistente, e juntá-los num `None` faria ele dizer "não conheço esse aparelho"
/// quando o problema era o contrário.
pub enum Busca {
    Um(Box<Conhecido>),
    Nenhum,
    Varios(Vec<String>),
}

fn carregar(path: &Path) -> Result<BTreeMap<String, Conhecido>, StorageError> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let raw = fs::read_to_string(path)?;

    // Mesmo cuidado com o BOM do `json_store`: quem abre o arquivo no Bloco de Notas
    // para conferir uma chave e salva sem querer põe três bytes invisíveis na frente.
    Ok(serde_json::from_str(raw.trim_start_matches('\u{feff}'))?)
}

fn gravar(path: &Path, mapa: &BTreeMap<String, Conhecido>) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, serde_json::to_string_pretty(mapa)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jarvis-chaveiro-{nome}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn aparelho(id: &str, chave: &str) -> Conhecido {
        Conhecido {
            id: id.to_owned(),
            nome: format!("Luz {id}"),
            local_key: chave.to_owned(),
            produto: "key123".to_owned(),
            categoria: "dj".to_owned(),
            online: true,
            ..Conhecido::default()
        }
    }

    fn nomeado(id: &str, nome: &str) -> Conhecido {
        Conhecido {
            nome: nome.to_owned(),
            ..aparelho(id, "chave")
        }
    }

    #[test]
    fn sem_arquivo_comeca_vazio() {
        assert!(Chaveiro::new(&pasta("vazio")).vazio());
    }

    #[test]
    fn ida_e_volta_pelo_disco() {
        let dir = pasta("roundtrip");

        let quantos = Chaveiro::new(&dir)
            .guardar(vec![aparelho("abc", "chave1"), aparelho("def", "chave2")])
            .expect("grava");
        assert_eq!(quantos, 2);

        let relido = Chaveiro::new(&dir);
        assert_eq!(relido.de("abc").expect("achou").local_key, "chave1");
        assert_eq!(relido.todos().len(), 2);
    }

    /// A `local_key` muda a cada pareamento, e a velha **não pode sobreviver**: ela
    /// falharia com um erro de decifragem que não se parece nem um pouco com "você
    /// repareou o aparelho".
    ///
    /// Quem saiu da conta perde nome e chave mas mantém a ficha — ele pode continuar
    /// anunciando na rede, e sumir da lista faria procurar defeito no lugar errado.
    #[test]
    fn reimportar_troca_a_chave_e_desarma_quem_saiu_da_conta() {
        let dir = pasta("substitui");
        let chaveiro = Chaveiro::new(&dir);

        chaveiro
            .guardar(vec![aparelho("abc", "velha"), aparelho("sumiu", "x")])
            .expect("grava");
        chaveiro
            .guardar(vec![aparelho("abc", "nova")])
            .expect("regrava");

        assert_eq!(chaveiro.de("abc").expect("achou").local_key, "nova");

        let saiu = chaveiro.de("sumiu").expect("a ficha continua");
        assert!(saiu.local_key.is_empty(), "a chave de quem saiu tem que morrer");
        assert!(saiu.nome.is_empty());
    }

    /// Importar não pode apagar o que só a REDE sabe. Sem isto, cada importação zerava o
    /// endereço e "apaga a luz da cozinha" voltava a exigir uma varredura antes.
    #[test]
    fn importar_nao_apaga_o_que_veio_da_rede() {
        let chaveiro = Chaveiro::new(&pasta("duas-fontes"));

        chaveiro.vistos(&[("abc", "192.168.3.12", "3.3")]);
        chaveiro.guardar(vec![nomeado("abc", "Luz Cozinha")]).expect("grava");

        let ficha = chaveiro.de("abc").expect("achou");
        assert_eq!(ficha.nome, "Luz Cozinha", "a nuvem escreve o nome");
        assert_eq!(ficha.ultimo_ip, "192.168.3.12", "e não encosta no endereço");
        assert_eq!(ficha.versao, "3.3");
        assert!(ficha.visto_em > 0);
    }

    /// Aparelho que a rede anuncia mas que a nuvem nunca viu tem ficha do mesmo jeito —
    /// existir na rede já basta. É o que faz o painel lembrar dele entre sessões mesmo
    /// sem credencial nenhuma configurada.
    #[test]
    fn a_rede_cria_ficha_para_quem_a_nuvem_nao_conhece() {
        let dir = pasta("so-da-rede");
        Chaveiro::new(&dir).vistos(&[("novo", "192.168.3.26", "3.5")]);

        let ficha = Chaveiro::new(&dir).de("novo").expect("sobreviveu ao restart");
        assert_eq!(ficha.ultimo_ip, "192.168.3.26");
        assert!(ficha.nome.is_empty() && ficha.local_key.is_empty());
    }

    #[test]
    fn anota_o_endereco_visto_na_varredura() {
        let dir = pasta("vistos");
        let chaveiro = Chaveiro::new(&dir);
        chaveiro.guardar(vec![aparelho("abc", "chave")]).expect("grava");

        chaveiro.vistos(&[
            ("abc", "192.168.3.12", "3.3"),
            ("nao_existe", "10.0.0.1", "3.3"),
        ]);

        assert_eq!(chaveiro.de("abc").expect("achou").ultimo_ip, "192.168.3.12");
        assert_eq!(chaveiro.de("abc").expect("achou").versao, "3.3");
        // Sobrevive ao restart: é o que faz "apaga a luz" funcionar sem varredura.
        assert_eq!(
            Chaveiro::new(&dir).de("abc").expect("achou").ultimo_ip,
            "192.168.3.12"
        );
    }

    /// A regra é "todas as palavras do NOME aparecem na frase", e não o contrário — o
    /// inverso casaria "apaga a luz" com qualquer luz da casa.
    #[test]
    fn acha_o_aparelho_pelo_nome_dito() {
        let chaveiro = Chaveiro::new(&pasta("por-nome"));
        chaveiro
            .guardar(vec![
                nomeado("cozinha", "Luz Cozinha"),
                nomeado("sala", "Lâmpada da Sala"),
            ])
            .expect("grava");

        assert!(matches!(
            chaveiro.achar_por_nome("apaga a luz da cozinha"),
            Busca::Um(achado) if achado.id == "cozinha"
        ));
        // Acento e caixa não podem atrapalhar: quem fala não digita cedilha.
        assert!(matches!(
            chaveiro.achar_por_nome("acende a lampada da sala"),
            Busca::Um(achado) if achado.id == "sala"
        ));
        assert!(matches!(
            chaveiro.achar_por_nome("liga o ventilador do quarto"),
            Busca::Nenhum
        ));
    }

    /// Duas luzes e um pedido que serve para as duas: perguntar é melhor que apagar a
    /// luz errada de alguém.
    #[test]
    fn nome_ambiguo_pergunta_em_vez_de_chutar() {
        let chaveiro = Chaveiro::new(&pasta("ambiguo"));
        chaveiro
            .guardar(vec![
                nomeado("um", "Luz Cozinha"),
                nomeado("dois", "Luz Quarto"),
            ])
            .expect("grava");

        // "luz" sozinha não distingue as duas — e nenhum dos nomes cabe inteiro na frase.
        assert!(matches!(chaveiro.achar_por_nome("apaga a luz"), Busca::Nenhum));

        // Já uma frase que serve às duas tem que virar pergunta, não escolha às cegas.
        match chaveiro.achar_por_nome("apaga a luz da cozinha e do quarto") {
            Busca::Varios(nomes) => assert_eq!(nomes.len(), 2),
            _ => panic!("dois aparelhos cabem na frase; era para perguntar"),
        }
    }

    /// Nome mais específico ganha: com "Luz" e "Luz Cozinha" cadastrados, o pedido pela
    /// cozinha não pode cair na luz genérica.
    #[test]
    fn o_nome_mais_especifico_vence() {
        let chaveiro = Chaveiro::new(&pasta("especifico"));
        chaveiro
            .guardar(vec![nomeado("generica", "Luz"), nomeado("certa", "Luz Cozinha")])
            .expect("grava");

        assert!(matches!(
            chaveiro.achar_por_nome("apaga a luz da cozinha"),
            Busca::Um(achado) if achado.id == "certa"
        ));
    }

    /// Arquivo corrompido não pode impedir o app de subir — a Casa degrada para a
    /// lista da rede, que é o que ela já era antes das chaves existirem.
    #[test]
    fn arquivo_ilegivel_nao_derruba_o_app() {
        let dir = pasta("corrompido");
        fs::create_dir_all(&dir).expect("cria a pasta");
        fs::write(dir.join(ARQUIVO), "{ isto não é json").expect("escreve");

        assert!(Chaveiro::new(&dir).vazio());
    }
}
