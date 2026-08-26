//! Uma nota da memória: um arquivo `.md` com frontmatter e `[[links]]`.
//!
//! O formato é o do Obsidian de propósito — a memória do Jarvis é uma pasta que você
//! abre, lê, corrige e versiona. Isso muda o que ela é: em vez de um blob opaco que
//! só o modelo entende, é conhecimento que as duas pontas escrevem.
//!
//! ```markdown
//! ---
//! tipo: fato
//! atualizado: 2026-08-26
//! ---
//!
//! Acorda 6h30 e vai para a academia. Depois abre [[meu-jogo]].
//! ```
//!
//! Nada de parser de YAML: o frontmatter tem dois campos, ambos `chave: valor` numa
//! linha. Uma dependência para isso seria o cúmulo.

use serde::{Deserialize, Serialize};

/// Para que serve a nota. Decide onde ela é usada, não onde ela mora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tipo {
    /// Algo durável sobre o usuário. Vai para o prompt de conversa.
    Fato,
    /// `apelido = alvo`. Vai para o prompt do ROTEADOR — é o que faz "abre meu jogo"
    /// passar a funcionar depois de ensinado uma vez.
    Apelido,
    /// Padrão observado no log de ações. Escrito pelo próprio Jarvis.
    Rotina,
    /// Destilado de conversas que já saíram da janela do prompt.
    Resumo,
    /// Conhecimento sobre o MUNDO, trazido de uma busca. Separado de [`Tipo::Fato`]
    /// porque fato é sobre o usuário: misturar os dois faria o Jarvis falar de Nikola
    /// Tesla como se fosse uma coisa que você contou dele.
    Aprendido,
}

