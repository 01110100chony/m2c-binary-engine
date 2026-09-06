# Planejamento M6 — checkpoint persistente

Este documento registra o planejamento, não autoriza nem afirma implementação do M6.
Atualizar incrementalmente após cada descoberta ou decisão relevante. Antes de avançar,
marcar o estágio como COMPLETE e registrar questões abertas. Preservar histórico e
registrar correções explicitamente. Não iniciar M7 automaticamente.

**Estado atual: implementação local M6 concluída em 2026-09-06; ressalvas em M6_RESULTS.md.**
Para retomar, ler o CHECKPOINT FINAL e os requisitos R1–R4 do CHECKPOINT 5.
Pendências e resultados provisórios nas seções anteriores são históricos; seus
desfechos estão nos adendos posteriores e no resumo final.

## CHECKPOINT 0 — estado inicial e fontes lidas — COMPLETE

- Solicitação: planejar M6, mantendo este arquivo suficiente para retomada por outro agente.
- Ambiente: `C:/projetos2026/pqc-mainframe-db`, PowerShell; data informada: 2026-09-05.
- Estado inicial: `git status --short` sem alterações; este arquivo não existia.
- Fontes lidas: instruções AGENTS.md fornecidas na conversa e pedido do usuário.
- Inspeção inicial limitada a diretório, estado Git e existência do checkpoint; análise profunda ainda não iniciada.
- Restrições: arquitetura/subconjunto congelados; priorizar correção, testes e simplicidade; não implementar funcionalidades durante este planejamento.
- Questões abertas: estado efetivo M0–M5; definição prévia de M6; ADRs aplicáveis; gates e evidências existentes.
- Próxima ação: ler AGENTS.md local, README, arquitetura, subconjunto, ADRs e documentação de milestones; registrar CHECKPOINT 1.

## Índice de checkpoints

- CHECKPOINT 1 — estado atual M0–M5 e arquitetura relevante — COMPLETE (registro abaixo)
- CHECKPOINT 2 — gaps e oportunidades candidatas para M6 — COMPLETE (registro abaixo)
- CHECKPOINT 3 — alternativas de escopo e trade-offs — COMPLETE (registro abaixo)
- CHECKPOINT 4 — escopo M6 recomendado — COMPLETE (registro abaixo)
- CHECKPOINT 5 — requisitos e invariantes — COMPLETE (registro abaixo)
- CHECKPOINT 6 — plano de implementação — COMPLETE (registro abaixo)
- CHECKPOINT 7 — testes, gates e acceptance criteria — COMPLETE (registro abaixo)
- CHECKPOINT FINAL — plano consolidado — COMPLETE (registro abaixo)

## CHECKPOINT 1 — estado atual M0–M5 e arquitetura relevante — COMPLETE

### Fontes e evidência

- Lidos: AGENTS.md local, README.md, Cargo.toml, src/lib.rs, src/main.rs,
  src/pipeline.rs, docs/ARCHITECTURE.md, docs/COPYBOOK_SUBSET.md,
  docs/M5_INDEPENDENT_REVIEW.md, tests/fixtures/README.md e trechos dos testes
  de propriedades/adversariais. Índices e referências M6 dos contratos M2/M4/M5 inspecionados;
  leitura dos detalhes relevantes continuará no checkpoint 2.
- HEAD inicial: `fec65e0` (`m5:closeout`); histórico recente contém entregas M1–M5.
- README declara M0–M5 implementados. Código confirma um pacote com biblioteca e CLI
  em `src/main.rs` (não existe src/cli.rs), conversão M3, recuperação M4 e módulo M5 opcional.
- M0: fundação documental; M1: parse/compile com layout público e diagnósticos;
  M2: CP037 e numéricos para Arrow; M3: Parquet síncrono em batches;
  M4: partes, recibos, lock, identidade e resume; M5: proteção autônoma sob `pqc`.
- `docs/adr/` não existe neste checkout. Os contratos vigentes estão nos documentos
  principais; não inventar ADRs existentes. Também não existem `.github/`, `benches/`,
  `fuzz/` ou `scripts/`.
- Há fixtures anotadas independentes do decoder, tabela pública CP037 e fixture externa
  OpenSSL/Python M5. A fixture fixed-record é artesanal (3 registros, 105 bytes),
  não captura real de mainframe. Proptest já é dependência de desenvolvimento.
- Revisão M5 relata testes históricos aprovados, mas isso não equivale a validação
  executada nesta sessão. Relata symlink sem privilégio, interoperabilidade externa
  unidirecional e harness adversarial temporário removido.

### Arquitetura relevante e direção já definida

- O roadmap define explicitamente **M6 — evidência técnica e demo: observabilidade
  local, fuzzing ampliado, benchmarks reproduzíveis e demonstração documentada**.
- Público: portfólio estudantil e leitores técnicos que precisam reproduzir evidências.
- Preservar compilação uma vez, subset congelado, decimais exatos, execução síncrona,
  memória de dados limitada por batch/chunk e APIs/formatos M3–M5.
- M3 pode deixar arquivo parcial e acumula metadados de footer; M4 valida integralmente
  entrada/prefixo na retomada; M5 exige Windows/MSVC + NTFS local, sem integração M4.
- Staging plaintext após crash, atomicidade por arquivo em keygen e best-effort de
  permissões/zeroização são limitações documentadas, não autorização para redesenho M6.
- Cloud, ML-DSA, novos formatos e demais extensões pertencem a M7/opcionais.
- A análise histórica `ANALISE_DO_PROJETO.md` descreve uma versão anterior; sua proposta
  de async/cloud não substitui arquitetura congelada ou roadmap atual.

### Questões abertas e retomada

- Determinar o mínimo útil de telemetria sem ampliar APIs nem afetar commit.
- Escolher fuzzing compatível com alvo Windows e explicitar cobertura versus geração aleatória.
- Definir workloads, medições e demo sem transformar dados sintéticos em único oráculo.
- Confirmar detalhes dos testes M4/M5, ferramentas locais e gates antes de consolidar.
- Próxima ação: análise dirigida dos gaps e viabilidade; registrar CHECKPOINT 2 antes de alternativas.

## CHECKPOINT 2 — gaps e oportunidades candidatas para M6 — COMPLETE

### Descobertas adicionais

- Lidos contrato DECODING integral, seções de commit/retomada/aceite M4, gates e
  invariantes M5; inspecionados pontos de entrada, hooks e testes permanentes M4/M5.
- CLI usa args_os e retorna sucesso silencioso; erros e avisos M5 vão para stderr.
  Não há estatísticas públicas M3/M4 (retornam `Result<(), ...>`).
- M4 já tem fault injection determinística privada, subprocessos e matriz de interrupções.
  Não substituir isso por sleeps ou flags de falha em produção para a demo.
- Propriedades M2: quatro grupos, 256 casos/seed fixa, sem persistência automática
  (`failure_persistence: None`). Não há campanha ampliada/replay de corpus versionado.
- Correção da leitura preliminar da revisão M5: o checkout **já contém** regressões
  permanentes `closeout_*` para tag final vazia, comprimento enorme e falha em frame
  posterior. Não recriar cegamente todo o harness histórico removido.
