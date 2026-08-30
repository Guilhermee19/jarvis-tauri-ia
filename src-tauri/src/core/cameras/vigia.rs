//! Vigiar uma câmera e avisar quando algo se mexer.
//!
//! ## O portão barato antes do caro
//!
//! A ideia ingênua é mandar cada quadro ao modelo de visão e perguntar "tem alguém aí?".
//! Não funciona: o modelo é o mesmo que interpreta os comandos, ele demora segundos por
//! imagem, e ocupar a GPU a cada quadro deixaria o assistente surdo enquanto vigia.
//!
//! Então há dois estágios. O primeiro é aritmética pura: reduzir o quadro a uma
//! miniatura em tons de cinza e comparar com a anterior. Custa microssegundos e derruba
//! 99% dos quadros — uma cena parada é parada. **Só quando essa diferença estoura o
//! limiar** é que o modelo é chamado, e aí ele responde a pergunta que interessa: mexeu,
//! mas foi gente ou foi a árvore?
//!
//! ## O modo de falha caro é o alarme falso
//!
//! Um vigia que avisa demais é desligado no segundo dia, e aí não avisa nunca. Por isso o
//! [`Vigilancia::deve_olhar`] tem duas travas além do limiar: o **primeiro quadro nunca
//! alerta** (não há com o que comparar, e a assinatura inicial dispararia tudo), e existe
//! um **descanso** por câmera depois de cada alerta — uma pessoa atravessando o quintal
//! gera movimento por vários quadros seguidos, e isso é um evento, não seis.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use image::imageops::FilterType;

/// De quanto em quanto tempo cada câmera vigiada é olhada.
///
/// Quatro segundos porque o primeiro estágio é barato: um JPEG do go2rtc mais uma
/// redução para 64×48. O que custa é o segundo estágio, e ele é raro por construção.
/// Muito mais curto que isto e uma pessoa atravessando o quadro apareceria em dois
/// quadros seguidos; muito mais longo e ela passa entre um e outro.
pub const INTERVALO: Duration = Duration::from_secs(4);

/// O tamanho da miniatura que vira assinatura do quadro.
///
/// Pequena de propósito. O que se quer detectar é "uma região da cena mudou", e nessa
/// escala o ruído do sensor e a compressão JPEG somem sozinhos — numa resolução maior
/// eles viram movimento, e o vigia dispara com a câmera olhando para uma parede.
const LARGURA: u32 = 64;
const ALTURA: u32 = 48;

/// Quanto a cena precisa mudar, na escala 0–1, para valer uma olhada do modelo.
///
/// Calibrado para o ruído: uma cena parada de câmera IP fica na casa de 0,005–0,015 por
/// causa do grão do sensor e do rebalanceamento de exposição. 0,06 passa longe disso e
/// ainda pega uma pessoa entrando no quadro.
const LIMIAR: f32 = 0.06;

/// Quanto tempo uma câmera fica quieta depois de alertar.
///
/// Uma pessoa atravessando o quintal produz movimento por vários quadros seguidos. Sem
/// isto, um evento vira seis avisos e o usuário desliga o recurso.
const DESCANSO: Duration = Duration::from_secs(90);

#[derive(Debug, thiserror::Error)]
pub enum VigiaError {
    #[error("não consegui ler a imagem da câmera: {0}")]
    Imagem(String),
}

/// A miniatura em tons de cinza que representa um quadro.
///
/// Guardada em vez do quadro inteiro porque é ela que a comparação usa, e porque manter
/// um JPEG de 1080p por câmera na memória para sempre não se justifica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assinatura(Vec<u8>);

/// Reduz um JPEG à sua assinatura.
///
/// `Triangle` e não `Nearest`: a média dos pixels vizinhos é justamente o que faz o
/// ruído de sensor desaparecer na redução, e é ela que separa "mexeu" de "granulou".
pub fn assinatura(jpeg: &[u8]) -> Result<Assinatura, VigiaError> {
    let imagem =
        image::load_from_memory(jpeg).map_err(|erro| VigiaError::Imagem(erro.to_string()))?;

    let miniatura = imagem
        .resize_exact(LARGURA, ALTURA, FilterType::Triangle)
        .to_luma8();

    Ok(Assinatura(miniatura.into_raw()))
}

