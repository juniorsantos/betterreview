# Review Picker with Prefetch — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `betterreview` sem argumentos dentro de um repo abre uma lista dos PRs/MRs abertos (branch atual destacada no topo) com prefetch do item destacado; Enter abre o review usando o snapshot já baixado.

**Architecture:** Um método novo `list_open` no contrato `ReviewProvider` (1 chamada por provider). Uma tela nova `src/tui/picker.rs` no padrão do app: reducer puro (`PickerState` + `update`) testável sem async, mais um loop fino que traduz `PickerCommand` em tasks tokio (prefetch com abort). O entrypoint ganha `run_loaded` (o miolo do `launch_key` pós-load) reutilizado pelos dois caminhos.

**Tech Stack:** Rust, tokio, ratatui/crossterm, gh/glab via `CommandRunner`, insta para snapshots.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-23-review-picker-design.md`.
- Commits **sempre de uma linha**, sem trailers (preferência do usuário).
- TDD: todo passo de código nasce de um teste vermelho.
- Sem paginação no picker: 50 mais recentes (footer avisa quando ==50).
- Tema: usar `crate::tui::theme` (nunca `Color::` hardcoded).
- Alvo explícito (URL/número) e `resume`/`sessions` mantêm o fluxo atual intocado.

---

### Task 1: `ChangeRequestSummary` + `list_open` no GitHub

**Files:**
- Modify: `src/domain/review.rs` (novo struct), `src/domain/mod.rs` (re-export)
- Modify: `src/providers/mod.rs` (método no trait)
- Modify: `src/providers/github/mod.rs`, `src/providers/github/graphql.rs`, `src/providers/github/wire.rs`
- Modify: `src/providers/gitlab/mod.rs` (stub que compila; implementação real na Task 2)
- Test: `tests/provider_github_read.rs`
- Create: `tests/fixtures/github/list-open.json`

**Interfaces:**
- Produces: `domain::ChangeRequestSummary { number: u64, title: String, author: String, source_branch: String, updated_at: time::OffsetDateTime, draft: bool, web_url: String }`
- Produces: `ReviewProvider::list_open(&self, host: &str, repository: &str) -> Result<Vec<ChangeRequestSummary>, ProviderError>`

- [ ] **Step 1: Fixture**

`tests/fixtures/github/list-open.json`:

```json
{
  "data": {
    "repository": {
      "pullRequests": {
        "nodes": [
          {
            "number": 7,
            "title": "feat: add picker",
            "isDraft": false,
            "updatedAt": "2026-07-23T10:00:00Z",
            "headRefName": "feature/picker",
            "url": "https://ghe.acme.test/acme/api/pull/7",
            "author": { "login": "jsjunior" }
          },
          {
            "number": 5,
            "title": "chore: bump deps",
            "isDraft": true,
            "updatedAt": "2026-07-22T09:00:00Z",
            "headRefName": "chore/deps",
            "url": "https://ghe.acme.test/acme/api/pull/5",
            "author": null
          }
        ]
      }
    }
  }
}
```

- [ ] **Step 2: Teste vermelho** (append em `tests/provider_github_read.rs`; o `RoutingRunner` existente responde por inspeção — adicionar um braço para a query de listagem)

No `RoutingRunner::respond`, antes do braço de cursor, detectar a listagem:

```rust
if args.iter().any(|arg| arg == "graphql") {
    let body: Value =
        serde_json::from_slice(spec.stdin.as_ref().expect("graphql stdin")).unwrap();
    if body["query"].as_str().unwrap_or("").contains("pullRequests(") {
        return ok(fixture("list-open.json"));
    }
    // ... braço do cursor existente
}
```

Teste:

```rust
#[tokio::test]
async fn lists_open_pull_requests_in_one_call() {
    let runner = Arc::new(RoutingRunner::new());
    let provider = GitHubProvider::new(runner.clone());

    let list = provider.list_open("ghe.acme.test", "acme/api").await.unwrap();

    assert_eq!(list.len(), 2);
    assert_eq!(list[0].number, 7);
    assert_eq!(list[0].source_branch, "feature/picker");
    assert_eq!(list[0].author, "jsjunior");
    assert!(!list[0].draft);
    assert_eq!(list[1].author, "unknown");
    assert!(list[1].draft);
    let calls = runner.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|spec| args(spec).get(1) == Some(&"graphql".to_owned()))
            .count(),
        1
    );
}
```

- [ ] **Step 3: Rodar e ver falhar** — `cargo test --test provider_github_read lists_open` → FAIL (`no method named list_open`).

- [ ] **Step 4: Implementar**

`src/domain/review.rs` (junto de `ProviderSnapshot`):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRequestSummary {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub source_branch: String,
    pub updated_at: time::OffsetDateTime,
    pub draft: bool,
    pub web_url: String,
}
```

