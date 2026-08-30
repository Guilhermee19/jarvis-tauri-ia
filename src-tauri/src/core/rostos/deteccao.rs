//! Achar rostos numa imagem, e onde estão os olhos deles.
//!
//! Usa o **YuNet**, o detector do OpenCV Zoo: 220 KB de ONNX que roda em milissegundos na
//! CPU. Ele não diz QUEM é ninguém — isso é o [`super::embedding`]. O que ele entrega é a
//! caixa do rosto e cinco pontos (dois olhos, nariz, dois cantos da boca), e são os
//! pontos que importam tanto quanto a caixa: sem alinhar o rosto pelos olhos antes de
//! reconhecer, uma cabeça inclinada vira outra pessoa.
//!
//! ## A saída crua não é uma lista de rostos
//!
//! O modelo devolve **doze tensores**, e nenhum deles é "os rostos que eu achei". São
//! três escalas (passos de 8, 16 e 32 pixels) × quatro grandezas (classe, objetividade,
//! caixa, pontos), e cada escala é uma grade de células cobrindo a imagem inteira: 80×80,
//! 40×40 e 20×20 — 8400 candidatos no total, quase todos lixo.
//!
//! Transformar isso em rostos é o trabalho deste módulo, e são dois passos:
//!
//! 1. **Decodificar**: cada célula prevê um deslocamento em relação a si mesma, não uma
//!    coordenada absoluta. A caixa sai de `(coluna + dx) * passo`, e o tamanho de um
//!    exponencial — é assim que uma grade fixa consegue descrever rostos de qualquer
//!    tamanho.
//! 2. **Suprimir**: um rosto ativa várias células vizinhas, então ele chega repetido cinco
//!    ou seis vezes. A [`supressao`] mantém o candidato mais confiante e descarta quem se
//!    sobrepõe demais a ele.

use super::RostoError;

/// O lado da imagem que o modelo espera. Ele é quadrado, então a foto entra com barras.
pub const ENTRADA: usize = 640;

/// Abaixo disto o candidato é descartado antes mesmo da supressão.
///
/// Generoso de propósito: o custo de um falso positivo aqui é baixo (o embedding depois
/// não vai casar com ninguém), e o de perder o rosto é a saudação não acontecer.
const CONFIANCA: f32 = 0.6;

/// Quanto dois candidatos podem se sobrepor antes de virarem o mesmo rosto.
const SOBREPOSICAO: f32 = 0.3;

/// Um rosto encontrado, em coordenadas da imagem ORIGINAL.
#[derive(Debug, Clone, PartialEq)]
pub struct Rosto {
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
    pub confianca: f32,
    /// Os cinco pontos, em pares `(x, y)`: olho direito, olho esquerdo, nariz, canto
    /// direito da boca, canto esquerdo da boca. **"Direito" é o do MODELO** — na imagem
    /// ele aparece à esquerda, como num espelho.
    pub pontos: [(f32, f32); 5],
}

impl Rosto {
    /// Área da caixa. Usada para escolher o rosto principal quando há vários.
    pub fn area(&self) -> f32 {
        self.largura * self.altura
    }

    fn interseccao(&self, outro: &Self) -> f32 {
        let x1 = self.x.max(outro.x);
        let y1 = self.y.max(outro.y);
        let x2 = (self.x + self.largura).min(outro.x + outro.largura);
        let y2 = (self.y + self.altura).min(outro.y + outro.altura);

        ((x2 - x1).max(0.0)) * ((y2 - y1).max(0.0))
    }

    /// Interseção sobre união — o quanto duas caixas são a mesma caixa.
    fn iou(&self, outro: &Self) -> f32 {
        let comum = self.interseccao(outro);
        let uniao = self.area() + outro.area() - comum;

        if uniao <= 0.0 {
            0.0
        } else {
            comum / uniao
        }
    }
}

/// Como a imagem foi encaixada no quadrado de 640×640.
///
/// O modelo só aceita quadrado, e esticar a foto deformaria o rosto — que é justamente o
/// que o reconhecimento não perdoa. Então ela entra proporcional, centralizada, com
/// barras nas sobras; isto guarda o que foi feito para desfazer nas coordenadas.
#[derive(Debug, Clone, Copy)]
pub struct Encaixe {
    pub escala: f32,
    pub deslocamento_x: f32,
    pub deslocamento_y: f32,
}

impl Encaixe {
    pub fn novo(largura: u32, altura: u32) -> Self {
        let escala = (ENTRADA as f32 / largura as f32).min(ENTRADA as f32 / altura as f32);

        Self {
            escala,
            deslocamento_x: (ENTRADA as f32 - largura as f32 * escala) / 2.0,
            deslocamento_y: (ENTRADA as f32 - altura as f32 * escala) / 2.0,
        }
    }

    /// Leva um ponto do quadrado do modelo de volta para a imagem original.
    fn desfazer(&self, x: f32, y: f32) -> (f32, f32) {
        (
            (x - self.deslocamento_x) / self.escala,
            (y - self.deslocamento_y) / self.escala,
        )
    }
}