/// O quanto duas assinaturas diferem, de 0 (idênticas) a 1 (opostas).
///
/// Diferença média absoluta por pixel. Não é a métrica mais sofisticada que existe, e é
/// a certa aqui: ela não precisa dizer O QUE mudou — só se vale acordar o modelo, que é
/// quem sabe olhar.
///
/// Assinaturas de tamanhos diferentes devolvem 0. Isso só acontece se a câmera trocar de
/// resolução no meio, e nesse caso "não alerte" é o palpite seguro: o outro caminho é um
/// alarme falso garantido a cada mudança de perfil.
pub fn diferenca(antes: &Assinatura, agora: &Assinatura) -> f32 {
    if antes.0.len() != agora.0.len() || antes.0.is_empty() {
        return 0.0;
    }

    let soma: u64 = antes
        .0
        .iter()
        .zip(agora.0.iter())
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();

    // Dividido por 255 para a escala virar 0–1 e o limiar não depender da profundidade
    // de bits da imagem.
    soma as f32 / (antes.0.len() as f32 * 255.0)
}

/// O que fazer com o quadro que acabou de chegar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Veredito {
    /// Cena parada, ou câmera em descanso. Nada a fazer — o caso comum.
    Nada,
    /// Mexeu o bastante. Vale pagar a olhada do modelo.
    Olhar,
}

/// A memória do vigia: o último quadro e o último alerta de cada câmera.
///
/// Um estado só para todas as câmeras, chaveado por id, em vez de um vigia por câmera:
/// o descanso é por câmera, mas a política é uma só, e espalhá-la faria cada câmera
/// poder divergir em silêncio.
#[derive(Default)]
pub struct Vigilancia {
    ultimo: HashMap<String, Assinatura>,
    alertado_em: HashMap<String, Instant>,
}

impl Vigilancia {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra o quadro e diz se ele merece a atenção do modelo.
    ///
    /// **O primeiro quadro de uma câmera nunca alerta.** Não há com o que comparar, e
    /// tratar "sem anterior" como mudança faria toda câmera disparar ao ser ligada — que
    /// é exatamente quando ninguém está olhando para julgar o falso positivo.
    pub fn deve_olhar(&mut self, camera: &str, quadro: Assinatura) -> Veredito {
        let anterior = self.ultimo.insert(camera.to_owned(), quadro.clone());

        let Some(anterior) = anterior else {
            return Veredito::Nada;
        };

        if diferenca(&anterior, &quadro) < LIMIAR {
            return Veredito::Nada;
        }

        // Mexeu — mas a câmera pode estar de molho depois do alerta anterior. O quadro
        // acima JÁ foi guardado: descansar não é ficar cego, é ficar calado, e a
        // comparação da próxima vez tem que ser com o que se viu agora.
        if self.descansando(camera) {
            return Veredito::Nada;
        }

        self.alertado_em.insert(camera.to_owned(), Instant::now());
        Veredito::Olhar
    }

    fn descansando(&self, camera: &str) -> bool {
        self.alertado_em
            .get(camera)
            .is_some_and(|quando| quando.elapsed() < DESCANSO)
    }

    /// Esquece tudo que não está mais sendo vigiado.
    ///
    /// Não é só higiene de memória. **Uma câmera que volta a ser vigiada tem que começar
    /// do zero**: sem isto, ela seria comparada com o quadro de horas atrás — de outra
    /// hora do dia, com outra luz — e o primeiro quadro da volta viraria um alarme falso
    /// garantido, justamente no momento em que o usuário acabou de ligar a vigilância e
    /// está julgando se ela presta.
    ///
    /// Recebe a lista do que fica, e não o que sai: quem chama tem em mãos as câmeras
    /// vigiadas de agora, e derivar "quem saiu" no chamador seria pedir a mesma conta em
    /// todo lugar que usar isto.
    pub fn reter(&mut self, vigiadas: &[String]) {
        self.ultimo
            .retain(|id, _| vigiadas.iter().any(|vivo| vivo == id));
        self.alertado_em
            .retain(|id, _| vigiadas.iter().any(|vivo| vivo == id));
    }
}

