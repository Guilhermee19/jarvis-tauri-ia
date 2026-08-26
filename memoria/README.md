# Memória do Jarvis

Esta pasta é o que o Jarvis sabe. **Abra ela no Obsidian** — o formato é o dele, com
frontmatter e `[[links]]` entre notas.

A memória tem dois donos. Ele escreve aqui quando aprende algo; você lê, corrige o que
ficou errado e apaga o que não quer que ele saiba. Editar um arquivo daqui muda o que
ele responde na próxima mensagem, sem reiniciar nada.

## O que é o quê

|                   |                                                                       |
| ----------------- | --------------------------------------------------------------------- |
| `MEMORIA.md`      | Índice, uma linha por nota. **Gerado** — não adianta editar.          |
| `notas/*.md`      | O conhecimento. É aqui que você mexe.                                 |
| `historico.jsonl` | Estado da UI: id, papel e hora exatos para redesenhar as bolhas.      |
| `acoes.jsonl`     | Log cru de tudo que ele executou. Vira `notas/rotinas-observadas.md`. |
| `estado.json`     | Marcador de até onde a conversa já virou resumo.                      |

**Isto não é o transcrito da conversa.** Cada nota é um documento sobre um assunto, que
cresce quando o assunto volta a aparecer — não uma ata do que foi dito. O que foi dito
literalmente fica no `historico.jsonl`, que existe só para a tela redesenhar as bolhas.

Os três últimos são estado de máquina, não conhecimento — o Obsidian ignora, e o
`.gitignore` daqui também.

## Como uma nota é

```markdown
---
tipo: fato
atualizado: 2026-08-26
---

Acorda 6h30 e vai para a [[academia]].
```

`tipo` muda onde a nota é usada:

- **fato** — entra no prompt de conversa quando vier ao caso.
- **apelido** — entra no prompt do ROTEADOR. Corpo no formato `apelido = alvo`. É o que
  faz "abre meu jogo" funcionar depois de ensinado uma vez.
- **rotina** — escrito por ele a partir do `acoes.jsonl`. Regravado, então editar não
  adianta.
- **resumo** — destilado das conversas que já saíram da janela do prompt.

Nota sem frontmatter também vale — o `tipo` cai em `fato`. Escreva à mão à vontade.

## Como ele escolhe o que lembrar na hora

Não é embedding. Ele casa palavra-chave com o que você disse e **segue um salto de
`[[link]]`**: perguntar "que horas eu acordo" acha `rotina-da-manha`, e como aquela nota
cita `[[academia]]`, a academia vem junto sem ter sido mencionada. É por isso que linkar
as notas entre si melhora as respostas — inclusive os links que você escrever na mão.

## Dois caminhos para guardar

- **Explícito**: "lembra que eu acordo 6h30". Passa pelo roteador e é confiável.
- **Automático**: ele tenta extrair fatos duráveis do que você conta. É _best-effort_ —
  calibrado para errar para menos, porque memória com furo você conserta com uma frase,
  e memória com lixo você tem que vir catar aqui.

Para apagar: "esquece a academia", ou delete o arquivo.
