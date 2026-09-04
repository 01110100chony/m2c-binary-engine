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

Este é um projeto educacional e de portfólio, mantido por um estudante de Engenharia da Computação. A arquitetura v0.1 está congelada. **M0 (fundação), M1 (compilador de copybook) e M2 (codecs e Arrow RecordBatch) estão implementados.**

O projeto ainda **não** oferece um pipeline de arquivos end-to-end. A biblioteca decodifica batches binários em memória para Arrow. Leitura de arquivos, CLI funcional, escrita Parquet, criptografia pós-quântica, cloud, checkpoints e observabilidade operacional pertencem a milestones posteriores. O software não deve ser usado para dados sensíveis ou cargas de produção.

## Objetivo da fase atual

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

O repositório usa um único pacote Rust com biblioteca e CLI. O fluxo local planejado é:

1. interpretar e compilar o copybook uma única vez;
2. em M3, ler um arquivo binário de registros de tamanho fixo em batches limitados;
3. usar os codecs M2 para decodificar cada batch para Arrow;
4. em M3, escrever partes Parquet incrementalmente no filesystem local;
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
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --doc
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
- [Decoding de registros M2](docs/DECODING.md)
- [Análise inicial do projeto](docs/ANALISE_DO_PROJETO.md)

## Referências

- [Apache Arrow](https://arrow.apache.org/)
- [Apache Parquet](https://parquet.apache.org/)
- [NIST FIPS 203 — ML-KEM](https://csrc.nist.gov/pubs/fips/203/final)
- [IBM Enterprise COBOL documentation](https://www.ibm.com/docs/en/cobol-zos)