impl Tipo {
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Fato => "fato",
            Self::Apelido => "apelido",
            Self::Rotina => "rotina",
            Self::Resumo => "resumo",
            Self::Aprendido => "aprendido",
        }
    }

    fn do_texto(texto: &str) -> Self {
        match texto.trim() {
            "apelido" => Self::Apelido,
            "rotina" => Self::Rotina,
            "resumo" => Self::Resumo,
            "aprendido" => Self::Aprendido,
            _ => Self::Fato,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nota {
    /// Slug, sem extensão. É o alvo de `[[nome]]` e o nome do arquivo.
    pub nome: String,
    pub tipo: Tipo,
    /// `YYYY-MM-DD`. Serve para o usuário saber o que está velho.
    pub atualizado: String,
    pub corpo: String,
}

impl Nota {
    pub fn nova(nome: &str, tipo: Tipo, corpo: &str, hoje: &str) -> Self {
        Self {
            nome: slug(nome),
            tipo,
            atualizado: hoje.to_owned(),
            corpo: corpo.trim().to_owned(),
        }
    }

    pub fn para_markdown(&self) -> String {
        format!(
            "---\ntipo: {}\natualizado: {}\n---\n\n{}\n",
            self.tipo.como_texto(),
            self.atualizado,
            self.corpo
        )
    }

    /// Aceita arquivo sem frontmatter: o usuário vai criar nota na mão no Obsidian, e
    /// exigir cabeçalho faria a nota dele ser ignorada em silêncio.
    pub fn do_markdown(nome: &str, texto: &str) -> Self {
        let (cabecalho, corpo) = separar(texto);

        Self {
            nome: nome.to_owned(),
            tipo: campo(cabecalho, "tipo").map_or(Tipo::Fato, |v| Tipo::do_texto(&v)),
            atualizado: campo(cabecalho, "atualizado").unwrap_or_default(),
            corpo: corpo.trim().to_owned(),
        }
    }

    /// Os `[[alvos]]` citados no corpo. É o que transforma notas soltas em grafo: a
    /// busca puxa a nota que casou E os vizinhos dela.
    pub fn links(&self) -> Vec<String> {
        let mut alvos = Vec::new();
        let mut resto = self.corpo.as_str();

        while let Some(abre) = resto.find("[[") {
            resto = &resto[abre + 2..];
            let Some(fecha) = resto.find("]]") else { break };

            let alvo = resto[..fecha].trim();
            // `[[nota|apelido de exibição]]` é sintaxe do Obsidian; só o alvo importa.
            let alvo = alvo.split('|').next().unwrap_or(alvo).trim();
            if !alvo.is_empty() {
                alvos.push(slug(alvo));
            }

            resto = &resto[fecha + 2..];
        }

        alvos.sort();
        alvos.dedup();
        alvos
    }

    /// Uma linha para o índice `MEMORIA.md`.
    pub fn linha_do_indice(&self) -> String {
        let gancho: String = self
            .corpo
            .lines()
            .find(|linha| !linha.trim().is_empty())
            .unwrap_or_default()
            .chars()
            .take(100)
            .collect();

        format!(
            "- [[{}]] ({}) — {}",
            self.nome,
            self.tipo.como_texto(),
            gancho.trim()
        )
    }
}

fn separar(texto: &str) -> (&str, &str) {
    let texto = texto.trim_start_matches('\u{feff}');
    let Some(resto) = texto.strip_prefix("---") else {
        return ("", texto);
    };
    let Some(fim) = resto.find("\n---") else {
        return ("", texto);
    };

    (&resto[..fim], &resto[fim + 4..])
}

fn campo(cabecalho: &str, chave: &str) -> Option<String> {
    cabecalho.lines().find_map(|linha| {
        let (nome, valor) = linha.split_once(':')?;
        (nome.trim() == chave).then(|| valor.trim().to_owned())
    })
}

/// Nome de arquivo seguro E alvo de link estável.
///
/// Tira acento em vez de recusar: o usuário fala português, e "rotina da manhã" tem
/// que virar um arquivo sem virar `rotina-da-manh-.md`.
pub fn slug(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut hifen_pendente = false;

    for c in texto.trim().to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            outro => outro,
        };

        if c.is_ascii_alphanumeric() {
            if hifen_pendente && !saida.is_empty() {
                saida.push('-');
            }
            hifen_pendente = false;
            saida.push(c);
        } else {
            hifen_pendente = true;
        }
    }

    // Um nome vazio viraria um arquivo `.md` oculto e sem alvo de link.
    if saida.is_empty() {
        "nota".to_owned()
    } else {
        saida.chars().take(60).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta_pelo_markdown() {
        let nota = Nota::nova(
            "Rotina da Manhã",
            Tipo::Fato,
            "Acorda 6h30 e vai para a [[academia]].",
            "2026-08-26",
        );
        assert_eq!(nota.nome, "rotina-da-manha");

        let lida = Nota::do_markdown(&nota.nome, &nota.para_markdown());
        assert_eq!(lida, nota);
    }

    /// O usuário vai criar nota na mão no Obsidian, sem cabeçalho nenhum.
    #[test]
    fn nota_sem_frontmatter_continua_valendo() {
        let lida = Nota::do_markdown("solta", "Só o texto, escrito na mão.");

        assert_eq!(lida.tipo, Tipo::Fato);
        assert_eq!(lida.corpo, "Só o texto, escrito na mão.");
        assert!(lida.atualizado.is_empty());
    }

    /// Os links são o que fazem a busca virar grafo em vez de lista.
    #[test]
    fn extrai_os_links_inclusive_com_apelido_de_exibicao() {
        let nota = Nota::nova(
            "x",
            Tipo::Fato,
            "Abre [[meu-jogo]] e depois [[VS Code|o editor]]. De novo [[meu-jogo]].",
            "2026-08-26",
        );

        assert_eq!(nota.links(), ["meu-jogo", "vs-code"]);
        // Colchete aberto sem fechar não pode entrar em laço nem em pânico.
        assert!(
            Nota::nova("y", Tipo::Fato, "quebrado [[sem fim", "2026-08-26")
                .links()
                .is_empty()
        );
    }

    #[test]
    fn slug_nao_devolve_nome_vazio() {
        assert_eq!(slug("  ???  "), "nota");
        assert_eq!(slug("Meu Jogo!"), "meu-jogo");
        assert_eq!(slug("preço  do   dólar"), "preco-do-dolar");
    }
}
