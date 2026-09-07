# M6 — evidência técnica local

Status: implementação local concluída. O plano e seu adendo estão em M6_PLAN_WIP.md;
resultados medidos, gates e ressalvas estão em [M6_RESULTS.md](M6_RESULTS.md).
APIs de biblioteca e formatos M0–M5 permanecem congelados.

## Resumo CLI v1

Os comandos existentes aceitam `--report-json`, sem valor, após o comando e fora
dos valores de outras flags. Emite um objeto JSON e newline em stdout após retorno
normal; diagnósticos humanos permanecem em stderr. Não há progresso interno.
M5 exige `pqc`. Sem a flag, sucesso continua silencioso e diagnósticos de uso
permanecem byte a byte iguais aos cinco comandos M5; a descoberta da flag fica nesta
documentação, sem inserir texto M6 no stderr legado.

Campos: report_version=1, command, mode, status, elapsed_ms, error_category,
input_bytes, output_bytes, dataset_records, dataset_parts, batch_records,
record_length, publication e warnings. Valores desconhecidos/inaplicáveis são null.
Warnings são códigos disponíveis no outcome, sem paths. Tempo é monotônico e inclui
dispatch/parsing/operação; exclui coleta final e emissão. Input é metadata observada,
não quantidade de bytes processados. Totais M4 são do dataset, inclusive em Resume.
Output size é observado apenas após sucesso de operação com saída de arquivo único.
Publication vem exclusivamente do outcome M5; erro de keygen, mesmo após primeiro
commit, usa null e não reconstitui estado pelo filesystem. Null não implica rollback.
Categorias: arguments, copybook, input_io, conversion, recovery, protection.
Não expor detalhes criptográficos, dados, caminhos, chaves ou fingerprints no JSON.
Falha de relatório é best-effort: aviso, sem mudar exit code ou refazer a operação.
Crash não garante relatório. Redirecionamentos devem ficar fora do namespace M4.

## Matriz de gaps da campanha

| Invariante | Evidência anterior | Incremento M6 |
|---|---|---|
| Numéricos/layout/batches | Quatro propriedades em decode_properties | Mesmas propriedades com seeds/cases ampliados |
| Fonte arbitrária | Parser, 256 fontes ASCII | Mesmo teste com orçamento/seed controlados |
| Resume rejeita sem cleanup | Casos individuais de metadata/namespace | Combinações de mutações com snapshots |
| M5 rejeita sem publicação | Corpus fixo, tamper e closeout | Mutações estruturadas de envelope/chaves e replay concreto |
| Vetores e crash M4 | Golden, matrizes completas de faults | Reutilizar sem duplicar |

Campanhas nativas generativas/mutacionais com proptest, sem feedback de cobertura.
Smoke: 128 casos puros/8 filesystem; Full: 10.000/256, seeds 0x4D3643–0x4D3646.
Parser/decoder: buffers <=64 KiB; M4 <=3 partes; M5 <=2 MiB+17 plaintext.
Ao encontrar falha gerada, o harness persiste o caso final minimizado com família,
origem, seed, número da avaliação, configuração e commit revisado. O replay lê esse
caso antes de consultar seed/cases. Um autoteste força uma falha apenas em subprocesso
de teste, exige exit não zero, valida o artefato, repete a falha sem PRNG e confirma
um replay conhecido-success. O `replay.json` preservado em campanhas sem falha é só
uma prova do caminho de replay representativo; não prova ausência de bugs. Timeout,
abort ou OOM é falha/inconclusivo, nunca sucesso. Fixtures não são dados de produção.

## Medições e demo

Runner local: scripts/m6.ps1, modos Verify, Fuzz, Bench e Demo; perfis Smoke/Full.
Stress é opt-in apenas para Bench M4 3M/batch256 e não integra gates.
Bench usa release, preparação e verificação fora do tempo, 1 warmup e 3/7 amostras.
M3 Full: 300k/3M, batches256/4096/65536. M4 Full: 300k nesses batches;
3M só batch65536. Resume mede validação. M5: 1/64 MiB, operações separadas.
Microcasos: compile e decode, std Instant/black_box, >=25/250ms por amostra.
Dados de escala repetem a fixture artesanal de três registros; não representam
distribuição de mainframe. Cada saída é verificada contra constantes independentes.
Receipts/hash não substituem comparação semântica de Parquet. Verifier externo usa
DTOs locais, nunca exports dos tipos privados M4.

PeakWorkingSet64 mede working set do processo Windows, não heap Rust. M3 retém
metadados por row group; M4/M5 processam dados por parte/chunk. Não há SLA ou teto
universal de RSS. Comparar escala 10x a batch fixo e batch variável separadamente.
Guardar ambiente, hash de lock/dados/binário, commit/dirty, parâmetros, amostras e
disponibilidade da métrica; cache warm sem purge, execução sequencial.

O runner consulta PeakWorkingSet64 enquanto o processo está vivo (intervalo de
20 ms) e guarda o maior valor observado como `observed_peak_working_set_bytes`.
É um limite inferior do pico real: pode perder uma alta final ou processo muito
curto; ausência de observação resulta em null. Não alegar pico exato de heap/RSS.
O cronômetro externo inclui startup/espera do processo. Executáveis release são
copiados por execução, com hashes, para builds default posteriores não substituírem
o binário pqc medido. Não rodar outras campanhas/builds durante Bench.

```powershell
./scripts/m6.ps1 -Mode Verify
./scripts/m6.ps1 -Mode Demo
./scripts/m6.ps1 -Mode Fuzz -Profile Full
./scripts/m6.ps1 -Mode Fuzz -Replay <case.json> -ReplayKind m4
./scripts/m6.ps1 -Mode Bench -Profile Full
# Opcional, fora dos gates:
./scripts/m6.ps1 -Mode Bench -Profile Stress
```

Cada diretório de execução em target/m6 contém environment.json, commands.json,
result.json e logs por comando. Bench acrescenta samples.jsonl e summary.json;
Fuzz guarda um replay.json concreto de exemplo por família/seed, mesmo em sucesso.
Failures permanecem registradas; uma execução posterior não reclassifica a anterior.
Datasets de escala e chaves da demo/bench são removidos após verificação, apenas
dentro do diretório exclusivo. Não compartilhar diretórios de execução inteiros.

CI fixa [checkout v4.2.2](https://github.com/actions/checkout/releases/tag/v4.2.2)
e [upload-artifact v4.6.2](https://github.com/actions/upload-artifact/releases/tag/v4.6.2)
por commit; usa Rust 1.95.0. Publica apenas manifests/resumos/amostras, excluindo
corpus de teste, chaves, dados e diagnósticos brutos. A execução remota
[34012733963](https://github.com/01110100chony/m2c-binary-engine/actions/runs/34012733963)
passou em Windows para o commit `8d44218605a59a190590772fa52232c5859c9bc8`,
com Verify, Fuzz Smoke, Demo, Bench Smoke e upload. Full continua exclusivamente local.
O status do step não distingue execução dinâmica de reparse de skip; não atribuir
essa cobertura ao run sem o respectivo log/artefato. A remediação posterior tem gates locais.

## Gates

G0: contrato/adendo; G1: API/formatos/deps preservados; G2: fmt, clippy -D warnings,
test --all-targets com/sem all-features, doctests com/sem all-features; G3: campanhas
Full e testes adversariais release; G4: Demo e Bench Full; G5: evidências rastreáveis.
Skips ambientais são explícitos. Symlink error1314 não prova rejeição dinâmica.
Staging M5 após crash e demais limitações M0–M5 continuam vigentes.