Re-exportar em `src/domain/mod.rs` no `pub use review::{...}`.

`src/providers/mod.rs`, dentro do trait (depois de `discover`):

```rust
async fn list_open(
    &self,
    host: &str,
    repository: &str,
) -> Result<Vec<crate::domain::ChangeRequestSummary>, ProviderError>;
```

`src/providers/github/graphql.rs`:

```rust
pub const LIST_OPEN_QUERY: &str = r#"
query ListOpen($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    pullRequests(states: OPEN, first: 50, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes { number title isDraft updatedAt headRefName url author { login } }
    }
  }
}
"#;
```

`src/providers/github/wire.rs`:

```rust
#[derive(Deserialize)]
pub struct ListData {
    pub repository: Option<ListRepository>,
}

#[derive(Deserialize)]
pub struct ListRepository {
    #[serde(rename = "pullRequests")]
    pub pull_requests: ListConnection,
}

#[derive(Deserialize)]
pub struct ListConnection {
    pub nodes: Vec<ListNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNode {
    pub number: u64,
    pub title: String,
    pub is_draft: bool,
    pub updated_at: String,
    pub head_ref_name: String,
    pub url: String,
    pub author: Option<Author>,
}
```

(usar o struct `Author { login }` já existente no wire.rs)

`src/providers/github/mod.rs`, no `impl ReviewProvider`:

```rust
async fn list_open(
    &self,
    host: &str,
    repository: &str,
) -> Result<Vec<ChangeRequestSummary>, ProviderError> {
    let (owner, name) = repository_parts(repository)?;
    let bytes = self
        .client
        .graphql(
            host,
            LIST_OPEN_QUERY,
            json!({ "owner": owner, "name": name }),
            "list open pull requests",
        )
        .await?;
    let envelope: GraphQlEnvelope<ListData> = parse_json(&bytes, "list open pull requests")?;
    ensure_graphql(&envelope, "list open pull requests")?;
    let nodes = envelope
        .data
        .and_then(|data| data.repository)
        .map(|repository| repository.pull_requests.nodes)
        .unwrap_or_default();
    nodes
        .into_iter()
        .map(|node| {
            Ok(ChangeRequestSummary {
                number: node.number,
                title: node.title,
                author: node
                    .author
                    .map_or_else(|| "unknown".into(), |author| author.login),
                source_branch: node.head_ref_name,
                updated_at: time::OffsetDateTime::parse(
                    &node.updated_at,
                    &time::format_description::well_known::Rfc3339,
                )
                .map_err(|error| malformed("list open pull requests", &error.to_string()))?,
                draft: node.is_draft,
                web_url: node.url,
            })
        })
        .collect()
}
```

Imports: acrescentar `ChangeRequestSummary` ao `use crate::domain::{...}` e `LIST_OPEN_QUERY`/`ListData` aos `use self::...`.

`src/providers/gitlab/mod.rs` (stub temporário para compilar; Task 2 troca):

```rust
async fn list_open(
    &self,
    _host: &str,
    _repository: &str,
) -> Result<Vec<ChangeRequestSummary>, ProviderError> {
    Err(unsupported("list open", "implemented in the next task"))
}
```

(importar `ChangeRequestSummary`)

- [ ] **Step 5: Verificar** — `cargo test --test provider_github_read` → PASS; `cargo test` inteiro → PASS.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: list open pull requests for the picker"`

---

### Task 2: `list_open` no GitLab

**Files:**
- Modify: `src/providers/gitlab/mod.rs`, `src/providers/gitlab/wire.rs`
- Test: `tests/provider_gitlab_read.rs`
- Create: `tests/fixtures/gitlab/merge-requests-list.json`

**Interfaces:**
- Consumes: `ChangeRequestSummary`, trait `list_open` (Task 1).
- Produces: implementação real GitLab (endpoint `projects/{proj}/merge_requests?state=opened&order_by=updated_at&sort=desc&per_page=50`).

- [ ] **Step 1: Fixture** `tests/fixtures/gitlab/merge-requests-list.json`:

