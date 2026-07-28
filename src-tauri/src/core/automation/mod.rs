//! PLACEHOLDER — controle do computador (v0.4).
//!
//! - `input.rs`: mouse e teclado via crate `enigo`.
//! - `screen.rs`: captura de tela via crate `xcap`, para o modelo enxergar a tela.
//!
//! Não é exposto ao frontend como comando: quem chama é `core::agent`, como
//! implementação das tools que o modelo pode invocar. Toda ação daqui é
//! potencialmente destrutiva, então este é o módulo onde entram as travas
//! (confirmação do usuário, allowlist de apps, kill switch).
