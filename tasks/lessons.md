# Lições (betterreview)

## 2026-07-23 — Validar campos GraphQL contra o schema real
O código usava `viewerPendingReview`, um campo que **não existe** no schema do
GitHub — todas as mutações (comentar, marcar arquivo, submeter) falhavam.
**Regra**: antes de escrever/alterar uma query GraphQL, confirmar os campos com
introspecção real (`gh api graphql -f query='{ __type(name: "...") { fields { name } } }'`).
Nunca escrever queries de memória.

## 2026-07-23 — Nunca depender de teclas que o terminal captura
Ctrl+S é XOFF (flow control) e não chega ao app em muitos terminais. Salvar e
submeter dependiam só dela. **Regra**: ações críticas precisam de uma tecla
primária que sempre chega (Enter, Esc, letras simples); modificadores Ctrl+X
só como alias. Todo modal precisa mostrar suas teclas na tela — o diálogo de
quit tinha opções sem nenhuma dica e o usuário ficou preso.

## 2026-07-23 — Chamadas externas em série são o gargalo padrão
Load fazia 1 chamada por arquivo (GitHub e GitLab) + 5 checks do doctor em
série. **Regra**: em qualquer fluxo com N chamadas de subprocess/rede, começar
pela pergunta "isso pode ser 1 chamada?" (o REST de files já trazia o sha) e
só depois paralelizar o que restar. Testes de timing com fake delays garantem
que a concorrência não regride.

## 2026-07-23 — Dados que não produzem efeito são bug, não só custo
Os blobs alimentavam `identities_match`, que exigia os dois lados quando os
providers só preenchem um — nunca casava. **Regra**: ao tocar num dado caro,
verificar quem consome e se o consumo é alcançável.
