# M6 — resultados locais

Data: 2026-09-06. Projeto experimental de portfólio. Não é uma auditoria de segurança.
Os identificadores abaixo apontam para diretórios locais sob `target/m6/` (ignorados
pelo Git), com logs, parâmetros e status por comando. Falhas preliminares não foram
apagadas nem convertidas retroativamente em sucesso.

## Funcionalidade e campanhas

| Execução | Evidência | Resultado |
|---|---|---|
| Demo inicial | 20260906-010411-af4d383978354e639088b4182c3f2dea | FAIL; binário default substituiu pqc compartilhado |
| Demo com binário por execução | 20260906-010703-6c4ddcf4b33a4ebbb649a65bfb26ed67 | PASS |
| Fuzz Smoke inicial | 20260906-010230-010b0561dc50445c9af6efa21b626dce | FAIL; replay vazio tratado como path |
| Fuzz Smoke corrigido | 20260906-010453-0f8d09bb8cfa481baf2239110035ef66 | PASS |
| Fuzz Full + replay | 20260906-011043-8956b319101d4af89dea3b8de42d058b | PASS |
| Bench Smoke | 20260906-011338-397af1b0b6bc46b9b7f35614fc09f51c | PASS |
| Bench Full | 20260906-011438-47b68d92cb4a40078fc19330db12500b | PASS |
| Gates Verify | 20260906-012521-0d3268cf8812447182bcdf6f8dd5e4fd | PASS; ressalva reparse abaixo |
| Demo final com no-clobber | 20260906-013336-2497a890b3af43cfa7eabc138e5c543b | PASS |

Full executou quatro seeds (5060163–5060166): 40.000 fontes parser, 160.000 casos
das quatro propriedades M2, 1.024 mutações M4 e 1.024 mutações M5, além do corpus
inicial e dois replays de artefatos concretos. Não houve falha/skip nessas campanhas.
São campanhas generativas e mutacionais; não há cobertura guiada ou prova de ausência de bugs.

O verifier externo rejeitou uma parte Parquet com valor alterado mesmo após atualizar
seu receipt com tamanho/hash corretos. Foram acrescentados testes para metadata
duplicada, incompleta, oversize e trailing e namespace desconhecido; esses casos
adicionais passaram nos gates finais. Usa DTOs próprios locais.

## Medições

Bench Full produziu 144 amostras verificadas, incluindo 18 warmups (7 medidas por
cenário). Smoke só comprova funcionamento do tooling, não é baseline de desempenho.
Cada arquivo é verificado antes de aceitar a amostra. Entrada sintética repete uma
fixture artesanal; setup, leitura de verificação e cleanup ficam fora da janela.

Working set é o maior PeakWorkingSet64 observado enquanto o processo vive, não pico
exato nem heap Rust. Valores ausentes são null. Tempos externos incluem startup.
Compilações são feitas antes das operações medidas; processos de benchmark são sequenciais.

| Operação | Registros / payload | Batch | Mediana ms | Maior memória observada MiB |
|---|---:|---:|---:|---:|
| M3 | 300.000 | 256 | 533,08 | 18,46 |
| M3 | 300.000 | 4.096 | 358,90 | 6,52 |
| M3 | 300.000 | 65.536 | 278,03 | 14,54 |
| M3 | 3.000.000 | 256 | 3.401,83 | 131,69 |
| M3 | 3.000.000 | 4.096 | 2.524,80 | 13,88 |
| M3 | 3.000.000 | 65.536 | 2.279,18 | 15,39 |
| M4 Create | 300.000 | 256 | 27.092,16 | 6,28 |
| M4 Create | 300.000 | 4.096 | 2.129,12 | 6,59 |
| M4 Create | 300.000 | 65.536 | 430,46 | 15,17 |
| M4 Create | 3.000.000 | 65.536 | 4.163,58 | 15,29 |
| Resume validação | 300.000 | 256 | 938,72 | 4,98 |
| Resume validação | 300.000 | 4.096 | 210,09 | 4,99 |
| Resume validação | 300.000 | 65.536 | 117,63 | 6,99* |
| Resume validação | 3.000.000 | 65.536 | 234,74 | 7,05 |
| Protect | 1 MiB | — | 147,94 | 5,28 |
| Protect | 64 MiB | — | 2.939,66 | 5,28 |
| Unprotect | 1 MiB | — | 144,38 | 5,29 |
| Unprotect | 64 MiB | — | 1.182,56 | 5,29 |

*Memória disponível em 6/7 amostras nesse cenário; nos demais, 7/7.
Há dispersão relevante: consultar [mínimos/máximos](evidence/m6-file-summary.json)
e [amostras brutas](evidence/m6-file-samples.jsonl). M3 batch256 não apresenta memória
global constante: o footer cresce com row groups, conforme contrato existente.
M4 batch65536 e M5 apresentaram memória observada próxima entre escalas; essa
observação local não constitui prova de um teto universal nem análise causal.

Micro Full separado: 6 cenários, 1 warmup +7 amostras, janela >=250ms,
com verificação independente após cada amostra. Medianas em ns/iteração:

| Fixture | Compile | Decode | Registros por decode |
|---|---:|---:|---:|
| Mista | 21.148 | 171.722 | 768 |
| Texto | 3.529 | 7.114 | 256 |
| Numérica | 10.791 | 18.623 | 256 |

[Amostras micro](evidence/m6-micro-samples.jsonl) preservam iterações, duração e bytes.
Comando: `cargo bench --bench m6 -- --profile full` (exit 0), após Verify, sem
outra campanha concorrente. Compile inclui parse+compile; decode usa layout pré-compilado.

Host: Windows 10.0.19045, PowerShell 7.6.5, Rust 1.95.0 MSVC, NTFS fixo,
AMD64 Family23 Model24, 8 processadores lógicos. A [proveniência](evidence/m6-environment.json)
registra hashes de fontes, fixtures, lock e executáveis isolados por execução.
O benchmark de arquivos precede o ajuste de parsing da flag de relatório após
argumento desconhecido e a expansão do microbench; não mede esses ajustes finais.
Não houve alteração do pipeline M3–M5 entre essas medições e os gates finais.

## Gates executados

Todos com exit 0 no Verify acima:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets`
- `cargo test --all-targets --all-features`
- `cargo test --doc` e `cargo test --doc --all-features`
- `cargo test --release --test protection --all-features`
- Teste reparse explícito com `--nocapture`: harness passou, teste pulado por 1314.

Após corrigir apenas o campo de bytes por iteração do microbench, fmt e Clippy
foram repetidos com sucesso, assim como o próprio microbench Full. `git diff --check`
passou (avisos locais de conversão LF/CRLF, sem erro de whitespace).

## Limitações e estado

- Validação dinâmica de reparse point historicamente pula com erro Windows 1314.
  Um `ok` do harness após skip não é evidência dessa proteção.
- CI foi configurada em arquivo local; nenhuma execução remota foi disparada.
- Stress M4 3M/batch256 é opcional e não foi executado.
- Métricas do host e dados sintéticos não sustentam SLA, produção ou avaliação de segurança.
- Não há desvio de arquitetura ou problema de correctness identificado pendente.
