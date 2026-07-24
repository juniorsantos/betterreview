# Roadmap inspirado no hunk — spec e planejamento

Data: 2026-07-24 · Origem: análise de modem-dev/hunk (README, código, PRs/issues abertos)
Status: itens 1/3/4 **aprovados para implementação imediata**; 2/5/6/7 documentados para depois.

## Contexto

O hunk valida nossa posição provider-first (as issues deles pedem o que já temos: abrir
PR por número, comentários reais, publicar review). O que vale absorver é ergonomia de
navegação e redução de ruído. Este documento planeja as 7 ideias; as três "S" entram já.

---

## Item 1 — Saltos por hunk e por comentário (IMPLEMENTAR AGORA)

**O quê:** teclas de prefixo no padrão já existente (`]f`/`[f`):
- `]h` / `[h` — próximo / anterior **hunk** (linha `@@` do diff).
- `]c` / `[c` — próximo / anterior **comentário** (display row `Comment { block_start: true }`).

**Regras:**
- Operam sobre as display rows do arquivo ativo, com clamp (sem wrap); sem alvo → notice
  ("não há próximo hunk" / "não há próximo comentário") via `push_notice`.
- Alvo de hunk = display row `Diff { row }` cujo `rendered_diff.rows[row]` corresponde a um
  `DiffRowKind::HunkHeader` do `parsed_diff` (usar `parsed_diff.rows[row].kind`; se
  `parsed_diff` for None, notice "diff ainda carregando").
- Ao aterrissar: mesma semântica do MoveCursor (Diff row sincroniza `session.cursor_row`).
- Novas actions: `NextHunk`, `PreviousHunk`, `NextComment`, `PreviousComment` no keymap de
  prefixo (`keymap.rs`, braços `(']', 'h')`, `('[', 'h')`, `(']', 'c')`, `('[', 'c')`).

**Testes:** reducer — `bracket_h_jumps_to_the_next_hunk_header`,
`bracket_c_jumps_between_comment_blocks`, `hunk_jump_clamps_at_the_last_hunk` (notice);
keymap — mapeamento dos 4 prefixos.

**Arquivos:** `src/tui/keymap.rs`, `src/app/event.rs`, `src/app/reducer.rs`,
`tests/app_reducer.rs`, `tests/tui_navigation.rs` (mapeos), help em `src/tui/render.rs`.

---

## Item 3 — Busca no diff (IMPLEMENTAR AGORA)

**O quê:** `/` abre input de busca na barra de status; `Enter` confirma; `n`/`N` navega
ocorrências; `Esc` cancela e limpa.

**Regras:**
- Estado novo (AppState, não persistido): `search_input: Option<String>` (modo digitação),
  `search_query: Option<String>`, derivação sob demanda dos matches.
- Match = display row cujo texto contém a query (case-insensitive): rows `Diff` usam o texto
  renderizado (`rendered_diff.rows[row].text` concatenado) e rows `Comment` usam `text`.
- `Enter`: fixa a query, pula para o primeiro match a partir do cursor (mesma aterrissagem
  do MoveCursor); `n` próximo, `N` anterior, com wrap e notice "sem resultados".
- Status bar em modo digitação mostra `/query▌` (precedência acima do resumo, abaixo de
  erro); com busca ativa mostra `“query” 3/17  n/N navega  Esc limpa`.
- Teclas no modo digitação: chars/backspace editam, Enter confirma, Esc cancela — tratado
  em `handle_key` ANTES do keymap (novo braço, padrão dos modais).
- `/` só ativa com foco no Diff e sem modal aberto.

**Testes:** reducer — `search_jumps_to_the_first_match`, `n_wraps_around_matches`,
`esc_clears_the_search`; render — status mostra query e contador; keys — `/` entra em modo
digitação e chars não vazam para o keymap.

**Arquivos:** `src/app/state.rs`, `src/app/event.rs` (`SearchInput(char)`? — não: edição do
input fica no handle_key mutando o estado direto, actions só `ConfirmSearch`,
`SearchNext`, `SearchPrevious`, `CancelSearch`), `src/tui/terminal.rs`,
`src/tui/widgets/status.rs`, `src/app/reducer.rs`, testes correspondentes.

---

## Item 4 — De-ênfase de arquivos gerados (IMPLEMENTAR AGORA)