```json
[
  {
    "iid": 12,
    "title": "feat: picker",
    "draft": false,
    "updated_at": "2026-07-23T10:00:00.000Z",
    "source_branch": "feature/picker",
    "web_url": "https://git.acme.test/group/api/-/merge_requests/12",
    "author": { "username": "jsjunior" }
  },
  {
    "iid": 9,
    "title": "Draft: wip",
    "draft": true,
    "updated_at": "2026-07-21T08:00:00.000Z",
    "source_branch": "wip",
    "web_url": "https://git.acme.test/group/api/-/merge_requests/9",
    "author": { "username": "dev2" }
  }
]
```

- [ ] **Step 2: Teste vermelho** (append em `tests/provider_gitlab_read.rs`; o `RoutingRunner` keyed por endpoint já serve):

```rust
#[tokio::test]
async fn lists_open_merge_requests_in_one_call() {
    let runner = Arc::new(RoutingRunner::new(vec![(
        "projects/group%2Fapi/merge_requests?state=opened&order_by=updated_at&sort=desc&per_page=50",
        fixture("merge-requests-list.json"),
    )]));
    let provider = GitLabProvider::new(runner.clone());

    let list = provider.list_open("git.acme.test", "group/api").await.unwrap();

    assert_eq!(list.len(), 2);
    assert_eq!(list[0].number, 12);
    assert_eq!(list[0].source_branch, "feature/picker");
    assert!(list[1].draft);
    assert_eq!(runner.calls.lock().unwrap().len(), 1);
}
```

- [ ] **Step 3: Rodar e ver falhar** — `cargo test --test provider_gitlab_read lists_open` → FAIL (stub retorna Unsupported).

- [ ] **Step 4: Implementar** — `src/providers/gitlab/wire.rs`:

```rust
#[derive(Deserialize)]
pub struct MergeRequestSummary {
    pub iid: u64,
    pub title: String,
    pub draft: bool,
    pub updated_at: String,
    pub source_branch: String,
    pub web_url: String,
    pub author: Author,
}
```

(reusar `Author { username }` existente). Substituir o stub em `src/providers/gitlab/mod.rs`:

```rust
async fn list_open(
    &self,
    host: &str,
    repository: &str,
) -> Result<Vec<ChangeRequestSummary>, ProviderError> {
    let project = encode(repository);
    let endpoint = format!(
        "projects/{project}/merge_requests?state=opened&order_by=updated_at&sort=desc&per_page=50"
    );
    let summaries: Vec<MergeRequestSummary> = parse_json(
        &self
            .read_api(host, api_args(host, [endpoint.as_str()]), "list open merge requests")
            .await?,
        "list open merge requests",
    )?;
    summaries
        .into_iter()
        .map(|summary| {
            Ok(ChangeRequestSummary {
                number: summary.iid,
                title: summary.title,
                author: summary.author.username,
                source_branch: summary.source_branch,
                updated_at: time::OffsetDateTime::parse(
                    &summary.updated_at,
                    &time::format_description::well_known::Rfc3339,
                )
                .map_err(|error| malformed("list open merge requests", &error.to_string()))?,
                draft: summary.draft,
                web_url: summary.web_url,
            })
        })
        .collect()
}
```

- [ ] **Step 5: Verificar** — `cargo test --test provider_gitlab_read` → PASS; suite inteira → PASS.

- [ ] **Step 6: Commit** — `git commit -am "feat: list open merge requests for the picker"`

---

### Task 3: Reducer puro do picker

**Files:**
- Create: `src/tui/picker.rs` (registrar `pub mod picker;` em `src/tui/mod.rs`)
- Test: `tests/picker_reducer.rs`

**Interfaces:**
- Consumes: `domain::{ChangeRequestSummary, ProviderSnapshot}`.
- Produces (usados nas Tasks 4-6):

```rust
pub struct PickerItem {
    pub summary: ChangeRequestSummary,
    pub has_session: bool,
    pub current_branch: bool,
}
pub struct PickerState {
    pub items: Vec<PickerItem>,
    pub highlight: usize,
    pub cache: BTreeMap<u64, ProviderSnapshot>,
    pub errors: BTreeMap<u64, String>,
    pub loading: Option<u64>,
    pub waiting: Option<u64>,
    pub error_banner: Option<String>,
    pub quit: bool,
    pub chosen: Option<(u64, Option<ProviderSnapshot>)>,
}
pub enum PickerEvent {
    Key(crossterm::event::KeyEvent),
    Tick,
    Loaded { number: u64, result: Result<ProviderSnapshot, String> },
    ListReloaded { items: Vec<PickerItem> },
}
pub enum PickerCommand { StartPrefetch(u64), ReloadList }
pub fn pin_current_branch(items: &mut Vec<PickerItem>);
pub fn update(state: &mut PickerState, event: PickerEvent) -> Vec<PickerCommand>;
impl PickerState { pub fn new(items: Vec<PickerItem>) -> Self; }
```