- Toolchain disponível: rustc/cargo 1.95.0, host `x86_64-pc-windows-msvc`; apenas
  toolchains stable GNU/MSVC listadas, sem nightly. Versões observadas, não pin obrigatório.

### Gaps priorizados

| Gap | Evidência atual | Oportunidade M6 |
|---|---|---|
| Observabilidade | CLI sem resumo de execução; biblioteca sem estatísticas | Resumo JSON opt-in na fronteira CLI, sem instrumentar hot path por padrão |
| Fuzzing ampliado | Proptest M2 pequeno e testes adversariais fixos | Campanhas de propriedades/mutação com seed, orçamento, corpus e replay; avaliar coverage-guided separadamente |
| Desempenho | Sem benches/scripts/baselines | Harness release reproduzível, medições separadas de parser/decoder e operações locais |
| Memória | Contratos e inspeção estrutural, sem relatório de escala | Medir memória do processo e crescimento com dataset/batch, respeitando footer M3 |
| Demonstração | Exemplos README e testes dispersos | Roteiro executável: conversão, retomada via testes existentes e round-trip/tamper M5 |
| Reprodutibilidade | Gates documentados, sem workflow versionado | Runner local obrigatório, CI Windows complementar, manifesto de ambiente e resultados |
| Interoperabilidade M5 | Fixture externa lida pelo M2C | Direção inversa externa é candidata complementar; implica tooling Python/OpenSSL |
| Evidência ambiental | Revisão histórica relata symlink pulado | Relatar skips explicitamente; executar em ambiente apto antes de alegar essa cobertura |

### Questões abertas e próximos passos

- Pergunta opcional enviada ao usuário: resumo JSON por comando (padrão recomendado)
  versus progresso interno por batch/parte. Aguardar enquanto avança na investigação independente.
- Comparar campanha nativa stable/proptest com cargo-fuzz, sem chamar teste aleatório
  de coverage-guided. Não instalar ferramentas durante planejamento.
- Gates históricos ainda não reexecutados nesta sessão; baseline deve ser separado
  de qualquer resultado futuro M6.
- Próxima ação: registrar alternativas/trade-offs, escolher defaults explícitos e delimitar escopo.

### Atualização após CHECKPOINT 2

- Usuário escolheu explicitamente **resumo JSON por comando: resultado, duração e
  volumes verificáveis, preservando APIs e fluxo atuais**. Questão de profundidade resolvida.
- `cargo fuzz --version` falhou: comando não instalado. `Get-Volume -DriveLetter C`
  retornou acesso negado; filesystem não confirmado por esse comando. Não elevar
  privilégio só para planejar; gates M5 deverão registrar preflight suportado.
- M5 diferencia erros tipados internamente, mas intencionalmente unifica textos de
  fingerprint/chave errada e autenticação. JSON deve preservar essa uniformidade.
- M4 rejeita arquivos desconhecidos no namespace; relatório M6 deve ficar fora dele.

### Evidências de viabilidade e baseline (em andamento)

