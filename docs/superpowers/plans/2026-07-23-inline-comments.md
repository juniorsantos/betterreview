# Inline Diff Comments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Comentários e drafts aparecem inline no diff (estilo GitHub), o cursor navega por eles, `e` edita / `x` exclui / `r` responde, e toda gravação mostra spinner + rótulo na barra de status.

**Architecture:** Uma camada pura nova `src/app/display.rs` converte `rendered_diff` + threads/drafts em "display rows" (1 linha de terminal cada; blocos de comentário com `block_start`). O reducer ganha `display_cursor` (in-memory; a sessão continua persistindo só a linha de diff), modos de editor (edit/reply), diálogo de confirmação de exclusão, e `pending_labels` para o spinner. O widget de diff renderiza as display rows.

**Tech Stack:** Rust, ratatui, padrão reducer existente (`src/app/reducer.rs`), tema `src/tui/theme.rs`, testes com TestBackend/insta.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-23-inline-comments-design.md` (ler antes de cada task).
- Commits SEMPRE de uma linha, sem trailers.
- TDD: teste vermelho antes de cada implementação.
- Cores só de `crate::tui::theme`; highlight de linha inteira via padding (padrão de `src/tui/widgets/diff.rs`).
- Schema da sessão (`SessionSnapshot`) INTOCADO — `display_cursor`, `comments_hidden`, `editing_draft`, `replying_thread`, `pending_labels`, `delete_dialog` são só `AppState`.
- `cargo clippy --all-targets` sem warnings; suite completa verde antes de cada commit.
- v1 não edita/exclui comentário publicado (só drafts) e não resolve thread pela caixa inline.

---

### Task 1: Camada pura de display rows

**Files:**
- Create: `src/app/display.rs` (registrar `pub mod display;` + re-export em `src/app/mod.rs`)
- Test: `tests/display_rows.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentEntry {
    Draft { id: crate::domain::DraftId },
    Thread { thread: crate::domain::ThreadId, comment_index: usize },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    Diff { row: usize },
    Comment { entry: CommentEntry, block_start: bool, text: String, author: Option<String> },
    OrphanHeader,
}
pub fn build_display_rows(
    rendered: &crate::diff::RenderedDiff,
    threads: &[crate::domain::ReviewThread],
    drafts: &[crate::domain::DraftComment],
    active_path: &crate::domain::RepoPath,
    hidden: bool,
) -> Vec<DisplayRow>;
```

Regras (da spec):
- `hidden == true` → só `Diff { row }` na ordem original.
- Âncora: primeiro row cujo binding (`left`/`right`) tem `position.side == c.side && position.line == c.line` (comparar com a `DiffPosition` do comentário; drafts usam `selection.end`). Blocos inseridos logo após a âncora, na ordem: threads (na ordem do vetor, cada comentário da thread = um bloco), depois drafts.
- Bloco = 1 row `block_start: true` com `text` = primeira linha do corpo e `author` preenchido, + 1 row (`block_start: false`, `author: None`) por linha restante do corpo (`body.lines()`).
- Sem âncora (posição não encontrada, ou `selection == None` no draft): blocos vão para o fim após um único `OrphanHeader` (só existe se houver órfãos).
- Threads/drafts de OUTROS arquivos (path ≠ active_path) são ignorados.

- [ ] **Step 1: Testes vermelhos** em `tests/display_rows.rs` (helpers construindo RenderedDiff com RowBinding — copiar padrão de `tests/tui_diff_render.rs`): `hidden_returns_only_diff_rows`, `draft_block_appears_under_its_anchor`, `multiline_body_marks_only_the_first_row_as_block_start`, `thread_with_two_comments_produces_two_blocks`, `unanchored_comments_group_after_an_orphan_header`, `other_files_comments_are_ignored`.
- [ ] **Step 2:** `cargo test --test display_rows` → FAIL (módulo inexistente).
- [ ] **Step 3:** Implementar `src/app/display.rs` conforme regras.
- [ ] **Step 4:** `cargo test --test display_rows` → PASS; suite completa PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat: build display rows for inline comments"`

---

### Task 2: Cursor sobre display rows + toggle T

**Files:**
- Modify: `src/app/state.rs` (campos novos), `src/app/reducer.rs`, `src/app/event.rs` (`AppAction::ToggleComments`), `src/tui/keymap.rs` (`T`)
- Test: `tests/app_reducer.rs` (append)

**Interfaces:**
- Produces: `AppState.display_cursor: usize`, `AppState.comments_hidden: bool` (default false), helper `pub fn display_rows(state: &AppState) -> Vec<DisplayRow>` (em `src/app/display.rs`, chama build com os campos do state).
- Consumes: Task 1.