/// Uma escala da saída do modelo, já separada por grandeza.
pub struct Escala<'a> {
    pub passo: usize,
    pub cls: &'a [f32],
    pub obj: &'a [f32],
    pub bbox: &'a [f32],
    pub kps: &'a [f32],
}

/// Transforma os doze tensores em rostos, na coordenada da imagem original.
pub fn decodificar(escalas: &[Escala<'_>], encaixe: Encaixe) -> Vec<Rosto> {
    let mut candidatos = Vec::new();

    for escala in escalas {
        let colunas = ENTRADA / escala.passo;

        for indice in 0..escala.cls.len() {
            // A confiança é o produto das duas cabeças, e a raiz devolve a escala: sem
            // ela, dois valores altos (0,9 × 0,9) já cairiam para 0,81 e o limiar teria
            // que ser recalibrado para nada.
            let confianca = (escala.cls[indice] * escala.obj[indice]).max(0.0).sqrt();
            if confianca < CONFIANCA {
                continue;
            }

            let coluna = (indice % colunas) as f32;
            let linha = (indice / colunas) as f32;
            let passo = escala.passo as f32;

            // Cada célula prevê um DESLOCAMENTO em relação a si mesma. É o que permite
            // uma grade fixa descrever um rosto em qualquer posição.
            let caixa = &escala.bbox[indice * 4..indice * 4 + 4];
            let centro_x = (coluna + caixa[0]) * passo;
            let centro_y = (linha + caixa[1]) * passo;
            // Exponencial no tamanho: a rede prevê o logaritmo, que é o que deixa uma
            // mesma escala cobrir rostos de tamanhos bem diferentes sem saturar.
            let largura = caixa[2].exp() * passo;
            let altura = caixa[3].exp() * passo;

            let mut pontos = [(0.0, 0.0); 5];
            for (i, ponto) in pontos.iter_mut().enumerate() {
                let px = (coluna + escala.kps[indice * 10 + i * 2]) * passo;
                let py = (linha + escala.kps[indice * 10 + i * 2 + 1]) * passo;
                *ponto = encaixe.desfazer(px, py);
            }

            let (x, y) = encaixe.desfazer(centro_x - largura / 2.0, centro_y - altura / 2.0);

            candidatos.push(Rosto {
                x,
                y,
                largura: largura / encaixe.escala,
                altura: altura / encaixe.escala,
                confianca,
                pontos,
            });
        }
    }

    supressao(candidatos)
}

/// Mantém o candidato mais confiante de cada aglomerado.
///
/// Um rosto ativa várias células vizinhas e chega aqui repetido. Sem isto, uma pessoa
/// viraria seis rostos — e o reconhecimento rodaria seis vezes sobre a mesma cara.
fn supressao(mut candidatos: Vec<Rosto>) -> Vec<Rosto> {
    // `total_cmp` e não `partial_cmp().unwrap()`: um `NaN` vindo do modelo entraria em
    // pânico no meio de uma saudação, que é o pior lugar para derrubar o app.
    candidatos.sort_by(|a, b| b.confianca.total_cmp(&a.confianca));

    let mut mantidos: Vec<Rosto> = Vec::new();
    for candidato in candidatos {
        if mantidos
            .iter()
            .all(|mantido| mantido.iou(&candidato) < SOBREPOSICAO)
        {
            mantidos.push(candidato);
        }
    }

    mantidos
}

/// O rosto principal da cena: o maior.
///
/// **Maior, e não o mais confiante.** Quem está usando o computador é quem está perto da
/// câmera; alguém passando ao fundo pode aparecer com confiança alta e não é a pessoa que
/// o Jarvis quer cumprimentar.
pub fn principal(rostos: Vec<Rosto>) -> Result<Rosto, RostoError> {
    rostos
        .into_iter()
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .ok_or(RostoError::NenhumRosto)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rosto(x: f32, y: f32, lado: f32, confianca: f32) -> Rosto {
        Rosto {
            x,
            y,
            largura: lado,
            altura: lado,
            confianca,
            pontos: [(0.0, 0.0); 5],
        }
    }

    /// Uma imagem deitada tem que entrar proporcional e centralizada — esticá-la
    /// deformaria o rosto, que é o que o reconhecimento menos perdoa.
    #[test]
    fn o_encaixe_centraliza_sem_deformar() {
        let encaixe = Encaixe::novo(1280, 720);

        assert!((encaixe.escala - 0.5).abs() < f32::EPSILON);
        assert_eq!(encaixe.deslocamento_x, 0.0);
        // 640 - 720*0.5 = 280, metade em cima e metade embaixo.
        assert_eq!(encaixe.deslocamento_y, 140.0);
    }

    /// O que o encaixe faz, ele tem que desfazer: um ponto no centro do quadrado é o
    /// centro da foto original.
    #[test]
    fn desfazer_e_o_inverso_do_encaixe() {
        let encaixe = Encaixe::novo(1280, 720);
        let (x, y) = encaixe.desfazer(320.0, 320.0);

        assert!((x - 640.0).abs() < 0.01);
        assert!((y - 360.0).abs() < 0.01);
    }

    /// Duas caixas iguais são o mesmo rosto; duas distantes não são.
    #[test]
    fn iou_mede_sobreposicao() {
        let a = rosto(0.0, 0.0, 10.0, 0.9);
        assert!((a.iou(&a) - 1.0).abs() < f32::EPSILON);

        let longe = rosto(100.0, 100.0, 10.0, 0.9);
        assert_eq!(a.iou(&longe), 0.0);

        // Metade sobreposta: comum 50, união 150.
        let meio = rosto(5.0, 0.0, 10.0, 0.9);
        assert!((a.iou(&meio) - (50.0 / 150.0)).abs() < 0.001);
    }

    /// O mesmo rosto detectado por células vizinhas tem que virar UM.
    #[test]
    fn a_supressao_junta_o_mesmo_rosto() {
        let quase_iguais = vec![
            rosto(10.0, 10.0, 50.0, 0.90),
            rosto(11.0, 11.0, 50.0, 0.95),
            rosto(12.0, 10.0, 50.0, 0.85),
        ];

        let mantidos = supressao(quase_iguais);
        assert_eq!(mantidos.len(), 1);
        // O mais confiante é quem sobra.
        assert!((mantidos[0].confianca - 0.95).abs() < f32::EPSILON);
    }

    /// Duas pessoas distantes na cena continuam sendo duas.
    #[test]
    fn a_supressao_preserva_rostos_distintos() {
        let dois = vec![rosto(0.0, 0.0, 40.0, 0.9), rosto(200.0, 0.0, 40.0, 0.8)];

        assert_eq!(supressao(dois).len(), 2);
    }

    /// Quem usa o computador está PERTO da câmera. Alguém passando ao fundo pode ter
    /// confiança maior e não é quem o Jarvis quer cumprimentar.
    #[test]
    fn o_principal_e_o_maior_e_nao_o_mais_confiante() {
        let cena = vec![
            rosto(0.0, 0.0, 30.0, 0.99),    // ao fundo, nítido
            rosto(50.0, 50.0, 120.0, 0.80), // na frente, grande
        ];

        let escolhido = principal(cena).unwrap();
        assert_eq!(escolhido.largura, 120.0);
    }

    #[test]
    fn cena_vazia_e_erro_e_nao_panico() {
        assert!(matches!(principal(vec![]), Err(RostoError::NenhumRosto)));
    }

    /// Uma célula com deslocamento zero e tamanho zero cai exatamente no centro dela.
    #[test]
    fn decodifica_a_celula_no_lugar_certo() {
        // Grade de passo 32: 20×20 = 400 células. A de índice 0 é o canto superior
        // esquerdo; com dx=dy=0 seu centro fica em (0,0).
        let cls = vec![1.0; 400];
        let obj = vec![1.0; 400];
        let mut bbox = vec![0.0; 400 * 4];
        // Célula 21 = linha 1, coluna 1. Centro em (1*32, 1*32) = (32, 32).
        bbox[21 * 4 + 2] = 0.0_f32; // exp(0) * 32 = 32 de largura
        bbox[21 * 4 + 3] = 0.0_f32;
        let kps = vec![0.0; 400 * 10];

        // Confiança só na célula 21.
        let mut cls_um = vec![0.0; 400];
        cls_um[21] = 1.0;

        let escalas = [Escala {
            passo: 32,
            cls: &cls_um,
            obj: &obj,
            bbox: &bbox,
            kps: &kps,
        }];

        // Imagem já quadrada de 640: escala 1, sem deslocamento.
        let achados = decodificar(&escalas, Encaixe::novo(640, 640));
        assert_eq!(achados.len(), 1);

        let r = &achados[0];
        // Centro (32,32) menos metade de 32 → canto em (16,16).
        assert!((r.x - 16.0).abs() < 0.01, "x = {}", r.x);
        assert!((r.y - 16.0).abs() < 0.01, "y = {}", r.y);
        assert!((r.largura - 32.0).abs() < 0.01);

        let _ = cls;
    }

    /// Candidato fraco não pode virar rosto — senão o embedding roda sobre uma parede.
    #[test]
    fn descarta_o_que_esta_abaixo_do_limiar() {
        let cls = vec![0.1; 400];
        let obj = vec![0.1; 400];
        let bbox = vec![0.0; 400 * 4];
        let kps = vec![0.0; 400 * 10];

        let escalas = [Escala {
            passo: 32,
            cls: &cls,
            obj: &obj,
            bbox: &bbox,
            kps: &kps,
        }];

        assert!(decodificar(&escalas, Encaixe::novo(640, 640)).is_empty());
    }
}