- `cargo fmt --all -- --check`: PASS, exit 0 nesta sessão.
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS, exit 0 nesta sessão.
- `cargo test --all-targets`: iniciado; resultado ainda pendente neste registro.
- Documentação primária consultada: [Rust Fuzz Book — setup](https://rust-fuzz.github.io/book/cargo-fuzz/setup.html)
  e [Windows](https://rust-fuzz.github.io/book/cargo-fuzz/windows.html). Cargo-fuzz
  suporta Windows com MSVC AddressSanitizer e requer nightly/tooling adicional.
  Ausência local não significa incompatibilidade de plataforma.
- [Proptest TestRunner](https://docs.rs/proptest/latest/proptest/test_runner/struct.TestRunner.html)
  documenta RNG configurável, shrinking e persistência de falhas; confirmar interfaces
  na versão de Cargo.lock durante implementação, sem atualizar dependência por hábito.
- Pergunta opcional enviada sobre proptest/mutações (padrão enxuto) versus incluir
  também cargo-fuzz coverage-guided. Ainda não consolidar essa escolha como resposta do usuário.

## CHECKPOINT 3 — alternativas de escopo e trade-offs — COMPLETE

| Decisão | Alternativas e custo | Recomendação |
|---|---|---|
| Amplitude | Só documentação deixaria roadmap incompleto; plataforma de observabilidade excederia o objetivo | Quatro entregas locais: resumo, campanha ampliada, medições e demo |
| Telemetria | Resumo CLI não vê progresso interno; callbacks/estatísticas exigiriam novas APIs e pontos críticos | Resumo CLI opt-in, escolha confirmada pelo usuário |
| Canal | Arquivo gerido pela aplicação exigiria lifecycle/permissões/isolamento; stdout deixa persistência ao chamador | `--report-json`, um objeto JSON em stdout; diagnóstico humano permanece em stderr |
| Fuzzing | Proptest já disponível, portable/stable, sem feedback de cobertura; cargo-fuzz adiciona nightly/sanitizer e harness | Proptest + mutação + corpus/replay como base; consulta sobre coverage-guided pendente |
| Benchmark | Criterion adicionaria dependência; cronômetro manual mal definido produziria ruído | Target Cargo bench com `harness = false`, std::time::Instant/black_box e runner PowerShell, dados crus e limites explícitos |
| Medidas | Só throughput mistura etapas e caches; instrumentar tudo aumenta superfície | Parser/decoder isolados e operações completas M3/M4/M5, memória do processo separada |
| Recuperação na demo | Matar por tempo é instável; flag de crash de produção quebra fronteira | Reutilizar teste privado determinístico M4; CLI demonstra Create/Resume concluído |
| Evidência externa | Direção M2C -> Python/OpenSSL agrega confiança mas amplia setup | Preservar fixture independente atual; não tornar tooling externo gate M6 base |
| Automação | Workflow remoto apenas não atende reprodução local; infraestrutura extensa é excessiva | Runner local versionado; CI Windows de regressão/smoke sem benchmark competitivo |

- NTFS confirmado por `[System.IO.DriveInfo]::new('C').DriveFormat`; isso não substitui
  validação Win32 de drive fixo/reparse/ACL feita pelo M5 nos testes.
- Cargo.lock usa proptest 1.11.0; nenhuma atualização ou instalação é necessária para
  a campanha nativa. Não expor módulos privados só para testes/benchmarks.
- Não incluir otimização de throughput como entrega: medir primeiro. Uma futura
  otimização só será proposta se houver gargalo demonstrado.
- Questões abertas: resposta sobre cargo-fuzz; detalhar semântica do relatório em
  falha/resume e orçamentos dos experimentos. Próxima ação: CHECKPOINT 4.

### Atualização de baseline após CHECKPOINT 3

- `cargo test --all-targets`: PASS, exit 0; 117 testes aprovados, zero falhas e zero
  ignored declarados pelo harness. Código M5 não compilado nessa execução.
- `cargo test --all-targets --all-features`: iniciado, resultado pendente.
- Um teste que retorna cedo por limitação ambiental pode aparecer como `ok` e não
  como ignored; não usar a contagem acima para afirmar cobertura real de symlinks.

## CHECKPOINT 4 — escopo M6 recomendado — COMPLETE

**M6: evidência reproduzível do pipeline local M0–M5.** Entrega orientada a um
avaliador técnico conseguir executar os exemplos, verificar resultados exatos,
reproduzir entradas adversariais e interpretar medições sem pressupor produção.

1. **Resumo JSON opt-in dos cinco comandos existentes** (`convert`, `convert-parts`,
   `keygen`, `protect`, `unprotect`), com resultado, tempo e volumes observáveis.
   M5 continua condicionado a `pqc`. Nenhuma alteração de assinatura de biblioteca.
2. **Campanha nativa ampliada** com proptest e mutação estruturada para parser/AST,
   layout/decoder, estado M4 e codecs/envelope M5, corpus persistente e replay.
   Defaults: smoke determinístico nos gates comuns e campanha ampliada explícita.
3. **Benchmarks reproduzíveis** de compilação, decoding, M3, M4 Create/Resume
   concluído, M5 protect/unprotect; memória e escala medidas em processos isolados.
4. **Demo local executável e documentação de evidências**, incluindo resultados
   independentes da fixture, recuperação determinística em teste e falha por tamper.
5. **Runner local de validação e CI Windows smoke**, para empacotar as quatro
   entregas. Não publicar/configurar serviços nem instalar ferramentas nesta sessão.

### Defaults e exclusões

- Default de fuzzing: campanha native/stable; consulta de cargo-fuzz ainda sem resposta
  no momento deste registro, após oportunidade de resposta. Se usuário optar por
  coverage-guided, registrar adendo e revisar checkpoints dependentes antes de consolidar.
- Sem Criterion, tracing/log framework, dependência runtime nova, exporter, servidor,
  callback público, processamento assíncrono, mudança COBOL, compressão ou otimização.
- Não adicionar integração M4+M5, cleanup M5 pós-crash, transação keypair, assinatura,
  cloud, novas garantias de filesystem ou interoperabilidade externa bidirecional.
- Sem promessa de throughput mínimo, RSS universalmente constante ou segurança de produção.
- Release benchmark é evidência do hardware/configuração medidos, não SLA.
- Questões abertas: nenhuma sobre objetivo; opção de cargo-fuzz pode refinar o plano.
  Semânticas e limites concretos serão definidos no CHECKPOINT 5, antes da implementação planejada.

## CHECKPOINT 5 — requisitos e invariantes — COMPLETE

### R1 — Interface e semântica do resumo

- Nova flag sem valor `--report-json`, após o nome do comando, no máximo uma vez.
  Parser continua rejeitando argumentos desconhecidos/duplicados e preserva caminhos OsString.
  Quando a flag estiver presente em comando reconhecido, reportar também erros de argumentos;
  duplicidade da própria flag é erro. Comando desconhecido mantém diagnóstico de uso atual.
- Sem flag: mesmos stdout/stderr, códigos de saída e chamadas de biblioteca atuais.
  Com flag: um objeto JSON compacto UTF-8 seguido de newline em stdout após retorno normal;
  erros e avisos humanos atuais continuam em stderr. Não há eventos de progresso.
- Campos obrigatórios v1: `report_version: 1`, `command`, `mode` (create/resume para
  M4; null nos demais), `status` (success/error), `elapsed_ms` (inteiro),
  `error_category` (null em sucesso), `input_bytes`, `output_bytes`,
  `dataset_records`, `dataset_parts`, `batch_records`, `record_length`,
  `publication` e `warnings`. Campos inaplicáveis/desconhecidos usam null;
  warnings é array de códigos, vazio quando nenhum aviso foi retornado.
- `elapsed_ms`: relógio monotônico desde entrada do dispatcher, incluindo parsing,
  leitura/compilação e operação. Termina antes da coleta auxiliar de metadados/serialização.
  Não representa soma de tempos internos. Injeção de relógio apenas em testes privados.
- `input_bytes`: tamanho observado por metadata de arquivo regular antes da operação,
  best-effort; não é contador de bytes lidos nem autenticados. `keygen` usa null.
- `output_bytes`: metadata do arquivo final após sucesso em M3/protect/unprotect;
  null em erro, M4 e keygen. Não somar namespace M4 nem ler payload para telemetria.
- `dataset_records`: apenas sucesso M3/M4, input_bytes/record_length se ambos disponíveis
  e divisíveis. `dataset_parts`: apenas sucesso M4, ceil(dataset_records/batch_records),
  com uma parte para vazio. São totais do dataset concluído; **não** progresso desta
  invocação, número de registros reprocessados ou partes novas. Resume usa essa mesma definição.
- `batch_records` e `record_length`: configuração validada/layout disponível ou null.
  Não inferir contagens de sucesso a partir de arquivo parcial ou de recibos em falha.
- `publication`: null em M3/M4 e erros; M5 sucesso usa objeto com status dos artefatos
  retornados (`output` ou `public_key`/`secret_key`), valores `published` ou
  `published_with_staging_residue`. Erro keygen pode ter publicado a chave pública;
  null significa desconhecido, nunca rollback. Warnings são apenas os efetivamente disponíveis.
- Categorias fechadas e de alto nível: `arguments`, `copybook`, `conversion`,
  `recovery`, `protection`, `input_io`. M5 mantém uma categoria única; não expor no JSON
  distinção de fingerprint/chave errada/tag ou Debug do erro.
- Sem paths, nomes/valores de campos, copybook, plaintext, chaves, seeds, nonces ou
  fingerprints no JSON. Diagnósticos humanos preexistentes não são convertidos em dumps.
- Coleta/serialização/escrita do resumo é best-effort: falha gera aviso fixo em stderr
  e não muda o resultado/exit code da operação, não desfaz commits e não causa retry.
  Usar Write falível, sem panic de println em broken pipe. Crash não garante relatório final.
- M2C não recebe caminho de relatório; redirecionamento pertence ao chamador. Scripts
  M6 só persistem relatórios fora do namespace M4, nunca sobre inputs/keys/outputs.

### R2 — Fuzzing ampliado, limites e evidência

- Campanha native usa proptest já fixado no lock, estratégias arbitrárias e mutações
  de entradas válidas. Documentar como testes generativos/mutacionais, **sem coverage-guided**.
- Famílias: (a) fonte fixed-format e AST pública; (b) layout público/decoder;
  (c) JSON e invariantes de manifest/receipt/completion M4; (d) codecs de chave/header
  M5; (e) stream M5; (f) Resume sobre namespace M4 mutado; (g) operações de arquivo M5.
  Famílias M5 somente com pqc. Testes privados ficam junto aos módulos privados.
- Smoke: 128 casos por família pura e 8 por família filesystem, seed `0x4D3643`.
  Extended: 10.000 por família pura e 256 por filesystem, cada um com seeds
  `0x4D3643`, `0x4D3644`, `0x4D3645`, `0x4D3646`. Stream M5 segue orçamento filesystem
  por custo, embora use buffers em memória. Não substituir o seed fixo M2 existente.
- Bounds de harness: fonte/bytes batch <=64 KiB; AST <=128 entries; documentos M4
  com casos até 4097 bytes (fronteira de 4096), mais tamanhos extremos declarados;
  estado M4 <=3 partes; arquivo/stream M5 <=2 chunks +17 bytes, incluindo vazio e
  fronteiras C-1/C/C+1/2C. Chaves malformadas incluem até 2417 bytes.
  Declarar limites gigantes sem alocar payload proporcional; layout validado enorme
  exercita validação, não cria um dataset gigantesco para decode.
- Oráculos: rejeição tipada ou resultado Arrow válido e schema exato; partições
  equivalentes; estado inválido M4 não limpa/avança; confirmado preservado; M5 inválido
  não publica destino. Mutação pode continuar válida: exigir erro apenas quando a
  propriedade da mutação garante invalidade. Panic não é erro tipado nem sucesso.
- Corpus pequeno versionado em `tests/fixtures/m6/`, com origem, propriedade e hash;
  sementes públicas de teste e fixtures existentes são permitidas, sem dados reais.
  Falhas novas: guardar entrada concreta minimizada + família/seed/case/config/commit
  em diretório exclusivo sob target/m6; replay da entrada não depende só do PRNG.
  Transformar todo bug corrigido em regressão determinística versionada.
- Runner extended configura testes por variáveis `M6_TEST_*` lidas exclusivamente
  sob cfg(test), mantém log por família/seed e watchdog externo de 30 min por família/seed.
  Timeout/abort/OOM ou contagem incompleta = inconclusive/fail para gate, nunca PASS;
  preservar seed, último caso identificado e logs para reprodução. Sem flags de falha na CLI.
- Sem alterar política runtime de tamanho do copybook/dados para acomodar o harness.
  Bug de contrato encontrado bloqueia a parte afetada: registrar inconsistencia e
  menor correção necessária; alteração material de contrato segue AGENTS.md, seção 9.

### R3 — Medições e workloads

- Um target `m6` em benches com `harness = false`; usar Instant e std::hint::black_box,
  dependências existentes. Executar medições somente com opt-in explícito
  `--profile smoke|full`; invocação por cargo test/Clippy não dispara campanha longa.
- Microbench de parse+compile separado do decoder. Decoder compila/valida uma vez
  fora da janela, recebe batch preparado e inclui criação/descarte do RecordBatch.
  Preparação, geração de dados, verificação e cleanup ficam fora da janela medida.
- End-to-end via CLI release pré-compilada: M3, M4 Create, M4 Resume de job concluído,
  protect e unprotect separados. Inclui startup/compilação do copybook, hashing,
  finalização/commit conforme cada comando; exclui compilação Rust e keygen de setup.
  Keygen e KEM/AEAD isolados não são benchmarks obrigatórios M6.
- Dataset escala: repetição streaming dos três registros anotados; identificar como
  workload artificial altamente repetitivo. Não comparar com carga real de mainframe.
  Microbench adicional de texto e numéricos usa vetores explícitos já suportados.
- Smoke: 3.000 registros, batch 256, 1 warmup +3 amostras.
  Full: 300.000 e 3.000.000 registros, batches 256/4096/65536 para M3/M4;
  1 warmup +7 amostras por cenário, sequenciais e destinos exclusivos.
  M5 usa payloads de 1 MiB e 64 MiB, mesmos warmups/amostras, entropia normal do SO.
  Microbench full: calibrar >=250 ms por amostra e registrar iterações; smoke >=25 ms.
- Persistir amostras cruas, mediana, min/max, bytes e records/s quando aplicáveis.
  Throughput M4 Resume concluído significa **validação** de dataset/prefixo, nunca
  conversão nova. Nenhum benchmark de resume parcial obrigatório; custo de recuperação
  interrompida fica demonstrado pela matriz funcional M4.
- Memória: runner Windows observa PeakWorkingSet64 do processo CLI isolado, via
  System.Diagnostics.Process, separadamente das medições internas; nomear métrica e
  registrar disponibilidade. Comparar fatores 10x de dados sob batch fixo, variar batch
  separadamente. Não chamar working set de heap Rust nem incluir geração/verificação.
- M3 pode crescer com número de row groups/footer; M4 limita dados por parte; M5
  buffers por chunk. Registrar tendências e relacionar ao código; crescimento
  inesperado de retenção do dataset exige investigação. Não inventar limite de RSS ou SLA.
- Cada amostra válida exige retorno de sucesso e verificação fora do tempo: Parquet
  lido em batches comparado à sequência de constantes esperadas; M4 segue recibos;
  M5 comparação/hash streaming byte a byte do original recuperado. Amostra inválida
  não entra na mediana e impede gate full até resolução.
- Manifesto de experimento: commit + dirty, hash Cargo.lock, rustc -Vv/cargo -V,
  feature/profile, SO/CPU/RAM/volume, parâmetros, hashes de dados/layout/binário,
  comandos, cache warm sem purge forçado e condições concorrentes conhecidas.
  Não executar campanhas/compilação em paralelo aos benchmarks medidos.

### R4 — Compatibilidade e escopo congelado

- Bibliotecas M0–M5 e formatos persistentes v1 ficam intactos; apenas CLI aditiva,
  tooling e testes. Não serializar tempo/métrica em manifest/receipt/header/AAD.
- Testes e scripts usam somente diretórios próprios exclusivos, validam caminhos
  absolutos antes de cleanup recursivo e não seguem reparse points para remoção.
- Não compartilhar logs/relatórios contendo chaves geradas. Demo usa dados públicos
  de teste; chave secreta temporária fica local e fora do pacote de evidências.
- Questões abertas: nenhuma semântica base pendente; eventual escolha coverage-guided
  exigirá adendo explícito. Próxima ação: sequenciar implementação no CHECKPOINT 6.

## CHECKPOINT 6 — plano de implementação — COMPLETE

### Sequência de entregas pequenas e revisáveis

1. **Contrato e infraestrutura de evidência.** Criar `docs/M6_EVIDENCE.md` com R1–R4,
   exemplos JSON, limites e comandos; manter este WIP como histórico. Preparar runner
   `scripts/m6.ps1` com `-Mode Verify|Fuzz|Bench|Demo`, `-Profile Smoke|Full` e
   `-OutputRoot` opcional (default target/m6). Criar subdiretório exclusivo por execução,
   manifesto de ambiente e status final. Verificar ferramentas/plataforma e nunca
   baixar/instalar dependências automaticamente. Artifact root externo a namespaces M4.
2. **Resumo CLI.** Acrescentar módulo privado do binário `src/cli_report.rs`, ligado
   por `mod` em src/main.rs, usando serde/serde_json existentes. Dispatcher coleta
   contexto disponível, chama as mesmas APIs uma vez, traduz resultado sem Debug,
   emite relatório falível e mantém os diagnósticos/exit code. Estender parsing dos
   comandos (inclusive keygen) e ajuda para a flag opcional, sem migração de dados.
   Adicionar `tests/cli_report.rs` e testes privados para relógio/escrita falível.
3. **Campanhas e replay.** Estratégias públicas M1/M2 em testes de integração;
   testes M4/M5 privados nos módulos já existentes, fatorando suporte somente sob
   cfg(test). Helpers de campanha em suporte de testes, sem exports na biblioteca.
   Ler família/seed/casos/profile/corpus/output das variáveis M6_TEST_* somente em testes.
   Runner executa família/seed sequencialmente e mantém contagens/logs/status; replay
   usa arquivo concreto de corpus antes de gerar entradas. Acrescentar regressões
   somente onde houver gap real além dos testes closeout já presentes.
4. **Bench e verificação de artefatos.** Registrar bench m6 no Cargo.toml, criar
   benches/m6.rs e helper de verificação em examples/m6_verify.rs. Helper reabre
   Parquet em batches e compara às constantes anotadas, valida sequência M4 por
   recibos e compara arquivos M5 por streaming; não chama decoder para fabricar esperado.
   Bench mede microcasos; runner pré-compila CLI/helper release e mede comandos em
   processos exclusivos, com metadados e amostras definidas em R3. Persistir JSONL
   bruto e summary JSON/Markdown. Não adicionar dependência de benchmark.
5. **Demo e automação.** Mode Demo cria uma árvore local nova com subpastas irmãs
   para entradas, conversão M3, job M4, chaves, proteção e relatórios. Executa a sequência
   abaixo e gera evidência legível com parâmetros/resultado. Adicionar workflow
   Windows de regressão + smoke invocando o mesmo runner; jobs em PR/push e execução
   manual, sem campanhas agendadas ou publicação de arquivos/chaves/datasets.
6. **Experimentos e fechamento.** Executar gates, campanha Full e benchmarks Full
   localmente, sequencialmente; investigar falhas e registrar limitação quando a medição
   não sustentar uma conclusão. Versionar relatório compacto em docs e pequenos
   resultados numéricos/manifesto sanitizado; datasets, binários, chaves e logs volumosos
   permanecem em target/m6. Atualizar README/arquitetura apenas para comportamento
   efetivamente entregue, sem modificar contratos M4/M5. Não iniciar M7.

### Sequência exata da demo

1. Validar hash da fixture de 105 bytes e registrar toolchain/feature/plataforma.
2. Converter M3 com batch 2; verificar schema, 3 linhas e valores independentes,
   2 row groups. Salvar resumo JSON em reports, fora dos destinos de dados.
3. Converter M4 Create com batch 2; verificar partes 2+1 e recibos, snapshots dos
   confirmados. Resume concluído via CLI deve preservar hashes de partes/recibos.
4. Executar teste privado M4 `process_interruption_matrix_converges_and_preserves_every_committed_part`
   por cargo test com filtro exato. Capturar comando, contagem e resultado: é a prova
   explícita de interrupção/retomada, não encenar crash com arquivo apagado ou sleep.
5. Com pqc, keygen fora do job M4; protect do Parquet M3 para diretório separado;
   unprotect para outro nome novo; comparar arquivos por streaming.
6. Adulterar cópia do envelope de teste, exigir erro e ausência de destino final.
   Confirmar integridade do original e de um destino preexistente em teste no-clobber.
7. Gerar índice de evidências e limitações, sem exportar chave secreta; limpeza
   somente de artefatos próprios, com caminhos validados. Em falha preservar evidências
   locais necessárias e indicar resíduos, sem varrer diretórios do usuário.

### Automação e interfaces auxiliares

- Runner Verify executa gates exatos do checkpoint 7, confere códigos de saída
  ($LASTEXITCODE após comandos nativos), continua coletando gates independentes e
  retorna falha global se qualquer gate falhar. Sem pipe que mascare o exit code.
- Helpers e scripts são tooling de desenvolvimento; não entram na API pública do pacote.
- Bench mode invoca `cargo bench --bench m6 -- --profile smoke|full` para microcasos
  e CLI release para arquivo; com feature pqc explicitamente nas medições M5.
- Flags adicionais do helper verifier ficam restritas a `--kind m3|m4|roundtrip`,
  paths de leitura, fixture/layout e número esperado de repetições. Não escreve nos
  artefatos verificados. Contagem/ordem/schema/valores são invariantes obrigatórios.
- CI Windows usa rustc 1.95.0 (baseline observado) e Cargo.lock via --locked nos
  passos adicionais; isso não redefine o MSRV público nem exige trocar o default local.
  Fixar revisões das actions na implementação e documentar a versão efetivamente usada.
  Sem baseline competitivo de performance em runner compartilhado.
- Não adicionar `[workspace]` nem crate auxiliar para M6 base. Mudanças de produção
  esperadas ficam na CLI; módulos de conversão só recebem testes privados se necessário.
- Questões abertas: nenhuma dependência entre entregas base indefinida. Planejamento
  não executa as entregas 1–6; próxima ação nesta sessão é registrar testes/gates e consolidar.

### Atualização de baseline após CHECKPOINT 6

- `cargo test --all-targets --all-features`: PASS, exit 0; **148 testes** (69 unitários,
  79 integração), zero falhas/ignored declarados. Contagem difere da revisão M5
  histórica porque regressões closeout foram acrescentadas depois.
- `cargo test --doc`: PASS, exit 0; 1 doctest.
- Ainda pendente nesta sessão: doctest com todas as features e inspeção final do WIP.

## CHECKPOINT 7 — testes, gates e acceptance criteria — COMPLETE

### Matriz de testes M6 planejada

| Grupo | Cenários obrigatórios | Evidência de aceite |
|---|---|---|
| CLI compatível | Cinco comandos com/sem flag, pqc ausente, paths com espaços, argumentos faltantes/desconhecidos/duplicados | Sem flag preserva comportamento; flag válida emite um único JSON parseável |
| Resumo exato | Vazio, fixture 3 registros B=2, M4 Create e Resume concluído, falha antes/depois de batch | Contagens e nulls conforme R1; nunca informar trabalho novo no resume nem concluir com base em parcial |
| M5 no resumo | Sucesso, aviso ACL, resíduo pós-commit, chave errada/tag, falha de keygen após primeiro commit | Códigos permitidos e publicação conhecida; erro não implica rollback; nenhum segredo/path no JSON |
| Telemetria falível | Metadata indisponível, writer que falha, broken pipe, relógio controlado | Operação/exit code preservados; aviso fixo; sem panic ou retry |
| Campanha/replay | Smoke/Extended, corpus válido/inválido, redução de falha artificial do harness | Mesma entrada concreta reexecuta sem RNG; contagens/configuracao registradas; runner propaga falha |
| Parser/decoder | ASCII/Unicode inválido, cláusulas não suportadas, AST/layout manipulados, limites/sinais/escalas/FILLER | Resultado conforme contrato ou erro tipado, sem panic; arrays válidos e esquema/valores exatos |
| M4 adversarial | JSON truncado/oversize/duplicado, identidade, gaps, órfão, corrupção de confirmado | Erros preservam namespace e confirmados; matriz de falha anterior continua passando |
| M5 adversarial | Header/chaves, comprimentos, truncamento, ordem/duplicação, tag tardia | Sem publicação inválida, sem alocação proporcional a tamanho declarado; corpus externo mantido |
| Bench e memória | Perfis, amostra inválida, warmup, unidades, processo novo, ausência de métrica | Sem campanha longa em cargo test; dados crus consistentes; indisponível não vira zero; verificação fora do timing |
| Demo/runner | Execução limpa, segunda execução, caminho conflitante, falha de comando/preflight | Diretorios exclusivos, sem overwrite/cleanup externo; status agregado correto e índice de evidências |

### Gates para implementação futura

- **G0 — Contrato:** confirmar plano base, R1–R4 e exclusões; documento M6 diferencia
  suporte atual, testes planejados e evidência executada. Nenhuma migração M4/M5.
- **G1 — Compatibilidade:** API pública M0–M5 e formatos/identidades preservados;
  sem dependência runtime nova; helper/hook determinístico acessível somente em testes;
  pqc permanece opcional e não entra no build default.
- **G2 — Regressão:** todos os comandos abaixo exit 0, capturados individualmente:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo test --doc
cargo test --doc --all-features
```

- **G3 — Correção M6:** matriz acima aprovada e corpus smoke executado em default e
  pqc; extended completo em release/all-features com seeds/casos de R2. Reexecutar
  regressões numéricas/adversariais e `cargo test --release --test protection --all-features`.
  Crash, panic, timeout, falha de oráculo ou caso não reproduzível permanece aberto;
  não declarar campanha concluída só por encerrar sem crash aparente.
- **G4 — Evidência:** benchmarks Full e observação de memória completos nos workloads
  R3, manifesto e amostras disponíveis, verificação funcional de todas as amostras;
  demo completa no alvo Windows/MSVC + NTFS local. Sem threshold arbitrário de velocidade.
  Duração/throughput/working set são observações, não prova geral de segurança ou memória.
- **G5 — Fechamento:** relatório rastreia requisito -> teste/comando -> resultado;
  nenhum blocker de correção/contrato aberto, documentação honesta e limitações visíveis.
  Skips ambientais são listados com nome/motivo; não contam como cobertura executada.
  Para alegar cobertura dinâmica de reparse/symlink, exigir execução em ambiente apto.
  Ambiente indisponível é BLOCKED para o cenário, não PASS. Limitações fora do escopo
  M6 podem permanecer explicitamente aceitas, sem ampliar garantias.

### Critério observável de conclusão

Um leitor em Windows/MSVC/NTFS, com toolchain/dependências documentadas, consegue
executar Verify, Demo, Fuzz Full e Bench Full, obter resultados funcionais exatos,
reproduzir um corpus de falha por entrada concreta e interpretar os custos de
conversão, validação M4 e proteção M5 a partir dos dados crus. JSON não altera os
resultados nem a superfície de informação sensível. README pode então marcar M6
implementado com links de evidência; até lá permanece planejado.

### Resultado desta sessão e pendências

- `cargo test --doc --all-features`: PASS, exit 0; 1 doctest. Os seis comandos G2
  já passaram **no código M0–M5 atual**, antes de qualquer implementação M6.
- Nenhum teste de funcionalidade M6, campanha ampliada, bench, demo nova ou workflow
  foi implementado/executado nesta sessão. Esses gates futuros continuam NOT RUN.
- Nenhum desvio arquitetural proposto; única interface pública nova é flag/JSON CLI.
- Questões abertas para consolidar: confirmar limitação de symlink do ambiente e
  revisão final de consistência. Opção cargo-fuzz permanece default nativo se não houver resposta.

### Revisão de evidência após CHECKPOINT 7

- Executado `cargo test --all-features reparse_point_in_write_path_fails_closed -- --nocapture`:
  exit 0, mas cenário **SKIPPED por ambiente**: criação de symlink falhou com erro
  Windows 1314 (privilégio ausente). Não comprova rejeição dinâmica de reparse point.
  Manter lacuna no baseline apesar da linha `test ... ok` do harness.
- Consulta sobre cargo-fuzz não recebeu resposta até esta consolidação. Decisão
  assumida: base nativa, sem coverage-guided; não é uma preferência atribuída ao usuário.
  A única escolha explicitamente respondida foi a profundidade do resumo JSON.
- Correção de precisão: teste/report de chaves M5 conserva `warnings` como códigos
  retornados (`permission_restriction_failed`); resíduo aparece em `publication`.
  Não inspecionar árvore de chaves após erro para inventar um outcome que a API não retornou.
- Perfis do runner: Verify inclui matriz default/pqc; Fuzz Smoke/Full exercita famílias
  base e M5 com builds correspondentes; Demo requer pqc e alvo M5 suportado; Bench
  usa microcasos default e CLI de arquivo compilada com pqc, registrando essa diferença.
  Profile Full não reduz os gates Verify e não inicia trabalho remoto.
- Resultados volumosos de benchmarks podem ser removidos após verificação de cada
  amostra, apenas dentro da pasta exclusiva validada; preservar logs, hashes e amostras
  numéricas. Isso limita uso de disco sem alterar o que é medido.

## CHECKPOINT FINAL — plano consolidado — COMPLETE

### Objetivo e escopo final recomendado

**M6 — evidência técnica e demo local**, conforme roadmap existente: tornar M0–M5
demonstráveis e reproduzíveis para avaliação de portfólio, preservando seus contratos.
Planejamento concluído; implementação M6 **não iniciada**.

| Entrega | Decisão final | Detalhamento para implementação |
|---|---|---|
| Observabilidade | `--report-json` por comando, opt-in, stdout, metadados best-effort | CHECKPOINT 5 / R1; escolha explícita do usuário |
| Fuzzing ampliado | Proptest + mutações + corpus concreto/replay; Smoke/Full com orçamento explícito | CHECKPOINT 5 / R2; default assumido, sem coverage-guided |
| Benchmarks | Microcasos parser/decoder e CLI release M3/M4/M5, amostras verificadas | CHECKPOINT 5 / R3; sem meta de otimização |
| Memória | Peak working set do processo, escala de dados/batch e limitações M3 | CHECKPOINT 5 / R3; sem promessa de RSS constante |
| Demo | Fixture conhecida -> Parquet/partes -> teste determinístico de recovery -> round-trip/tamper | CHECKPOINT 6; nenhuma flag de crash em produção |
| Reprodutibilidade | Runner PowerShell local e CI Windows smoke, manifesto e resultados crus | CHECKPOINT 6; sem infraestrutura operacional |

Implementar na ordem: contrato/runner básico -> resumo CLI e testes -> campanhas/replay
-> benchmark/verifier -> demo/CI -> execuções Full e documentação de resultados.
Dependências e interfaces estão fechadas em R1–R4; critérios de aceitação no CHECKPOINT 7.

### Invariantes e exclusões finais

- Não mudar assinaturas públicas da biblioteca, subset COBOL, representações numéricas,
  sincronismo, identidade/formatos M4, suíte/formato/AAD/publicação M5 ou feature opcional.
- JSON não entra em namespace/commit; sem bytes sensíveis, nomes de campos, paths ou
  distinções adicionais de falha criptográfica. Falha de telemetria não reverte resultado.
- Totais de dataset M4 são identificados como totais, nunca como conversão realizada
  naquela retomada. Métricas ausentes usam null, não zero inventado.
- Oráculos independentes existentes permanecem; geração sintética é evidência de escala
  ou propriedade, não única prova de correção. Medições exigem verificação fora do timing.
- Não incluir cloud/ML-DSA/novos formatos, integração M4+M5, recovery M5 pós-crash,
  transação keypair, UI, async, observabilidade remota ou otimizações sem evidência.
- Correções encontradas pelas campanhas são triadas sob AGENTS.md; incompatibilidade
  material entre código e contrato bloqueia a parte afetada e não autoriza redesenho.

### Baseline real desta sessão

| Comando executado | Resultado |
|---|---|
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS, exit 0 |
| `cargo test --all-targets` | PASS, exit 0; 117 testes |
| `cargo test --all-targets --all-features` | PASS, exit 0; 148 testes |
| `cargo test --doc` | PASS, exit 0; 1 doctest |
| `cargo test --doc --all-features` | PASS, exit 0; 1 doctest |
| Teste direcionado de reparse point com `--nocapture` | Comando exit 0; cenário SKIPPED (privilégio 1314) |

Ambiente observado: Rust/Cargo 1.95.0, x86_64-pc-windows-msvc, drive C fixed/NTFS.
Get-Volume foi negado; DriveInfo confirmou tipo/formato sem elevação. Cargo-fuzz ausente
e nightly não listado; nada instalado. Revisão histórica M5 não substitui estes resultados.
Baseline aprovado não significa G0–G5 de M6 executados: **gates da implementação M6 NOT RUN**.

### Questões abertas, limitações e retomada

- **Nenhuma questão de escopo base bloqueia o planejamento.** Usuário confirmou resumo
  JSON; abordagem nativa de fuzzing é default explicitamente assumido após consulta
  opcional sem resposta. Se preferir cargo-fuzz, revisar R2, tooling e gates em adendo.
- **Lacuna ambiental aberta:** evidência dinâmica de symlink/reparse exige ambiente
  com privilégio; não atribuir PASS ao cenário com base no harness que retornou cedo.
- Limitações M3/M4/M5 permanecem: footer M3, validação integral M4, garantias apenas
  no modelo de falha documentado e possível staging plaintext M5 após crash.
- Não foi feita auditoria exaustiva de correção/segurança; inspeção dirigida não
  encontrou conflito concreto entre implementação e contratos que exigisse redesenho.
- Único arquivo de projeto criado/alterado nesta sessão: `docs/M6_PLAN_WIP.md`.
  Nenhum teste/código de produção/dependência foi alterado. Artefatos Cargo são locais.
- Próximo agente: conferir estado Git, ler este resumo e CHECKPOINTS 5–7. Se a tarefa
  continuar sendo planejamento, refinar este arquivo; implementar somente quando
  houver solicitação de implementação. Ao implementar, começar pelo item 1 do
  CHECKPOINT 6 e manter a distinção entre plano, execução e evidência. Não iniciar M7.

## ADENDO — avaliação das sugestões pós-plano — COMPLETE

Este adendo refina o CHECKPOINT FINAL e prevalece onde reduzir ou esclarecer o
escopo anterior. As cinco sugestões avaliadas são eficazes e foram aceitas.

### 1. Keygen parcial: `publication = null` — REQUIRED, ACEITA

- A API retorna `Result<KeyGenerationOutcome, ProtectionError>`. A chave pública é
  publicada antes da secreta; se `secret_stage.finish()` falhar, o chamador recebe
  somente `Err`, sem `KeyGenerationOutcome`, embora `public.key` possa já existir.
- O relatório JSON deve derivar publicação apenas do outcome retornado. Nesse erro,
  `publication` é obrigatoriamente null e `warnings` contém somente avisos realmente
  retornados ao reporter (normalmente array vazio sem outcome).
- É proibido examinar diretório, nomes, links ou arquivos após o erro para reconstruir
  um estado de publicação. Essa inferência teria corrida, confundiria resíduos/arquivos
  externos com resultado da chamada e criaria uma garantia ausente da API M5.
- Testar a tradução de `Err` após o ponto lógico de primeiro commit: status error,
  categoria protection, publication null e nenhum rollback alegado. O teste pode
  comprovar separadamente que o arquivo público persiste, sem usar isso no JSON.

### 2. Verificador M4 externo sem exportar DTOs — REQUIRED, ACEITA

- `Manifest`, `Receipt`, `Completion` e `manifest::read_json` são `pub(crate)` por
  desenho. O example é um consumidor separado da biblioteca e não deve tornar esses
  tipos públicos só para facilitar tooling M6.
- `examples/m6_verify.rs` deve consumir o formato persistido como um terceiro faria:
  DTOs privados locais com `serde(deny_unknown_fields)` ou `serde_json::Value`, limite
  de 4096 bytes antes do parse e validações explícitas do contrato M4 v1.
- Verificar ao menos: nomes canônicos, versão/formato/profile, campos e tipos exatos,
  hashes hex, `job_id`, sequência contígua de receipts, índices/ranges, tamanhos e
  SHA-256 das partes, `complete.json`, part_count e total_records. Rejeitar artefatos
  desconhecidos, lacunas, trailing JSON e campos ausentes/duplicados/desconhecidos.
- Essa duplicação pequena e deliberada é um oracle externo do wire format; helpers
  internos podem compartilhar utilidades entre testes internos, mas não com o example.
  Nenhuma nova API pública ou comando runtime de validação é criado.

### 3. Semântica dos Parquets M4 — RECOMMENDED, ACEITA COMO OBRIGATÓRIA

- Receipt prova identidade dos bytes declarados, não que esses bytes contenham os
  valores esperados. Mesmo schema/row count/footer não provam conteúdo correto.
- Após validar receipt e SHA-256, o verifier reabre cada parte, exige schema completo,
  compressão e row groups esperados, lê em batches e compara ordem e valores com as
  constantes independentes da fixture anotada. A concatenação deve resultar exatamente
  nos três registros esperados, inclusive Decimal128, CP037 e ausência de FILLER.
- Para datasets de benchmark gerados por repetição, comparar cada linha pela posição
  modulo 3 às mesmas constantes; não chamar o decoder M2 para gerar o esperado.
- Uma diferença semântica invalida a amostra e o gate, mesmo quando receipts/hashes
  conferem. Essa regra torna explícito e reforça R3 e o item 4 do CHECKPOINT 6.

### 4. Cenário M4 `3M / batch=256` — RECOMMENDED, ACEITA COMO STRESS OPCIONAL

- O cenário cria `ceil(3.000.000/256) = 11.719` partes e o mesmo número de receipts:
  23.438 artefatos principais por amostra. Com 1 warmup + 7 amostras seriam até
  187.504 criações de parte/receipt ao longo do cenário, além de manifests, validação,
  hashing e cleanup. Ele mediria fortemente overhead de namespace/NTFS e alongaria o gate.
- Matriz Full revisada:
  - M3: 300 mil e 3 milhões de registros com batches 256, 4096 e 65536.
  - M4: 300 mil com batches 256, 4096 e 65536; 3 milhões apenas com batch 65536.
  - M4 Resume concluído: medir para os mesmos cenários M4, sempre identificado como
    validação integral e sem contar records/parts como trabalho novo da invocação.
- `3M / 256` passa para `-Profile Stress`, explicitamente opt-in, fora dos gates G2–G5
  e sem expectativa de execução em CI. Seus resultados não são necessários para
  declarar M6 completo. O runner estima 11.719 partes/23.438 artefatos antes de iniciar,
  registra espaço livre e exige confirmação explícita de perfil via argumento.
- Essa redução mantém dois experimentos úteis: efeito do batch em 300 mil registros e
  efeito de escala em batch 65536, sem fazer explosão de arquivos parte do aceite.

### 5. Fuzzing como cobertura complementar — RECOMMENDED, ACEITA

- Antes de criar uma campanha, produzir matriz `invariante -> teste atual -> gap`.
  Cada novo target deve nomear o gap que cobre; caso contrário, ampliar seeds/cases
  de propriedade existente ou não adicionar o target.
- Reutilizar como base: propriedades M2 já existentes; testes arbitrários do parser;
  rejeições typed-JSON M4; matrizes de fault injection/namespace M4; corpus, limites,
  tamper e sequência M5. Não duplicar sistematicamente as sete famílias listadas na
  versão inicial de R2.
- Escopo inicial refinado para gaps prováveis e confirmados por inspeção:
  1. combinações bounded de mutações de documentos + namespace M4, onde testes atuais
     cobrem casos individuais mas não sequências combinadas;
  2. envelopes/chaves M5 mutados de forma estruturada em regiões e limites, ampliando
     o corpus fixo sem refazer round-trips e vetores já cobertos;
  3. campanhas maiores/múltiplas seeds das propriedades M1/M2 existentes, sem criar
     uma segunda implementação equivalente.
- Smoke/Full, bounds, replay concreto e tratamento de timeout de R2 continuam válidos,
  aplicados somente aos targets aprovados pela matriz de gaps. A quantidade de famílias
  deixa de ser critério de completude; evidência nova e reproduzível passa a ser o critério.
- Se a matriz mostrar que um gap já está coberto, documentar a evidência e encerrar o
  item. Fuzz M6 complementa testes determinísticos; qualquer bug encontrado vira uma
  regressão mínima permanente, e a campanha continua sendo descrita como generativa/
  mutacional, sem alegação de coverage-guided.

### Impacto consolidado

- Nenhuma sugestão exige mudança da arquitetura congelada, dependência runtime ou
  export de API. As duas regras REQUIRED fecham ambiguidades de telemetria e tooling.
- A verificação semântica eleva a força da evidência M4. A matriz de benchmark revisada
  reduz custo sem perder as comparações centrais. A triagem de fuzz evita redundância
  e está alinhada às prioridades de correção, testabilidade e simplicidade do AGENTS.md.
- Questões abertas após este adendo: nenhuma. A abordagem coverage-guided continua
  fora do baseline; `3M / 256` M4 fica disponível apenas no perfil Stress.

## Execução M6 — COMPLETE

- Usuário autorizou seguir o plano de implementação. Checkpoints anteriores são histórico.
- Ordem: contrato/tooling básico, JSON CLI, campanhas complementares, verifier/bench,
  demo/CI, gates e execuções Full. Não iniciar M7.
- Baseline de entrada: somente este WIP não versionado; código M0–M5 sem alterações.
- Próximo passo: contrato M6 e instrumentação apenas da CLI; biblioteca/formatos preservados.

### Implementação — checkpoint A — COMPLETE

- Criados contrato M6, resumo JSON privado da CLI e testes de conversão/M4/M5.
- Verificador externo com DTOs privados locais e comparação semântica contra fixture.
  Teste com receipt/hash recalculado e valor alterado passou (rejeição semântica).
- Campanhas M4/M5 adicionadas sob cfg(test), corpus seed e persistência/replay de
  bytes concretos. Propriedades M2 e fontes arbitrárias M1 recebem orçamento/seed de teste.
- Bench micro inicial com opt-in explícito; Clippy em execução. Gates finais ainda pendentes.
- Próximo: runner PowerShell, CI/demo e execução das campanhas/benchmarks Full.

### Implementação — checkpoint B — COMPLETE

- Runner, perfis Smoke/Full/Stress e workflow Windows criados. CI ainda não executado remotamente.
- Smoke Fuzz passou após corrigir tratamento de variável replay vazia no harness.
- Primeira demo falhou porque um build default concorrente substituiu o executável
  release pqc compartilhado. Runner agora copia CLI/verifier para bin/ próprio da
  execução, registra hashes e usa essas cópias; nova demo em andamento.
- Bench customizado ajustado para a flag --bench acrescentada pelo Cargo.
- Refinamento honesto da métrica: maior PeakWorkingSet64 observado em consultas
  de 20ms; limite inferior do pico, não pico exato. Null se não observado. Documentado.
- Próximo: verificar demo, campanhas Full, Smoke/Full Bench e gates finais sequenciais.

### Implementação — checkpoint C — COMPLETE

- Demo PASS: target/m6/20260906-010703-6c4ddcf4b33a4ebbb649a65bfb26ed67.
- Fuzz Full PASS: target/m6/20260906-011043-8956b319101d4af89dea3b8de42d058b:
  4 seeds, 10.000 fontes/seed, 4 propriedades ×10.000 casos/seed, 256 mutações M4 e
  256 M5/seed, corpus inicial e replay concreto M4/M5. Sem falhas ou skips na campanha.
- Corpus concreto de exemplo preservado mesmo em sucesso; runner agora verifica que
  filtros executaram testes e que campanhas/replays emitiram marcador de conclusão.
- Fonte/hash por arquivo e parâmetros de campanha adicionados ao manifesto do runner.
- Próximo: Bench Smoke, Full e gates finais; não há resultado de performance declarado ainda.

### Implementação — checkpoint D — COMPLETE

- Bench Full PASS: 20260906-011438-47b68d92cb4a40078fc19330db12500b,
  144 amostras verificadas (18 warmups); dados e proveniência em docs/evidence/.
- M3 com batch256 apresentou crescimento de memória observada associado à escala;
  não sustenta alegação de memória global constante (footer documentado). M4 com
  batch65536 apresentou valores próximos em 300k/3M. Sem atribuir causalidade ou SLA.
- Gates finais em execução; fmt, Clippy e testes default passaram até este registro.
- Microbench ampliado para texto/números/misto, com oráculos independentes; corrigida
  contagem de bytes por iteração de compile para o tamanho do copybook.
- Pendente: finalizar gates, medir micro Full e repetir demo com no-clobber explícito.

### Implementação — CHECKPOINT FINAL — COMPLETE

- Gates Verify PASS: 20260906-012521-0d3268cf8812447182bcdf6f8dd5e4fd.
  fmt, Clippy, all-targets default/pqc, doctests default/pqc e proteção release.
- Micro Full final PASS: seis cenários (compile/decode × misto/texto/numérico),
  48 amostras incluindo warmups. Bytes de compile corrigidos; fmt/Clippy repetidos PASS.
- Demo final PASS: 20260906-013336-2497a890b3af43cfa7eabc138e5c543b,
  incluindo no-clobber e hashes preservados. Bench/Fuzz Full já registrados acima.
- Evidência consolidada em docs/M6_RESULTS.md e docs/evidence/; contrato/tooling
  em docs/M6_EVIDENCE.md; README/arquitetura refletem a fronteira CLI implementada.
- APIs públicas, formatos e dependências runtime preservados. Nenhum M7 iniciado.
- Questões abertas de ambiente: teste reparse pulado por erro1314; CI configurada
  mas não executada remotamente; Stress opcional não executado. Não contar skip como
  cobertura dinâmica. Não há blocker de implementação identificado.
- Proveniência: medições de arquivos precedem refinamento de argumento inválido da
  CLI e expansão do microbench; hashes preservados distinguem essa revisão medida.
