# M2C Quantum-Safe Data Pipeline

Pipeline experimental, escrito principalmente em Rust, para estudar a conversão de dados binários legados de mainframe em dados colunares tipados:

```text
arquivo binário fixed-record + COBOL copybook
    -> layout compilado
    -> decoding tipado
    -> Arrow / Parquet
    -> proteção quantum-safe opcional
    -> sink local ou cloud
```

## Status

Este é um projeto educacional e de portfólio, mantido por um estudante de Engenharia da Computação. A arquitetura v0.1 está congelada. **M0 a M6 estão implementados:** fundação, compilador de copybook, codecs e Arrow, conversão local para Parquet, recuperação em partes, proteção experimental de artefatos e evidência técnica local.

O projeto oferece conversão local síncrona de um arquivo fixed-record para Parquet, em batches limitados, pela biblioteca e pela CLI. O M4 acrescenta saída em partes determinísticas com manifest e retomada após interrupção do processo, preservando a conversão de saída única M3. O M5 acrescenta, sob a feature opcional `pqc`, proteção autônoma de arquivos com ML-KEM-768, HKDF-SHA-256 e AES-256-GCM/STREAM-BE32. M6 acrescenta resumo JSON por comando, campanhas complementares, verificação externa e benchmarks reproduzíveis. Cloud e infraestrutura de observabilidade permanecem futuras. O software não deve ser usado para dados sensíveis ou cargas de produção.

## Evidência local M6

Adicione `--report-json` aos comandos existentes para obter resultado, duração e
volumes observáveis em stdout; diagnósticos e códigos de saída são preservados.
Campos desconhecidos são `null`; o relatório não contém caminhos ou chaves.
O runner PowerShell 7 oferece `Verify`, `Demo`, `Fuzz` e `Bench`:

```powershell
./scripts/m6.ps1 -Mode Verify
./scripts/m6.ps1 -Mode Demo
./scripts/m6.ps1 -Mode Fuzz -Profile Full
./scripts/m6.ps1 -Mode Bench -Profile Full
```

Consulte o [contrato e reprodução](docs/M6_EVIDENCE.md) e os
[resultados locais](docs/M6_RESULTS.md). Gates locais passaram; reparse/symlink
teve skip ambiental Windows 1314. O workflow Windows passou remotamente no commit
`8d44218605a59a190590772fa52232c5859c9bc8` com Verify/Fuzz Smoke/Demo/Bench Smoke;
esse run precede a remediação final, validada localmente. Full permanece local.
As medições não estabelecem SLA nem memória global constante.

## Desempenho — benchmarks locais

Medições empíricas em máquina local (AMD Ryzen 5 3400G, 16 GB DDR4, Windows 10/NTFS, Rust 1.95 MSVC release, 1 warmup + 5 execuções medidas):

- **M3 (conversão direta para Parquet)**: 3.000.000 registros (100,14 MiB) em batch 65.536 com mediana de **2.016,67 ms** (~1,49M reg/s, 49,65 MiB/s de entrada, pico de memória de trabalho observado de 15,24 MiB).
- **M4 (conversão multipart recuperável)**: 3.000.000 registros em batch 65.536 (46 partes) com mediana de **4.735,55 ms** (~633,5k reg/s, 21,15 MiB/s, pico observado de 15,33 MiB).
- **M5 (proteção quantum-safe)**: payload de 64 MiB com protect em **1.642,92 ms** (38,96 MiB/s, 5,28 MiB WS) e unprotect em **1.687,85 ms** (37,92 MiB/s, 5,29 MiB WS), verificado por igualdade estrita de SHA-256.
- **Microbenchmarks (in-memory)**: decode de batch misto a **4,99M reg/s** (153.905 ns/it para 768 registros) e texto puro a **38,21M reg/s** (6.700 ns/it para 256 registros).

Consulte a metodologia, limites e reprodutibilidade completa em [BENCHMARKS.md](docs/BENCHMARKS.md). Para executar o harness reproduzível:
```powershell
./scripts/benchmark.ps1 -Profile Full
```

## Compatibilidade externa — validação Spark / Cobrix

Validação diferencial independente de exatidão semântica (não benchmark de desempenho):
- Processamento de dataset realista sintetizado com GnuCOBOL (`input.ebcdic`, 100 registros de 24 bytes com texto CP037 e decimais compactados COMP-3).
- Decodificação independente pelo conector oficial **AbsaOSS Cobrix 2.9.4** sobre **Apache Spark 4.0.1** (Java 17, Ubuntu 24.04 LTS via WSL2).
- Comparação semântica campo a campo via [`scripts/compare_cobrix.py`](scripts/compare_cobrix.py): **100/100 registros idênticos** após normalização de tipos (`int32` vs `decimal128(9,0)`).
- Relatório completo e declaração técnica formal em [EXTERNAL_COMPATIBILITY.md](docs/EXTERNAL_COMPATIBILITY.md).

