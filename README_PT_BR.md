# betterreview

[![CI](https://github.com/juniorsantos/betterreview/actions/workflows/ci.yml/badge.svg)](https://github.com/juniorsantos/betterreview/actions/workflows/ci.yml)

🇺🇸 [Read in English](README.md)

Revisão de código no terminal para pull requests do GitHub e merge requests do GitLab. Navegue o diff com teclas estilo vim/lazygit, comente inline como no GitHub, marque arquivos como revisados e envie a revisão — sem sair do terminal.

![Tela de revisão](assets/review.svg)

Rodando dentro de um repositório, o seletor lista as revisões abertas, mostra o status do HEAD atual e já faz o prefetch do PR destacado:

![Seletor de PRs](assets/picker.svg)

Aprove, solicite mudanças ou comente sem perder o diff e os rascunhos visíveis atrás do modal:

![Modal de aprovação da revisão](assets/approve-review.svg)

## Recursos

- Diff com paridade visual com o GitHub: fundo verde/vermelho fim a fim, uma única coluna de número de linha, nome do arquivo no topo e trechos ocultos expansíveis (`z`)
- Comentários inline em cartões: criar (`c`), editar (`e`), excluir (`x`), responder (`r`) e sugestões de código (`s`)
- Seleção de linha ou bloco (`v`) para comentar exatamente como no GitHub
- Cópia limpa para o clipboard: linha ou seleção atual (`y`) e hunk atual (`Y`)
- Compartilhamento da revisão: hunk como patch bruto (`p`) ou todos os comentários como Markdown (`C`)
- Links clicáveis no terminal para a revisão, o commit HEAD atual e o arquivo ativo no GitHub ou GitLab
- Arquivos em árvore com checkbox de revisado (`m`), pastas recolhíveis e arquivos gerados de-emfatizados
- Saltos rápidos: primeira/última linha do diff ou arquivo (`gg`/`G`), próximo hunk (`]h`), próximo comentário (`]c`), próximo arquivo (`]f`), próximo não revisado (`]u`)
- Busca no diff (`/`, `n`/`N`), suporte a mouse (scroll e clique)
- Sessão persistente: feche e retome a revisão de onde parou (`betterreview resume`)
- Status de revisão vinculado ao HEAD atual: um novo commit faz a revisão voltar automaticamente para não revisada
- Envio da revisão completa (`R`): aprovar, solicitar mudanças ou comentar

## Comparação

O betterreview é um cliente de revisão, não um visualizador de diff: ele lê o pull request, guarda seu progresso e publica a revisão de volta no forge. É esse o eixo que a tabela compara.

| Capacidade | [betterreview](https://github.com/juniorsantos/betterreview) | [hunk](https://github.com/modem-dev/hunk) | [lumen](https://github.com/jnsahaj/lumen) | [gh](https://cli.github.com) / [glab](https://gitlab.com/gitlab-org/cli) | [delta](https://github.com/dandavison/delta) |
| --- | --- | --- | --- | --- | --- |
| GitHub **e** GitLab | ✅ | ❌ | ❌ | um cada | ❌ |
| Publica a revisão no forge | ✅ | ❌ | ❌ | ✅ | ❌ |
| Comentário inline em linha ou seleção | ✅ | ❌ | ❌ | ❌ | ❌ |
| Respostas e resolução de thread | ✅ | ❌ | ❌ | ❌ | ❌ |
| Sugestão de código | ✅ | ❌ | ❌ | ❌ | ❌ |
| Aprovar / pedir mudanças | ✅ | ❌ | ❌ | ✅ | ❌ |
| Progresso por arquivo e por hunk | ✅ | ❌ | por arquivo | ❌ | ❌ |
| Sessão sobrevive ao fechar | ✅ | ❌ | ❌ | — | ❌ |
| TUI voltada à revisão | ✅ | ✅ | ✅ | ❌ | ❌ |
| Lado a lado e unificado | ✅ | ✅ | ✅ | ❌ | ✅ |
| Contexto oculto expansível | ✅ | ✅ | ✅ | ❌ | ❌ |
| Busca dentro do diff | ✅ | ✅ | ✅ | ❌ | ❌ |
| Suporte a mouse | ✅ | ✅ | ✅ | ❌ | ❌ |
| Realce de sintaxe | via delta | ✅ | ✅ | ❌ | ✅ |
| Dispensa chave de provedor de IA | ✅ | ✅ | para recursos de IA | ✅ | ✅ |
| Ponte para anotações de agente | [planejado](https://github.com/juniorsantos/betterreview/issues/6) | ✅ | ✅ | ❌ | ❌ |
| Revisa diff local sem PR | [planejado](https://github.com/juniorsantos/betterreview/issues/22) | ✅ | ✅ | ❌ | ✅ |
| Funciona como pager / difftool | ❌ | ✅ | ❌ | ❌ | ✅ |
| Diff estrutural | ❌ | ❌ | ❌ | ❌ | ❌ |

`gh` e `glab` publicam revisões, mas cada um fala com um forge e nenhum renderiza o diff para revisar — você comenta por arquivo e número de linha, não apontando para o código. `hunk` e `lumen` são visualizadores com anotações locais: ótimos para ler um changeset, mas a revisão nunca chega ao pull request.


## Dependências

| Ferramenta | Para quê | Instalação |
|---|---|---|
| [git](https://git-scm.com) | contexto do repositório | já vem no macOS/Linux |
| [gh](https://cli.github.com) | PRs do GitHub (`gh auth login`) | `brew install gh` |
| [glab](https://gitlab.com/gitlab-org/cli) | MRs do GitLab (`glab auth login`) | `brew install glab` |
| [delta](https://github.com/dandavison/delta) | renderização do diff | `brew install git-delta` |

Rode `betterreview doctor` para verificar se está tudo pronto.

## Instalação

### Homebrew (macOS e Linux)

```sh
brew tap juniorsantos/tap
brew install betterreview
```

Ou em um comando só: `brew install juniorsantos/tap/betterreview`.

Instala `gh`, `glab` e `delta` automaticamente como dependências.

### Binário do release

Baixe o binário da sua plataforma na [página de releases](https://github.com/juniorsantos/betterreview/releases):

```sh
# macOS Apple Silicon
VERSION=v1.2.0
curl -sSL "https://github.com/juniorsantos/betterreview/releases/download/${VERSION}/betterreview-${VERSION}-aarch64-apple-darwin.tar.gz" | tar xz
sudo mv betterreview /usr/local/bin/
```

Targets disponíveis: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`.

### Via cargo

```sh
cargo install --git https://github.com/juniorsantos/betterreview
```

## Uso

```sh
# dentro de um repositório: abre o seletor de PRs/MRs abertos
betterreview

# direto por URL
betterreview https://github.com/owner/repo/pull/42

# retomar a última sessão de revisão
betterreview resume

# listar sessões salvas
betterreview sessions

# checar dependências e autenticação
betterreview doctor
```

### Autocompletar do shell

O Homebrew instala automaticamente. Para outras instalações:

```sh
betterreview completions zsh  > ~/.zfunc/_betterreview     # zsh
betterreview completions bash > ~/.local/share/bash-completion/completions/betterreview
betterreview completions fish > ~/.config/fish/completions/betterreview.fish
```

### Atalhos principais

| Tecla | Ação |
|---|---|
| `j`/`k` | mover cursor |
| `gg` / `G` | ir para a primeira / última linha do diff ou arquivo |
| `Tab`, `2`/`3` | alternar foco entre Arquivos e Diff |
| `v` | iniciar/encerrar seleção de linhas |
| `c` | comentar na linha ou seleção |
| `s` | sugerir código na seleção |
| `y` / `Y` | copiar a linha ou seleção atual / hunk atual |
| `p` / `C` | copiar o hunk como patch bruto / todos os comentários como Markdown |
| `e` / `x` / `r` | editar / excluir / responder comentário sob o cursor |
| `m` | marcar arquivo como revisado |
| `z` | expandir trecho oculto do diff (ou recolher pasta no painel Arquivos) |
| `]h` `[h` / `]c` `[c` | próximo/anterior hunk / comentário |
| `]f` `[f` / `]u` `[u` | próximo/anterior arquivo / arquivo não revisado |
| `/`, `n`/`N` | buscar no diff |
| `R` | enviar revisão (aprovar / solicitar mudanças / comentar) |
| `?` | ajuda com todos os atalhos |
| `q` | sair |

## Desenvolvimento

```sh
cargo test          # suíte completa
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo run --example readme_screenshots
```

Releases são automáticos: commits `feat:`/`fix:` na `main` geram bump de versão semântica, tag e release com binários via GitHub Actions.

## Licença

Distribuído sob a licença MIT. Veja [LICENSE](LICENSE) e [NOTICE](NOTICE).
