# Restyle gwm — spec aprovada (mockups fechados com o usuário)

Data: 2026-07-24 · Referência: kbrdn1/gwm-cli (ratatui) · Status: APROVADO, implementar.
Pré-requisito: sistema de diálogos unificado (Dialog) já entregue na branch.

## Regras transversais (valem em TODA a interface)

1. **Teclas sempre em ACCENT + BOLD, rótulos em MUTED**, pares separados por ` · ` —
   na barra de status, nos rodapés de diálogo (helper no próprio Dialog), no corpo do
   modal de ajuda e nas bordas dos cartões de comentário.
2. **Seleção de lista** (picker, menus de diálogo): `▶ ` + bg `theme::SELECTION` + BOLD
   na linha inteira (pad até a largura interna). Não selecionado: dois espaços, sem bg.
3. **Chips reverse-video**: exatamente dois no header (nome do app em ACCENT
   REVERSED|BOLD; versão à direita em MUTED REVERSED). Nunca em outro lugar.
4. **Sem DIM**: de-ênfase sempre por cor (MUTED) — já é o padrão do projeto.
5. Bordas de painel: ACCENT quando focado, BORDER quando não (já existe — manter).

## Tela 1 — Picker (header/status sem borda; lista em painel bordado)

```
 betterreview  owner/repo                                               v0.1

╭ Reviews abertos ─────────────────────────────────────────────────────────╮
│ ▶ #42 fix: corrige restauração da sessão   você     feature/fix   2h  ●  │
│   #40 feat: seletor com prefetch           @maria   feat/picker   5h     │
│                                                        4 reviews abertos │
╰──────────────────────────────────────────────────────────────────────────╯

 ⠹ baixando #42…                     j/k mover · Enter abrir · r recarregar · q sair
```

- Header: chip `betterreview` + repositório (MUTED) + chip versão à direita (versão do
  Cargo via `env!("CARGO_PKG_VERSION")`). Sem borda.
- Painel: BorderType::Rounded, título ` Reviews abertos `, borda ACCENT.
- Linha: número BOLD, título FG (truncado com `…`), autor MUTED (`você` p/ o próprio,
  senão `@login`), branch MUTED, idade MUTED; badges à direita: `●` ACCENT (branch
  atual), `sessão` WARNING, `draft` MUTED.
- Contador inferior direito dentro do painel: `N reviews abertos` /
  `50 mais recentes` (MUTED).
- Status: spinner+estado do prefetch à esquerda (ACCENT), hints planos à direita
  (regra 1). Erro em DANGER substitui a linha toda.

## Tela 2 — Review (header chip + barra de status planos)

- Linha de título atual vira: chip `betterreview` + ` owner/repo #42 · título · @autor `
  (MUTED) + chip versão à direita.
- Rodapé de atalhos atual (linha 4) é REMOVIDO; os hints migram para a barra de status
  (linha 3) no formato flat: à esquerda spinner/notice/erro (precedência atual), à
  direita hints `j/k · ]h hunk · ]c comentário · / buscar · ? ajuda` truncando com `…`
  quando faltar espaço (mensagem de status tem prioridade). O layout vertical passa a
  ter 3 linhas (header, corpo, status) — atualizar constraints e testes.

## Tela 3 — Cartões de comentário com caixa completa

```
      ╭─ @você · draft ─────────────────────╮
      │ corpo do comentário                 │
      ╰─ e editar · x excluir ──────────────╯
```

- `src/app/display.rs`: `push_block` passa a emitir borda superior (row própria com a
  meta), corpo, e borda inferior (row própria com as teclas). Trocar
  `block_start: bool` por `kind: CommentRowKind { Header, Body, Footer }` com helper
  `is_block_start(kind) == Header` para navegação/`]c` (atualizar reducer, testes de
  display_rows/navegação e os testes de render — os saltos param no Header).
- Widget: Header `╭─ @autor · marcador ─…─╮` (autor ACCENT, marcador WARNING p/ draft /
  `✓` SUCCESS p/ resolvida / `comentário` MUTED); Body `│ texto …pad… │`; Footer
  `╰─ teclas ─…─╯` (teclas na regra 1; draft: `e editar · x excluir`; thread:
  `r responder`). Largura = largura interna do painel. Highlight de bloco inteiro
  (cursor em qualquer row do bloco) já existe — manter.
- Órfãos: grupo continua com o header `— comentários desatualizados —`.

## Tela 4 — Ajuda colorida

Corpo do modal de ajuda re-renderizado como pares estilizados (não string única):
teclas ACCENT BOLD, descrições FG, títulos de seção MUTED BOLD, alinhado em colunas
(como o mockup aprovado). Conteúdo/teclas iguais aos atuais + `]h/[h`, `]c/[c`, `/`.

## Testes

- Picker render: header chip presente, painel com título, `▶` + bg na seleção,
  contador, hints à direita.
- Status flat: tecla e rótulo presentes; truncamento com `…` em largura estreita;
  mensagem de erro mantém prioridade.
- Display rows: novas rows Header/Body/Footer (formas exatas), navegação para no
  Header, `]c` idem.
- Render do cartão: bordas completas `╭─`/`╰─`, teclas no footer, largura total.
- Ajuda: teclas em accent (asserção de estilo em uma célula) + textos.
- Snapshots insta regenerados e conferidos (layout de 3 linhas muda todos).