Regras do reducer:
- `PickerState::new`: chama `pin_current_branch` (item com `current_branch == true` vai para o índice 0; demais mantêm ordem) e destaca 0.
- `j`/Down: `highlight = (highlight + 1).min(items.len().saturating_sub(1))`; `k`/Up: `saturating_sub(1)`.
- `Enter`: `number` do destacado; cache hit → `chosen = Some((number, Some(snapshot.clone())))`; senão `errors.remove(&number)`, `waiting = Some(number)`, `error_banner = None`; se `loading != Some(number)` → `loading = Some(number)` e retorna `[StartPrefetch(number)]`.
- `q`/`Esc`: `quit = true`.
- `r`: retorna `[ReloadList]`.
- `Tick`: `target` = número do destacado; se `!cache.contains && !errors.contains && loading != Some(target)` → `loading = Some(target)` + `[StartPrefetch(target)]` (o tick de ~300ms é o debounce).
- `Loaded Ok`: insere no cache; `loading = None` se era esse número; se `waiting == Some(number)` → `chosen = Some((number, Some(snapshot)))`.
- `Loaded Err`: `errors.insert`; `loading = None` se era esse; se `waiting == Some(number)` → `waiting = None`, `error_banner = Some(mensagem)`.
- `ListReloaded`: substitui items, re-pina, clampa highlight.
- Lista vazia: qualquer Key além de q/Esc/r vira no-op.

- [ ] **Step 1: Testes vermelhos** — `tests/picker_reducer.rs` com um helper `summary(number, branch)` e casos:
  1. `new_pins_the_current_branch_item_first`
  2. `tick_prefetches_the_highlighted_item_once` (segundo Tick sem mudança → sem comando)
  3. `moving_highlight_prefetches_the_new_item_on_next_tick`
  4. `enter_with_cache_hit_chooses_immediately`
  5. `enter_without_cache_waits_for_the_inflight_load` (Enter → StartPrefetch; Loaded Ok → chosen)
  6. `load_error_surfaces_only_when_entering_the_item` (Tick → Loaded Err → sem banner; Enter → banner após Err OU retry: Enter limpa erro e re-prefetcha)
  7. `q_quits_and_r_reloads`

Código dos testes: construir `ChangeRequestSummary` direto (updated_at = `time::OffsetDateTime::UNIX_EPOCH`), `ProviderSnapshot` mínimo pode ser criado com `provider_snapshot()` copiado do helper de `tests/session_restore.rs` (files vazios).

- [ ] **Step 2: Rodar e ver falhar** — `cargo test --test picker_reducer` → FAIL (módulo inexistente).
- [ ] **Step 3: Implementar** `src/tui/picker.rs` conforme regras acima (sem render ainda).
- [ ] **Step 4: Verificar** — `cargo test --test picker_reducer` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat: add picker state machine"`

---

### Task 4: Render do picker

**Files:**
- Modify: `src/tui/picker.rs` (fn `render` + fn `age`)
- Test: `tests/picker_render.rs`

**Interfaces:**
- Produces: `pub fn render(frame: &mut Frame, state: &PickerState)`; `fn age(now: OffsetDateTime, updated: OffsetDateTime) -> String` (`"agora"`, `"5m"`, `"3h"`, `"2d"`).

Layout (tela cheia, tema `theme::*`):
- Linha 0: ` Reviews abertos — owner/repo` (título via primeiro item; se vazio, ` Reviews abertos`).
- Lista: uma linha por item: `▸ #42 titulo  @autor  branch  3h  [draft] [sessão]`; destacado com bg `CURSOR_LINE` + bold, linha inteira (padding como no diff); item da branch atual com `●` em `ACCENT` antes do número; `[draft]` em `MUTED`, `[sessão]` em `WARNING`; título truncado com `…` para caber.
- Status do prefetch na penúltima linha: `baixando #42…` (MUTED) / `#42 pronto` (SUCCESS) / erro do banner (DANGER).
- Última linha: ` j/k mover  Enter abrir  r recarregar  q sair`; se `items.len() == 50`, acrescentar `  (50 mais recentes)`.