## Base de conversão

O M1 transforma um copybook do subconjunto documentado em uma representação compilada. O M2 usa esse layout para decodificar bytes sem reinterpretar COBOL no hot path:

```text
sample.cpy
    -> normalização fixed-format
    -> parser
    -> AST mínima
    -> CompiledCopybook
         - record length
         - field offsets e byte lengths
         - physical encodings e signedness
         - precision e scale
         - logical Arrow types
         - Arrow Schema
```

O subconjunto aceito é intencionalmente pequeno. Sintaxe fora dele deve produzir um diagnóstico explícito com localização, nunca ser ignorada silenciosamente. Consulte [COPYBOOK_SUBSET.md](docs/COPYBOOK_SUBSET.md) para o contrato completo.

## Arquitetura v0.1

O repositório usa um único pacote Rust com biblioteca e CLI. O fluxo é:

1. interpretar e compilar o copybook uma única vez;
2. em M3, ler um arquivo binário de registros de tamanho fixo em batches limitados;
3. usar os codecs M2 para decodificar cada batch para Arrow;
4. em M3, escrever incrementalmente row groups em um único arquivo Parquet local;
5. em M4, oferecer partes locais, recibos imutáveis de commit e retomada explícita;
6. em M5, proteger opcionalmente um arquivo já produzido usando AEAD + ML-KEM;
7. somente depois da demonstração local, considerar um sink de object storage.

A descrição dos limites e invariantes está em [ARCHITECTURE.md](docs/ARCHITECTURE.md). A análise que motivou a reconstrução permanece em [ANALISE_DO_PROJETO.md](docs/ANALISE_DO_PROJETO.md).

## Mapeamento lógico congelado

| Campo COBOL | Tipo lógico Arrow |
|---|---|
| `PIC X...` | `Utf8` |
| DISPLAY inteiro | `Int64` |
| DISPLAY com escala implícita `V` | `Decimal128` |
| COMP/BINARY sem escala | `Int64` |
| COMP/BINARY com escala implícita `V` | `Decimal128` |
| COMP-3/PACKED-DECIMAL | `Decimal128` |

`FILLER` ocupa bytes e participa dos offsets e do tamanho do registro, mas não é exposto no Arrow Schema.