Regras:
- `MoveCursor(delta)` com foco Diff: move `display_cursor` pelo vetor de display rows, parando apenas em rows com parada válida (`Diff` ou `Comment { block_start: true }`; `OrphanHeader` e continuações são puladas). Ao parar num `Diff { row }`, sincronizar `state.session.cursor_row = row` e `dirty = true`. Num Comment, `session.cursor_row` fica no último valor.
- `ToggleSelection` (`v`): se a display row atual não é `Diff`, no-op + notice "mova para uma linha de código".
- Navegação de arquivo (`]f` etc.) reseta `display_cursor = 0` (junto do reset atual de cursor_row).
- `ToggleComments` (`T`, shift+t — `KeyCode::Char('T')`): alterna `comments_hidden`; ao esconder, `display_cursor` re-sincroniza para a posição do `session.cursor_row` atual.
- Snapshot refresh / render novo (EffectOutcome::Rendered): clampar `display_cursor` ao novo tamanho.

- [ ] **Step 1: Testes vermelhos** (append `tests/app_reducer.rs`): `cursor_walks_through_comment_blocks`, `cursor_on_comment_keeps_session_row`, `selection_refused_on_comment_rows`, `toggle_comments_resyncs_cursor`.
- [ ] **Step 2:** RED → **Step 3:** implementar → **Step 4:** GREEN + suite completa.
- [ ] **Step 5: Commit** — `git commit -am "feat: navigate the diff through inline comments"`

---

### Task 3: Render dos blocos no widget de diff

**Files:**
- Modify: `src/tui/widgets/diff.rs`
- Test: `tests/tui_diff_render.rs` (append)

**Interfaces:**
- Consumes: `display_rows(state)`, `DisplayRow`, `state.display_cursor`.

Regras:
- O widget itera display rows (não mais `rendered_diff.rows` direto). `Diff { row }` renderiza como hoje (gutter + linha). `Comment` block_start: `│ @autor  draft` (autor em ACCENT; sufixo `draft` em WARNING quando `CommentEntry::Draft`; `✓` SUCCESS quando thread resolvida); continuação: `│ <linha do corpo>` em FG; prefixo `│` em BORDER, indentado após o gutter (6 espaços). `OrphanHeader`: `— comentários desatualizados —` em MUTED.
- Cursor: display row atual == `display_cursor` → bg CURSOR_LINE linha inteira (mesmo padding atual). Seleção continua marcando rows Diff pelo índice de diff (comportamento atual preservado quando não há comentários).
- Scroll do viewport usa `display_cursor` e o total de display rows.

- [ ] **Step 1: Testes vermelhos** (append em `tests/tui_diff_render.rs`, reusar helper `app()` adicionando um draft com selection na linha 5): `comment_box_renders_under_its_line` (contém `@` do autor e o corpo logo após a linha ancorada), `cursor_highlights_a_comment_row_full_width`, `toggle_hides_comment_rows`.
- [ ] **Step 2:** RED → **Step 3:** implementar → **Step 4:** GREEN; atualizar snapshots insta afetados via `INSTA_UPDATE=always` nos testes de layout e conferir o diff dos .snap.
- [ ] **Step 5: Commit** — `git commit -am "feat: render inline comments in the diff"`

---

### Task 4: Ações e/x/r sobre comentários

**Files:**
- Modify: `src/app/state.rs` (`editing_draft: Option<DraftId>`, `replying_thread: Option<ThreadId>`, `delete_dialog: Option<DraftId>`), `src/app/reducer.rs`, `src/app/event.rs` (`AppAction::{EditComment, DeleteComment, ConfirmDelete(bool), ReplyComment}`), `src/tui/keymap.rs`, `src/tui/terminal.rs` (handle_key), `src/tui/widgets/editor.rs` (título por modo), `src/tui/render.rs` + novo bloco de confirmação (reusar padrão do quit dialog em `src/tui/widgets/quit.rs` — extrair helper ou duplicar mínimo)
- Modify: `src/app/effect.rs`/`src/app/runtime.rs` (`EffectOutcome::DraftDeleted { id: DraftId, result: Result<(), String> }` no lugar de `Completed` para DeleteDraft)
- Test: `tests/tui_keys.rs` + `tests/app_reducer.rs` (append)

**Interfaces:**
- Consumes: display rows (entry sob o cursor).
- Produces: fluxos completos de editar/excluir/responder.