- [ ] **Step 1: Testes vermelhos** (`tests/picker_render.rs`, TestBackend 100x30 como nos outros):
  1. `renders_items_with_pin_and_metadata` — contém `#42`, `@jsjunior`, branch, `[draft]` no item draft.
  2. `highlight_covers_the_full_line` — célula na coluna 97 da linha destacada tem bg `theme::CURSOR_LINE`.
  3. `age_formats_minutes_hours_days` — teste unitário da fn `age` (expor `pub` para teste ou testar via render com updated_at controlado; expor `pub fn age` é aceitável).
- [ ] **Step 2: Ver falhar** → **Step 3: Implementar** → **Step 4: PASS** (`cargo test --test picker_render`).
- [ ] **Step 5: Commit** — `git commit -am "feat: render the review picker"`

---

### Task 5: Loop async do picker (prefetch com abort + reload)

**Files:**
- Modify: `src/tui/picker.rs`

**Interfaces:**
- Produces:

```rust
pub enum PickerOutcome {
    Quit,
    Open { number: u64, snapshot: Option<ProviderSnapshot> },
}
pub struct PickerSource {
    pub provider: std::sync::Arc<dyn crate::providers::ReviewProvider>,
    pub kind: ProviderKind,
    pub host: String,
    pub repository: String,
    pub branch: Option<String>,
    pub sessions: std::collections::BTreeSet<u64>,
}
pub async fn run(
    terminal: &mut ratatui::DefaultTerminal,
    mut state: PickerState,
    source: PickerSource,
) -> Result<PickerOutcome, crate::tui::TuiError>
```

Comportamento (mesmo padrão de `tui::terminal::run`):
- `EventStream` + `tick` de 300ms + `mpsc::unbounded_channel::<PickerEvent>()`.
- Ctrl+C → `Ok(PickerOutcome::Quit)`.
- Cada `PickerCommand`:
  - `StartPrefetch(number)`: aborta o `JoinHandle` anterior (guardar `Option<(u64, JoinHandle<()>)>`; se o número for o mesmo, não reinicia); spawna `provider.load(&key_for(number))` e envia `PickerEvent::Loaded { number, result: result.map_err(|e| e.to_string()) }` no canal.
  - `ReloadList`: spawna `provider.list_open(&host, &repository)` e envia `ListReloaded` com os itens re-marcados (helper `pub fn mark_items(list: Vec<ChangeRequestSummary>, branch: Option<&str>, sessions: &BTreeSet<u64>) -> Vec<PickerItem>`); em erro, envia `Loaded`? Não — usar `error_banner` via um `PickerEvent::Loaded` não serve; adicionar variante `PickerEvent::ListFailed(String)` que só seta `error_banner`.
- `key_for(number)` monta `ChangeRequestKey { provider: kind, host, repository, number }`.
- Loop termina quando `state.quit` (→ `Quit`) ou `state.chosen` (→ `Open`).

- [ ] **Step 1: Teste vermelho para `mark_items`** (append em `tests/picker_reducer.rs`): branch match seta `current_branch`, sessions set seta `has_session`.
- [ ] **Step 2: Ver falhar** → implementar `mark_items` + variante `ListFailed` (com regra no reducer: seta `error_banner`) + o loop `run` (o loop em si é I/O fino; cobertura via reducer).
- [ ] **Step 3: Suite inteira** — `cargo test` → PASS; `cargo clippy --all-targets` → 0 warnings.
- [ ] **Step 4: Commit** — `git commit -am "feat: drive the picker with prefetch tasks"`

---

### Task 6: Integração no entrypoint

**Files:**
- Modify: `src/entrypoint.rs`

**Interfaces:**
- Consumes: `picker::{run, PickerOutcome, PickerSource, PickerState, mark_items}`, `JsonSessionStore::list`, Task 1/2 `list_open`.
- Produces: `InstalledRuntime::run_loaded(&self, key, fresh: ProviderSnapshot, terminal: &mut ratatui::DefaultTerminal) -> Result<(), LaunchError>`.

Mudanças:

