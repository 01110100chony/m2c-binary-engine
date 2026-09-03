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

Este é um projeto educacional e de portfólio, mantido por um estudante de Engenharia da Computação. A arquitetura v0.1 está congelada. **M0 (fundação do repositório) e M1 (compilador de copybook) estão concluídos; M2 não foi iniciado.**

O projeto ainda **não** oferece um pipeline end-to-end. Decoding de registros, escrita Arrow/Parquet, criptografia pós-quântica, cloud, checkpoints e observabilidade operacional pertencem a milestones posteriores. O software não deve ser usado para dados sensíveis ou cargas de produção.

## Objetivo da fase atual

O M1 transforma um copybook do subconjunto documentado em uma representação compilada, sem reinterpretar COBOL no futuro hot path de decoding:

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

O repositório usa um único pacote Rust com biblioteca e CLI. O fluxo local planejado é:

1. interpretar e compilar o copybook uma única vez;
2. ler um arquivo binário de registros de tamanho fixo;
3. em M2, implementar codecs e decodificar dados para Arrow;
4. em M3, processar batches com memória limitada e escrever partes Parquet no filesystem local;
5. em milestones posteriores, adicionar robustez operacional e proteção híbrida AEAD + ML-KEM;
6. somente depois da demonstração local, considerar um sink de object storage.

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
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

Os testes do M1 usam fixtures pequenas e determinísticas para validar AST, layout compilado, offsets, tamanho do registro, tipos e rejeição de construções não suportadas. Datasets públicos e sintéticos maiores serão introduzidos apenas quando o pipeline de decoding existir.

A API de entrada do milestone é `parse_and_compile_copybook(&str)`. Para inspecionar separadamente as duas etapas, use `parse_copybook(&str)` seguido de `compile_copybook(&CopybookAst)`.

## Roadmap

- **M0 — fundação:** status e documentação honestos, módulos e contratos compatíveis com a arquitetura v0.1, CI local limpo.
- **M1 — copybook compiler:** normalização fixed-format, parser do subconjunto, AST mínima, layout compilado, Arrow Schema e diagnósticos.
- **M2 — codecs e Arrow:** CP037, DISPLAY, COMP/BINARY, COMP-3 e produção de `RecordBatch` tipado.
- **M3 — Core MVP local:** source fixed-record, batches com memória limitada, CLI de conversão e escrita/validação de Parquet local.
- **M4 — robustez e recuperação:** partes determinísticas, manifest, atomic commit, fault injection e retomada.
- **M5 — proteção PQC experimental:** AEAD para o payload e ML-KEM para estabelecimento/proteção de chaves, com suites versionadas.
- **M6 — evidência técnica e demo:** observabilidade local, fuzzing ampliado, benchmarks reproduzíveis e demonstração documentada.
- **M7 — extensões opcionais:** sink de object storage/cloud, ML-DSA e novos formatos apenas depois da versão de portfólio local.

O projeto não pretende implementar COBOL completo, substituir ferramentas IBM, criar um database engine ou oferecer infraestrutura enterprise.

## Documentação

- [Arquitetura v0.1](docs/ARCHITECTURE.md)
- [Subconjunto COBOL Copybook v0.1](docs/COPYBOOK_SUBSET.md)
- [Análise inicial do projeto](docs/ANALISE_DO_PROJETO.md)

## Referências

- [Apache Arrow](https://arrow.apache.org/)
- [Apache Parquet](https://parquet.apache.org/)
- [NIST FIPS 203 — ML-KEM](https://csrc.nist.gov/pubs/fips/203/final)
- [IBM Enterprise COBOL documentation](https://www.ibm.com/docs/en/cobol-zos)