**O quê:** lockfiles e arquivos gerados/minificados não devem disputar atenção.

**Regras:**
- `pub fn is_generated(path: &str) -> bool` em `src/app/display.rs` (ou módulo novo
  `src/app/generated.rs`): basename em {`package-lock.json`, `yarn.lock`,
  `pnpm-lock.yaml`, `Cargo.lock`, `composer.lock`, `Gemfile.lock`, `go.sum`,
  `poetry.lock`, `uv.lock`, `bun.lockb`}, OU sufixos `.min.js`, `.min.css`, `.map`,
  `.lock`, OU componentes de caminho `vendor/`, `node_modules/`, `dist/`, `generated/`,
  `__generated__/`.
- Painel Files: entrada renderizada em MUTED com marcador `⊘` no lugar do status colorido.
- Navegação `]u`/`[u` (não revisados) **pula** gerados (tratados como se revisados);
  `j`/`k`/`]f` continuam alcançando (nada fica inacessível).
- Contadores `+N -N` continuam visíveis.

**Testes:** unit — `is_generated` (tabela de casos); reducer —
`unreviewed_navigation_skips_generated_files`; render — arquivo gerado com `⊘` e sem
letra de status colorida.

**Arquivos:** `src/app/display.rs` (ou `generated.rs`), `src/app/reducer.rs`
(`navigate_unreviewed`), `src/tui/widgets/files.rs`, testes.

---

## Item 2 — Expandir contexto não alterado (`z` no diff) — DEPOIS (M)

Expandir os trechos recolhidos entre hunks carregando o arquivo real (blob no head/base via
`gh api`/`glab api`), com placeholder "· · · N linhas ocultas · · ·", spinner durante o
fetch (infra de labels já existe) e cache por arquivo. Exige: fetch de blob por provider
(novo método no contrato), inserção de rows sintéticas no display layer (novo
`DisplayRow::Context`), e invalidação no refresh. Planejar spec própria quando priorizado —
tocará `providers/*`, `display.rs`, `diff widget` e o parser.

## Item 5 — Hunk revisado com persistência — DEPOIS (M)

`SessionSnapshot.files[path]` ganharia `reviewed_hunks: BTreeSet<String>` (hash estável do
hunk header + conteúdo) — exige bump de `SESSION_SCHEMA_VERSION` e migração tolerante.
Tecla `m` com cursor dentro de um hunk marca o hunk; arquivo vira revisado quando todos os
hunks estão. Render esmaece hunks vistos. Dependência: identidade de hunk robusta a
rebase (hash do conteúdo, não das linhas).

## Item 6 — Layout side-by-side com auto-fallback — DEPOIS (M/L)

`mode = auto|split|stack` (tecla `2`/`1`/`0`), split só quando largura ≥ ~160 colunas.
Exige pareamento old/new por hunk (parser já tem old_line/new_line por row — pareamento é
zip por hunk com placeholders), segunda coluna de render e dois viewports sincronizados.
Maior risco: interação com display rows de comentários (âncora em um dos lados). Spec
própria obrigatória.

## Item 7 — Ponte para agentes (propor → aprovar → publicar) — DEPOIS (L, aposta grande)

Superfície local (Unix socket em `$XDG_RUNTIME_DIR/betterreview.sock`) com comandos:
`propose-comment` (path, side, line, body), `list`, `navigate`. Comentários propostos
entram como estado novo `CommentEntry::Suggested` (cor própria), o humano promove (`a`
aceita → vira draft real via CreateDraft) ou descarta (`x`). O submit publica só os
promovidos. É a metade que falta no hunk (issues #460/#115 deles) e nosso diferencial:
nós já temos o publish. Riscos: segurança (só loopback/socket com permissão 0600),
concorrência com o event loop (canal mpsc novo no `tui::run`). Spec própria obrigatória.

---

## Execução (itens 1/3/4)

Dois agentes em worktrees isoladas, TDD, commits de uma linha:
- **Agente A**: itens 1 + 3 (mesmos arquivos: keymap/reducer/terminal/status).
- **Agente B**: item 4 (files widget + navigate_unreviewed).
Merge pelo controller (conflitos esperados apenas em `AppAction`/help — resolução manual),
suite completa + clippy + release build ao final. Revisão final única dos três itens.
