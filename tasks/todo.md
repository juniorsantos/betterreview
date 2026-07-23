# Etapa 1 — Bugs + Performance (2026-07-23)

Contexto: usuário reportou (a) load do projeto muito lento, (b) não consegue salvar
comentário (Ctrl+S não chega ao app), (c) revisão não é submetida (mesmo Ctrl+S no modal).

Causas raiz encontradas:
- GitHub: 1 chamada GraphQL por arquivo (`load_blob`) só para o oid do blob — sequencial.
  O REST `/pulls/files` já retorna `sha` (confirmado em PR real, inclusive `removed`).
- GitLab: 6 chamadas de metadados em série + 1 REST por arquivo em série.
- Doctor: 5 subprocessos em série antes de qualquer coisa (`gh auth status` vai à rede).
- `identities_match` (restore.rs) exige base_blob E head_blob; providers só preenchem um
  → identidade nunca casa; as N chamadas hoje não têm efeito algum.
- Ctrl+S é XOFF (flow control) em muitos terminais — únicas teclas de salvar/submeter.

## Tarefas

- [x] 1. Teclas: Enter salva comentário / envia revisão; Alt+Enter quebra linha; Ctrl+S vira alias.
      Testes primeiro em tests/tui_keys.rs (expor handler de teclas); atualizar hints e snapshots.
- [x] 2. GitHub: usar `sha` do REST files; remover load_blob/BLOB_QUERY; teste atualizado
      exige zero chamadas extras de blob.
- [x] 3. restore.rs: identities_match casa com um único blob presente (teste novo primeiro).
- [x] 4. GitLab: metadados via try_join + blobs com buffered(8); fake do teste passa a ser
      keyed por endpoint (ordem não-determinística).
- [x] 5. Doctor: checks em paralelo (join); dentro do provider: version → join(help, auth).
- [x] 6. Verificação: cargo test completo + clippy.

## Revisão (Etapa 1)

- TDD em todos os itens: cada mudança teve teste vermelho antes (tui_keys novo; timing
  tests forçando sobreposição no GitLab e no Doctor; fixtures GitHub sem respostas de blob).
- Resultado: 125 testes passando, 0 falhas, clippy sem warnings, cargo fmt aplicado.
- GitHub: PR de N arquivos caiu de N+3 chamadas sequenciais para 3 fixas (+1 GraphQL por
  página de threads). GitLab: 6 metadados agora concorrentes; blobs 8 por vez. Doctor:
  ~2 janelas de latência em vez de 5.
- identities_match agora funciona com a evidência disponível (um lado do blob), então
  progresso de review sobrevive a rebase quando o arquivo não mudou.
- Teste `missing_blob_identity_resets_reviewed_progress` ajustado: fixture antiga tinha
  blob presente e igual (codificava o comportamento defeituoso); intenção preservada.
- Não commitado — aguardando ok do usuário.

## Rodada 2 (mesmo dia, feedback do teste do usuário)

- [x] Bug: `viewerPendingReview` não existe no schema GitHub → `reviews(states: PENDING, first: 1)`.
      Quebrava comentar, marcar arquivo (m) e submeter.
- [x] Bug: help não fechava (agora Esc/q/Enter/? fecham; teclas não vazam) e ficou compacto (51x7).
- [x] Bug: diálogo de quit sem dicas nem seleção → menu real com j/k + Enter + Esc e hints.
- [x] Tema GitHub Dark High Contrast (src/tui/theme.rs) em todos os painéis + cores +/- do delta.
- [x] Diff: coluna única de número de linha; cursor destaca a linha inteira.
- [x] Files: agrupado por diretório (referência do usuário), letra de status colorida,
      contadores +N -N à direita, `e` expande o painel (30↔50 colunas).
- [x] Perf rodada 2: doctor em paralelo com o load; GitHub threads/files/diff em paralelo;
      per_page=100 nos dois providers. Startup ~2s → ~0,8-1,2s (dominado por 2 RTTs).

## Etapas seguintes (aprovadas, specs pendentes)
- Etapa 2: seletor de PR/MR ao abrir no repo (branch atual destacada + lista, prefetch do
  destacado com debounce ~300ms). Brainstorm concluído; escrever spec + plano.
- Etapa 3: reforma visual/UX da TUI (tema, layout, statusline com feedback, ajuda).