1. Extrair de `launch_key` tudo que vem DEPOIS de `let fresh = loaded?;` para `run_loaded(key, fresh, terminal)` — a criação do terminal (`ratatui::init()` + `TerminalRestore`) fica no chamador:
   - `launch_key` (caminho direto/URL): doctor ∥ load como hoje → `let mut terminal = ratatui::init(); let _restore = TerminalRestore; self.run_loaded(key, fresh, &mut terminal).await`.
2. No `launch(ResolvedLaunch::Review)`, braço `DiscoveryInput::CurrentBranch { provider, host, repository, branch }` passa a abrir o picker (o braço `Exact` continua `discover` → `launch_key`):

```rust
crate::context::DiscoveryInput::CurrentBranch { provider, host, repository, branch } => {
    let review_provider = self.providers.get(provider);
    let runner: Arc<dyn CommandRunner> = self.runner.clone();
    let doctor = Doctor::new(runner);
    let (report, listed) = tokio::join!(
        doctor.check(Some(provider), Some(&host)),
        review_provider.list_open(&host, &repository),
    );
    if !report.is_ready() {
        return Err(LaunchError::Dependencies(report.to_string()));
    }
    let list = listed?;
    if list.is_empty() {
        return Err(LaunchError::NoReview);
    }
    let store = JsonSessionStore::discover()?;
    let sessions: std::collections::BTreeSet<u64> = store
        .list()?
        .into_iter()
        .filter(|summary| {
            summary.key.provider == provider
                && summary.key.host == host
                && summary.key.repository == repository
        })
        .map(|summary| summary.key.number)
        .collect();
    let items = crate::tui::picker::mark_items(list, Some(branch.as_str()), &sessions);
    let mut terminal = ratatui::init();
    let _restore = TerminalRestore;
    let outcome = crate::tui::picker::run(
        &mut terminal,
        crate::tui::picker::PickerState::new(items),
        crate::tui::picker::PickerSource {
            provider: review_provider.clone(),
            kind: provider,
            host: host.clone(),
            repository: repository.clone(),
            branch: Some(branch.clone()),
            sessions,
        },
    )
    .await?;
    match outcome {
        crate::tui::picker::PickerOutcome::Quit => Ok(()),
        crate::tui::picker::PickerOutcome::Open { number, snapshot } => {
            let key = ChangeRequestKey { provider, host, repository, number };
            let fresh = match snapshot {
                Some(fresh) => fresh,
                None => review_provider.load(&key).await?,
            };
            self.run_loaded(key, fresh, &mut terminal).await
        }
    }
}
```

(Nota: `ProviderRegistry::get` retorna `Arc<dyn ReviewProvider>` — clonar para o `PickerSource`.)

3. `run_loaded` reutiliza o corpo atual: store/open/restore, save, renderer, runtime, initial render, `tui::run(terminal, app, runtime)`.

- [ ] **Step 1: Compilar e rodar a suite** — `cargo test` → PASS (as mudanças são de fiação; os testes de `tests/cli.rs` e `doctor.rs` continuam passando).
- [ ] **Step 2: Verificação manual (obrigatória)** — num repo com PRs: `./target/release/betterreview` → picker abre com a branch atual no topo; navegar; Enter num item prefetchado abre instantâneo; `r` recarrega; `q` sai; URL explícita continua indo direto.
- [ ] **Step 3: Commit** — `git commit -am "feat: open reviews from the repo picker"`

---

### Task 7: Fechamento

- [ ] **Step 1:** `cargo fmt && cargo clippy --all-targets` (0 warnings) e `cargo test` (suite inteira verde).
- [ ] **Step 2:** `cargo build --release`.
- [ ] **Step 3:** Atualizar `tasks/todo.md` (marcar Etapa 2 entregue, seção de revisão) e o rodapé/help se ganhou tecla nova (não ganhou — picker tem footer próprio).
- [ ] **Step 4: Commit** — `git commit -am "docs: record picker delivery"`

## Self-Review

- Spec coverage: contrato `list_open` (T1/T2), fluxo de launch com doctor ∥ listagem (T6), tela com branch pinada + marcadores (T3/T4), prefetch destacado com debounce/abort e espera no Enter (T3/T5), erro de lista com retry via `r` (T5/T6), lista vazia → NoReview (T6), alvo explícito intocado (T6). Fora de escopo respeitado (sem paginação/filtros).
- Sem placeholders; tipos batem entre tasks (`ChangeRequestSummary`, `PickerItem`, `PickerOutcome`, `run_loaded`).
- Commits: uma linha, sem trailers.