Regras:
- Tecla `e` (foco Diff, display row = `Comment { entry: Draft { id } }`): abre o editor pré-preenchido com o corpo do draft (`EditorSnapshot` com `lines` do body; selection copiada do draft), `editing_draft = Some(id)`. `Enter` no editor: se `editing_draft` é Some → `AppAction::UpdateDraft { id, body }` (em vez de CreateDraft); ao receber `DraftUpdated` Ok, limpar `editing_draft`. Esc cancela e limpa o modo. Em row que não é Draft: notice "só drafts podem ser editados".
- Tecla `x` (row Draft): `delete_dialog = Some(id)`; diálogo `Excluir comentário?` com opções `Excluir`/`Cancelar` (menu ▸ como o quit: j/k, Enter confirma, Esc cancela). Confirmar → `AppAction::DeleteComment(id)` → efeito DeleteDraft. Novo `EffectOutcome::DraftDeleted`: Ok → `state.provider.drafts.retain(|d| d.id != id)` (hoje o Completed genérico NÃO remove o draft — este é um bug latente que esta task conserta); Err → error banner.
- Tecla `r` (row Thread): abre editor vazio em modo reply (`replying_thread = Some(thread)`); `Enter` → `AppAction::Reply { thread, body }`; `ThreadUpdated` Ok já substitui a thread (existente) — limpar `replying_thread` e fechar editor. FORA de comment rows, `r` continua Refresh (decidir em handle_key pela display row atual).
- Título do editor por modo: `" Comment editor — …"` / `" Editing draft — …"` / `" Replying — …"`.
- `c`/`s` numa comment row: notice "mova para uma linha de código".

- [ ] **Step 1: Testes vermelhos**: em `tests/app_reducer.rs`: `edit_opens_editor_with_draft_body_and_enter_updates`, `delete_dialog_confirms_and_removes_the_draft` (incluindo DraftDeleted Ok removendo de provider.drafts), `reply_on_thread_dispatches_reply`; em `tests/tui_keys.rs`: `r_replies_when_cursor_is_on_a_thread_comment`, `r_refreshes_elsewhere`.
- [ ] **Step 2:** RED → **Step 3:** implementar → **Step 4:** GREEN + suite completa.
- [ ] **Step 5: Commit** — `git commit -am "feat: edit delete and reply to comments inline"`

---

### Task 5: Spinner e rótulos de gravação

**Files:**
- Modify: `src/app/state.rs` (`pending_labels: BTreeMap<u64, &'static str>`, `spinner_frame: usize`), `src/app/reducer.rs`, `src/tui/widgets/status.rs`
- Test: `tests/app_reducer.rs` + novo teste de render em `tests/tui_diff_render.rs` ou arquivo próprio `tests/status_feedback.rs`

Regras:
- Ao agendar efeitos (na função que cria o `EffectEnvelope`/insere em `busy_operations`), registrar rótulo por id: CreateDraft → `"salvando comentário…"`, UpdateDraft → `"atualizando comentário…"`, DeleteDraft → `"excluindo comentário…"`, Reply → `"respondendo…"`, SubmitReview → `"enviando revisão…"`, RefreshSnapshot → `"atualizando…"`. Demais efeitos: sem rótulo.
- `EffectFinished` remove o id de `pending_labels` (junto do remove de busy_operations).
- `Tick` avança `spinner_frame = spinner_frame.wrapping_add(1)` quando `pending_labels` não está vazio.
- `status.rs`: quando houver rótulo pendente (o mais recente, maior id), prefixar a mensagem com o frame do spinner `["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧"][spinner_frame % 8]` + rótulo, em ACCENT; error banner continua tendo precedência (DANGER).
- [ ] **Step 1: Testes vermelhos**: `scheduling_a_draft_registers_a_pending_label`, `finished_effect_clears_its_label`, render: `status_shows_spinner_while_saving` (montar app com pending_labels + busy e conferir o texto).
- [ ] **Step 2:** RED → **Step 3:** implementar → **Step 4:** GREEN + suite.
- [ ] **Step 5: Commit** — `git commit -am "feat: show saving feedback in the status bar"`

---

### Task 6: Fechamento

- [ ] **Step 1:** `cargo fmt`; `cargo clippy --all-targets` 0 warnings; `cargo test` inteiro verde.
- [ ] **Step 2:** `cargo build --release`.
- [ ] **Step 3:** Atualizar `tasks/todo.md` (pacote comentários entregue) e o texto do help (`?`) com `e/x/r` e `T` (atualizar teste/snapshot do help se necessário).
- [ ] **Step 4: Commit** — `git commit -am "docs: record inline comments delivery"`

## Self-Review

- Spec coverage: display rows (T1), navegação/cursor/T (T2), render caixas/órfãos (T3), e/x/r + confirmação + DraftDeleted (T4), spinner/labels incluindo submit (T5). Sessão intocada em todas.
- Tipos consistentes entre tasks (DisplayRow/CommentEntry consumidos por T2-T4; pending_labels por T5).
- Sem placeholders; commits de uma linha.