## Verificação de desenvolvimento

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo test --doc
cargo test --doc --all-features
```

Os testes do M1 validam AST, layout e rejeição de sintaxe não suportada. O M2 acrescenta a tabela pública CP037 completa, uma fixture binária anotada comparada a um RecordBatch esperado, testes adversariais e propriedades com seed fixa. Consulte a [origem das fixtures](tests/fixtures/README.md).

A API de entrada do milestone é `parse_and_compile_copybook(&str)`. Para inspecionar separadamente as duas etapas, use `parse_copybook(&str)` seguido de `compile_copybook(&CopybookAst)`.

## Decoding M2

```rust
use m2c_pipeline::{parse_and_compile_copybook, RecordDecoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layout = parse_and_compile_copybook(
        "       01 ROOT.\n       05 COUNT-FIELD PIC 9(2).\n"
    )?;
    let decoder = RecordDecoder::try_new(&layout)?;
    let batch = decoder.decode_batch(&[0xF1, 0xF2, 0xF0, 0xF3])?;
    assert_eq!(batch.num_rows(), 2); // Int64: 12 e 3
    Ok(())
}
```

O decoder valida o layout uma vez e pode ser reutilizado. O chamador fornece batches
limitados com registros inteiros. Texto mantém espaços e controles CP037; erros
numéricos retornam diagnósticos tipados, sem batch parcial. As políticas de sinais,
precisão, capacidade e posições estão no [contrato de decoding](docs/DECODING.md).

## Conversão local M3

```bash
cargo run -- convert --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output sample.parquet --batch-records 2
```

Os quatro argumentos são obrigatórios. `--batch-records` deve ser um inteiro
positivo: limita quantos registros são lidos e decodificados por vez. O exemplo
produz três linhas em dois row groups (2 + 1), sem compressão adicional. A saída
deve ser um caminho novo, com diretório pai existente; arquivos existentes nunca
são sobrescritos. Erros são escritos em stderr e retornam status não zero.

A biblioteca expõe
`convert_file(&CompiledCopybook, &Path, &Path, usize) -> Result<(), ConversionError>`.
Os caminhos são entrada e saída, nessa ordem; o último argumento limita os
registros por batch. O copybook é compilado uma vez e um único `RecordDecoder`
é reutilizado. Schema, nomes, ordem, tipos, precisão/escala e valores M2 são
preservados. A validação reabre o Parquet nos testes; a CLI não faz uma segunda
leitura obrigatória do resultado.

Entrada vazia produz Parquet vazio com schema. Registro incompleto no EOF,
batch zero, overflow e dados numéricos inválidos retornam erros tipados. Erros
de decoding indicam a posição absoluta no arquivo e o índice global do registro;
o offset de byte do contexto M2 permanece relativo ao batch. Layouts somente FILLER são rejeitados na conversão M3, sem alterar
seu suporte no compilador ou decoder.

A memória dos dados é limitada por batch; o footer Parquet acumula metadados
proporcionais à quantidade de row groups. Uma falha pode deixar saída parcial;
não há atomic commit, manifest, retry ou retomada. O teste `local_conversion`
executa a CLI sobre a fixture conhecida com batch de dois registros, reabre a
saída e compara schema e valores com constantes independentes do decoder.

## Conversão recuperável M4

```bash
cargo run -- convert-parts --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output-dir sample-parts --batch-records 2
cargo run -- convert-parts --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output-dir sample-parts --batch-records 2 --resume
```

Os quatro argumentos são obrigatórios em ambos os modos. Sem `--resume`, o
diretório de saída deve ser novo, com pai existente; com a flag, deve existir.
Um batch corresponde a uma parte Parquet, com nomes e intervalos determinísticos.
O exemplo produz duas partes (2 + 1 registros). Entrada vazia produz uma parte
com schema e zero linhas.

A biblioteca expõe `convert_parts(&CompiledCopybook, &Path, &Path, usize,
RecoveryMode) -> Result<(), RecoveryError>`, com modos `Create` e `Resume`.
`manifest.json` identifica a conversão; cada parte publicada recebe um recibo
imutável em `commits/`; `complete.json` marca a conclusão. Um Parquet sem recibo
é órfão, não um commit. Resume valida entrada, layout, configuração e todos os
confirmados antes de limpar staging ou regenerar o próximo órfão.

A identidade SHA-256 usa o conteúdo integral da entrada e o layout/schema
canônico. Entrada idêntica em outro caminho pode retomar; entrada alterada,
layout diferente ou outro tamanho de batch exige novo destino. Partes confirmadas
ausentes ou corrompidas causam erro e nunca são regeneradas automaticamente.
Uma retomada concluída revalida o resultado sem reescrever partes confirmadas.

O alvo inicial é Windows/MSVC com NTFS local e Rust 1.89 ou superior. Um lock de
arquivo impede invocações M4 simultâneas no mesmo destino. Staging e publicação
permanecem no mesmo filesystem; a garantia cobre falha do processo, sem prometer
durabilidade após queda de energia ou falha do sistema operacional. A entrada e
o diretório administrado devem permanecer imutáveis para outros programas durante
cada invocação. Hashes conferem identidade/integridade e não protegem o payload.

O [contrato M4](docs/M4_RECOVERY.md) define formato, bootstrap, recuperação,
invariantes, fault injection, critérios de aceite e limitações. Dados e hashing
usam memória limitada; artefatos e metadados em disco crescem com a quantidade de
partes. A validação da retomada relê entrada e partes confirmadas.

## Proteção experimental M5

O M5 é compilado somente com a feature `pqc` e opera separadamente do pipeline M4:

```bash
cargo run --features pqc -- keygen --output-dir sample-keys
cargo run --features pqc -- protect --input sample.parquet --public-key sample-keys/public.key --output sample.parquet.m5
cargo run --features pqc -- unprotect --input sample.parquet.m5 --secret-key sample-keys/secret.key --output recovered.parquet
```

`keygen` exige um diretório de destino inexistente. `protect` e `unprotect` exigem
um diretório pai existente e nunca sobrescrevem o nome final. A garantia de
publicação M5 v1 cobre somente Windows/MSVC em volume NTFS local: staging e destino
ficam no mesmo diretório e o commit usa criação atômica de hard link com falha se o
nome final existir. Outros filesystems, compartilhamentos e plataformas falham
fechado. Nenhuma operação M5 escreve em namespace administrado pelo M4; um artefato
M4 pode ser usado somente como entrada de leitura.

A suíte fechada v1 usa ML-KEM-768 para estabelecimento de chave, HKDF-SHA-256 e
AES-256-GCM em STREAM-BE32, com chunks de 1 MiB e cabeçalho integral como AAD de
cada frame. O limite formal é `2^32` frames e `2^52` bytes de plaintext. A produção
obtém toda entropia do sistema operacional. `recipient_public_key_sha256` é apenas
fingerprint/identificador da representação da chave pública; sua integridade vem do
AAD autenticado e ele não autentica a identidade do destinatário.

A biblioteca expõe `generate_keypair`, `protect_file` e `unprotect_file` em
`m2c_pipeline::protection`. As operações processam o payload com memória limitada,
publicam somente após validação integral e retornam erros, avisos de permissão e
status de resíduo de staging tipados. Permissões restritivas e zeroização dos
buffers secretos possuídos pelo M2C são mitigações de melhor esforço. Proteção da
chave secreta em repouso, assinatura, múltiplos destinatários, KMS/HSM, integração
M4 e suporte cloud permanecem fora do escopo.

O formato binário, modelo de falhas, limites e limitações normativas estão no
[contrato congelado M5](docs/M5_PROTECTION.md).

`keygen` publica cada arquivo atomicamente, mas não oferece uma transação da keypair
inteira: `public.key` é publicado antes de `secret.key`. Se a segunda publicação
falhar, a operação retorna erro e preserva a chave pública já publicada; o diretório
parcial não é adotado nem sobrescrito em nova execução e exige tratamento manual.
A limpeza dos stagings próprios é best-effort. Um resíduo público pós-commit também
pode permanecer. Nesse erro não há `KeyGenerationOutcome`, portanto os avisos e o
status do primeiro commit não são retornados separadamente. A garantia de ausência
de publicação parcial refere-se ao conteúdo de cada arquivo, não ao par como transação.

Durante `unprotect`, plaintext autenticado de frames anteriores pode existir no
staging antes da autenticação do arquivo completo. Em retornos normais de erro,
`Drop` tenta remover esse staging em best-effort. Encerramento abrupto do processo ou
queda de energia antes do commit pode deixar `.m2c-m5-staging-*` contendo plaintext
parcial, sem publicar o destino final. O destino só é publicado após autenticação
completa e validação de tamanho. Cleanup/recovery de staging após crash e resume
estão fora do M5; não há garantia adicional de proteção contra acesso local ao
staging durante a operação ou após crash.

## Roadmap

- **M0 — fundação:** status e documentação honestos, módulos e contratos compatíveis com a arquitetura v0.1, CI local limpo.
- **M1 — copybook compiler:** normalização fixed-format, parser do subconjunto, AST mínima, layout compilado, Arrow Schema e diagnósticos.
- **M2 — codecs e Arrow:** CP037, DISPLAY, COMP/BINARY, COMP-3 e produção de `RecordBatch` tipado.
- **M3 — Core MVP local:** source fixed-record, batches com memória limitada, CLI de conversão e escrita/validação de Parquet local.
- **M4 — robustez e recuperação:** partes determinísticas, manifest, atomic commit, fault injection e retomada.
- **M5 — proteção PQC experimental (implementado):** AEAD para o payload e ML-KEM para estabelecimento/proteção de chaves, com suíte fechada e versionada.
- **M6 — evidência técnica e demo:** observabilidade local, fuzzing ampliado, benchmarks reproduzíveis e demonstração documentada.
- **M7 — extensões opcionais:** sink de object storage/cloud, ML-DSA e novos formatos apenas depois da versão de portfólio local.

O projeto não pretende implementar COBOL completo, substituir ferramentas IBM, criar um database engine ou oferecer infraestrutura enterprise.

## Documentação

- [Arquitetura v0.1](docs/ARCHITECTURE.md)
- [Subconjunto COBOL Copybook v0.1](docs/COPYBOOK_SUBSET.md)
- [Decoding de registros M2](docs/DECODING.md)
- [Recuperação local M4](docs/M4_RECOVERY.md)
- [Proteção experimental M5](docs/M5_PROTECTION.md)
- [Benchmarks e Desempenho](docs/BENCHMARKS.md)
- [Validação Externa Spark/Cobrix](docs/EXTERNAL_COMPATIBILITY.md)
- [Análise inicial do projeto](docs/ANALISE_DO_PROJETO.md)

## Referências

- [Apache Arrow](https://arrow.apache.org/)
- [Apache Parquet](https://parquet.apache.org/)
- [NIST FIPS 203 — ML-KEM](https://csrc.nist.gov/pubs/fips/203/final)
- [IBM Enterprise COBOL documentation](https://www.ibm.com/docs/en/cobol-zos)