/// Trava que garante **um** laço de vigilância por execução do app.
///
/// A ronda é iniciada de dentro do `start_cameras`, que é chamado toda vez que a janela
/// de câmeras abre e a cada cadastro. Sem esta trava, abrir e fechar a janela cinco vezes
/// deixaria cinco laços vivos — cada um pedindo quadros e acordando o modelo por conta
/// própria, com o sintoma de "as notificações vieram cinco vezes".
///
/// Registrado com `app.manage`, como os outros estados de capacidade.
#[derive(Default)]
pub struct Sentinela {
    ligado: AtomicBool,
}

impl Sentinela {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` na PRIMEIRA chamada, `false` em todas as seguintes.
    ///
    /// `swap` e não um par ler-depois-escrever: duas chamadas simultâneas passariam as
    /// duas por um `if !ligado { ligado = true }`, que é exatamente o que isto evita.
    pub fn ligar_uma_vez(&self) -> bool {
        !self.ligado.swap(true, Ordering::SeqCst)
    }
}

/// A pergunta que o modelo responde quando o movimento passou do limiar.
///
/// Pede um veredito curto e **manda dizer "nada" explicitamente**: sem essa saída, um
/// modelo pequeno descreve a cena vazia com entusiasmo ("uma garagem tranquila ao
/// entardecer") e todo movimento de galho vira notificação.
pub const PERGUNTA: &str = "Algo se moveu nesta cena. Há uma pessoa, um animal ou um veículo \
    visível? Se houver, diga em poucas palavras o que é e onde está. Se não houver nada disso \
    — só sombra, luz mudando, chuva ou vegetação ao vento —, responda exatamente: nada.";

/// Se a resposta do modelo merece virar aviso.
///
/// O modelo responde em texto livre mesmo tendo sido instruído; esta é a rede de
/// segurança. Comparação por prefixo normalizado, porque "Nada." e "nada" são a mesma
/// resposta e um `==` cru deixaria a pontuação virar notificação.
pub fn vale_avisar(resposta: &str) -> bool {
    let limpo = crate::core::memory::normalizar(resposta);
    let limpo = limpo.trim_end_matches(['.', '!', ' ']).trim();

    !limpo.is_empty() && limpo != "nada"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assinatura_de(valor: u8) -> Assinatura {
        Assinatura(vec![valor; (LARGURA * ALTURA) as usize])
    }

    /// Metade dos pixels indo de 0 a 255 é meia diferença — a âncora da escala.
    #[test]
    fn a_diferenca_vai_de_zero_a_um() {
        let preto = assinatura_de(0);
        let branco = assinatura_de(255);

        assert_eq!(diferenca(&preto, &preto), 0.0);
        assert!((diferenca(&preto, &branco) - 1.0).abs() < f32::EPSILON);

        let mut metade = preto.0.clone();
        for pixel in metade.iter_mut().take((LARGURA * ALTURA / 2) as usize) {
            *pixel = 255;
        }
        assert!((diferenca(&preto, &Assinatura(metade)) - 0.5).abs() < 0.01);
    }

    /// Tamanhos diferentes só acontecem se a câmera trocar de resolução no meio, e aí
    /// "não alerte" é o palpite seguro.
    #[test]
    fn tamanhos_diferentes_nao_alertam() {
        assert_eq!(
            diferenca(&assinatura_de(0), &Assinatura(vec![255; 10])),
            0.0
        );
    }

    /// Toda câmera dispararia ao ser ligada — justo quando ninguém está olhando para
    /// julgar o falso positivo.
    #[test]
    fn o_primeiro_quadro_nunca_alerta() {
        let mut vigia = Vigilancia::new();
        assert_eq!(
            vigia.deve_olhar("garagem", assinatura_de(0)),
            Veredito::Nada
        );
    }

    #[test]
    fn cena_parada_nao_acorda_o_modelo() {
        let mut vigia = Vigilancia::new();
        vigia.deve_olhar("garagem", assinatura_de(100));

        // O mesmo quadro de novo: nada mudou.
        assert_eq!(
            vigia.deve_olhar("garagem", assinatura_de(100)),
            Veredito::Nada
        );
    }

    #[test]
    fn mudanca_grande_acorda_o_modelo() {
        let mut vigia = Vigilancia::new();
        vigia.deve_olhar("garagem", assinatura_de(0));

        assert_eq!(
            vigia.deve_olhar("garagem", assinatura_de(255)),
            Veredito::Olhar
        );
    }

    /// Um evento vira seis avisos sem o descanso, e o usuário desliga o recurso.
    #[test]
    fn depois_de_alertar_a_camera_descansa() {
        let mut vigia = Vigilancia::new();
        vigia.deve_olhar("garagem", assinatura_de(0));
        assert_eq!(
            vigia.deve_olhar("garagem", assinatura_de(255)),
            Veredito::Olhar
        );

        // Continua mexendo muito, e mesmo assim fica calado.
        assert_eq!(
            vigia.deve_olhar("garagem", assinatura_de(0)),
            Veredito::Nada
        );
    }

    /// Descansar é ficar calado, não ficar cego: o quadro do descanso tem que ser
    /// guardado, senão a comparação seguinte usaria uma cena velha e alertaria de novo.
    #[test]
    fn o_descanso_ainda_guarda_o_quadro() {
        let mut vigia = Vigilancia::new();
        vigia.deve_olhar("garagem", assinatura_de(0));
        vigia.deve_olhar("garagem", assinatura_de(255));

        vigia.deve_olhar("garagem", assinatura_de(128));
        assert_eq!(vigia.ultimo.get("garagem"), Some(&assinatura_de(128)));
    }

    /// Parar e voltar a vigiar tem que começar do zero: comparar com o quadro de horas
    /// atrás (outra luz, outra hora do dia) daria um alarme falso garantido na volta.
    #[test]
    fn parar_de_vigiar_esquece_o_quadro() {
        let mut vigia = Vigilancia::new();
        vigia.deve_olhar("garagem", assinatura_de(0));

        // Saiu da lista de vigiadas.
        vigia.reter(&[]);

        // De volta: este é de novo um PRIMEIRO quadro, e primeiro quadro não alerta.
        assert_eq!(
            vigia.deve_olhar("garagem", assinatura_de(255)),
            Veredito::Nada
        );
    }

    /// Reter não pode derrubar quem continua na lista.
    #[test]
    fn reter_preserva_quem_segue_vigiado() {
        let mut vigia = Vigilancia::new();
        vigia.deve_olhar("garagem", assinatura_de(0));
        vigia.deve_olhar("quintal", assinatura_de(0));

        vigia.reter(&["garagem".to_owned()]);

        // A garagem manteve o quadro anterior, então a mudança é vista.
        assert_eq!(
            vigia.deve_olhar("garagem", assinatura_de(255)),
            Veredito::Olhar
        );
        // O quintal foi esquecido: volta a ser primeiro quadro.
        assert_eq!(
            vigia.deve_olhar("quintal", assinatura_de(255)),
            Veredito::Nada
        );
    }

    /// O descanso é por câmera: o quintal não pode ficar mudo porque a garagem alertou.
    #[test]
    fn o_descanso_nao_vaza_entre_cameras() {
        let mut vigia = Vigilancia::new();
        vigia.deve_olhar("garagem", assinatura_de(0));
        vigia.deve_olhar("garagem", assinatura_de(255));

        vigia.deve_olhar("quintal", assinatura_de(0));
        assert_eq!(
            vigia.deve_olhar("quintal", assinatura_de(255)),
            Veredito::Olhar
        );
    }

    /// Sem esta rede, "uma garagem tranquila ao entardecer" vira notificação.
    #[test]
    fn nada_em_qualquer_forma_nao_vira_aviso() {
        assert!(!vale_avisar("nada"));
        assert!(!vale_avisar("Nada."));
        assert!(!vale_avisar("  NADA  "));
        assert!(!vale_avisar(""));

        assert!(vale_avisar("uma pessoa de moletom perto do portão"));
    }
}
