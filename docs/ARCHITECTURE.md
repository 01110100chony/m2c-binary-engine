# Arquitetura v0.1

## Status e propósito

Esta é a arquitetura congelada para a reconstrução do M2C Quantum-Safe Data Pipeline. O projeto é experimental, educacional e orientado a portfólio. A prioridade é demonstrar correctness, limites de memória, interoperabilidade de formatos e decisões de segurança justificadas; não construir uma plataforma enterprise.

O escopo de implementação atual termina no M3: compilador de copybook, codecs, decoding de batches para Arrow e conversão local síncrona para Parquet. As etapas posteriores estabelecem apenas limites entre componentes e não afirmam que recuperação, proteção ou cloud já existem.

## Forma do repositório

O projeto permanece em **um único pacote Rust**, contendo:

- uma biblioteca com tipos, parser, compilador e, futuramente, o pipeline;
- uma CLI fina que chama a biblioteca;
- testes unitários e golden fixtures pequenos e determinísticos.

Não haverá workspace de serviços, sistema de plugins ou framework de providers nesta fase. APIs devem ser pequenas, explícitas e orientadas pelos casos de uso já definidos.

## Fluxo de dados congelado

```text
COBOL copybook
    -> normalização fixed-format
    -> parser do subconjunto
    -> AST mínima
    -> CompiledCopybook

arquivo binário fixed-record
    -> source local                                 [M3]
    -> batches com memória limitada                 [M3]
    -> decoding orientado pelo layout compilado     [M2]
    -> Arrow RecordBatch                            [M2]
    -> row groups em um arquivo Parquet local        [M3]
    -> proteção híbrida opcional                    [milestone posterior]
    -> adaptadores opcionais de destino             [milestone posterior]
```

O copybook é interpretado **uma única vez**. O resultado compilado resolve offsets, comprimentos físicos, encoding, signedness, precisão, escala, tipo lógico Arrow e tamanho total do registro. O hot path recebe esse layout pronto e não volta a interpretar tokens, cláusulas ou strings PIC.

## Componentes do M0/M1

### `copybook`

Responsável por:

- normalizar linhas COBOL fixed-format;
- identificar a posição original para diagnósticos;
- tokenizar e fazer parsing apenas do subconjunto v0.1;
- produzir uma AST mínima;
- rejeitar explicitamente toda sintaxe ou cláusula fora do contrato.

O contrato sintático e semântico completo está em [COPYBOOK_SUBSET.md](COPYBOOK_SUBSET.md).

### `schema`

Responsável por compilar a AST para `CompiledCopybook`:

- calcular offsets determinísticos em ordem de declaração;
- calcular byte lengths conforme o encoding físico;
- resolver signedness, precision e scale;
- resolver o tipo lógico Arrow;
- calcular `record_length`;
- gerar o Arrow Schema sem grupos e sem `FILLER`.

Grupos organizam hierarquia, mas não consomem bytes por conta própria. Somente campos elementares contribuem para o layout físico. Um `FILLER` elementar contribui normalmente para offsets e `record_length`, embora seja omitido do schema público.

### `error`

Responsável por erros tipados. Erros de copybook devem informar, no mínimo, linha, coluna e causa. Entradas inválidas, cláusulas não suportadas e overflow de cálculos devem retornar erro; não podem causar panic.

### `cli`

Permanece uma camada fina sobre a biblioteca. No M3, `convert` recebe caminhos de copybook, entrada e saída e um limite positivo de registros por batch. Compila o copybook uma vez e chama a conversão local. A API de inspeção/compilação M1 permanece preservada.

## Componentes do M2

- `codec`: funções internas para CP037, DISPLAY, COMP/BINARY big-endian e COMP-3; retornam texto ou inteiros sem escala aplicada, nunca ponto flutuante.
- `decode`: `RecordDecoder` empresta o `CompiledCopybook`, valida seus campos públicos uma vez e produz `RecordBatch` com builders Arrow. Não há uma segunda AST ou arquitetura de layouts.
- `error`: também contém `DecodeError`, suas causas tipadas e contexto de registro/campo, offset no batch e localização original no copybook.

Cada chamada recebe bytes de registros concatenados, possui builders próprios e retorna um batch completo ou o primeiro erro. O schema compilado é preservado, inclusive nomes e não nulabilidade. FILLER mantém offsets sem produzir valores. O [contrato M2](DECODING.md) detalha validação, sinais e capacidade.

## Componentes do M3

- `source`: buffer limitado por `record_length × batch_records`, com cálculo de capacidade verificado e leitura síncrona que trata leituras curtas e interrupções;
- `parquet_io`: criação exclusiva de uma saída local, sem sobrescrita, usando ArrowWriter e Parquet sem compressão adicional;
- `pipeline`: `convert_file(&CompiledCopybook, &Path, &Path, usize)` retorna `Result<(), ConversionError>`, reutiliza um único RecordDecoder e escreve/finaliza cada batch como row group.

Os módulos são internos; somente a função de conversão e o erro são exportados.
Os caminhos da API são entrada e saída, respectivamente. A configuração de CLI
usa apenas `args_os`, sem arquivo de configuração nem framework de providers.
O contrato mínimo M3 concretiza a escrita incremental anteriormente descrita
como partes em um único arquivo Parquet; particionamento e lifecycle recuperável
de artefatos permanecem para M4.

Entrada vazia preserva schema com zero linhas. Layouts somente FILLER recebem
erro explícito antes da criação da saída; o contrato M1/M2 para esses layouts
permanece intacto. EOF com registro parcial, capacidade inválida e erros de
decoding/I/O/Parquet interrompem a conversão. Causas originais são preservadas;
erros de decoding acrescentam o offset do batch no arquivo e traduzem o índice
do registro para o índice global, preservando o offset de byte relativo, campo,
span e causa M2. A CLI informa o erro em stderr e retorna status não zero.

Um destino existente nunca é sobrescrito. Falhas podem deixar arquivos parciais;
não há transação, remoção automática, manifest, retry, checkpoint ou retomada.
A validação de saída ocorre por reabertura em testes, sem uma segunda passagem
obrigatória em runtime.

## Componentes posteriores já delimitados

Estes componentes não pertencem ao M0/M1/M2/M3:

- `sink`: adaptadores de destino e object storage opcional; M3 escreve diretamente no filesystem local;
- `crypto`: envelope versionado com AEAD para bulk data e ML-KEM para chaves;
- `telemetry`: logs estruturados e estatísticas reproduzíveis.

Sua enumeração registra fronteiras futuras; não autoriza implementá-los antecipadamente.

## Invariantes de correctness

1. Um copybook válido é compilado uma vez para uma estrutura imutável e autocontida.
2. A soma dos byte lengths dos campos elementares, incluindo `FILLER`, determina `record_length`.
3. Offsets são contíguos, determinísticos e não se sobrepõem.
4. Grupos não acrescentam bytes além dos seus filhos.
5. `FILLER` nunca aparece no Arrow Schema.
6. O tamanho físico de COMP/BINARY segue a regra IBM definida para o subset, não uma aproximação genérica baseada em bits ou caracteres.
7. `V` é um ponto decimal implícito: afeta precisão, escala e tipo lógico, mas não ocupa byte.
8. Toda cláusula desconhecida ou não suportada falha explicitamente.
9. Nenhuma entrada inválida deve causar panic.
10. Correctness e simplicidade prevalecem sobre alegações de zero-copy.

## Execução e memória

O M1 não processa datasets. O M2 recebe um batch em memória, limitado pelo chamador, e não lê arquivos nem retém registros entre chamadas. A memória adicional corresponde às colunas Arrow e a uma string temporária reutilizada. No M3, o source local produz batches limitados por configuração e o pipeline escreve e descarrega cada row group antes de ler o próximo batch. Os dados do arquivo inteiro não são materializados em memória; o footer Parquet mantém metadados proporcionais ao número de row groups. A execução é síncrona; concorrência só poderá ser introduzida com evidência de benchmark e sem alterar os contratos centrais.

## Segurança e cloud

PQC é uma decisão experimental posterior. Quando implementada, a proteção usará uma cifra AEAD para o conteúdo e ML-KEM somente para estabelecimento/proteção de chaves; nenhuma primitiva criptográfica será implementada pelo projeto. O formato e o threat model exigirão documentação própria antes do código.

O primeiro sink funcional será o filesystem local. Integrações cloud serão adaptadores opcionais depois que o pipeline local estiver correto, testado e demonstrável. O mainframe é a origem lógica dos formatos, não uma dependência de infraestrutura: nenhum IBM Z real é necessário.

## Non-goals da arquitetura v0.1

Não fazem parte do trabalho atual:

- PQC ou qualquer criptografia de payload;
- Azure, outro cloud provider ou object storage;
- Tokio, canais `mpsc` ou pipeline assíncrono;
- Prometheus, Grafana ou AIOps;
- checkpoints e resume;
- ROOT ou integrações externas de análise;
- UI;
- Kubernetes ou microservices;
- HSM/KMS real;
- database ou SQL engine;
- registros variáveis, múltiplos layouts ou COBOL completo.

Também não são objetivos futuros: distributed consensus, reimplementação de primitivas criptográficas ou competição com ferramentas IBM.

## Critério de conclusão do M1

M1 termina quando fixtures golden comprovarem, de forma determinística:

```text
copybook -> AST -> CompiledCopybook
```

incluindo `record_length`, offsets, byte lengths, physical encodings, signedness, precision/scale e Arrow Schema, além de diagnósticos explícitos para construções não suportadas. M2 não começa como parte desse trabalho.

## Critério de conclusão do M2

Uma fixture binária conhecida junto ao layout compilado M1 deve produzir o RecordBatch esperado exatamente, incluindo schema, ordem, valores e escala. Testes dos codecs, rejeições, propriedades reproduzíveis e toda a suíte M0/M1 devem passar, junto de formatação e Clippy sem warnings. M3 não começa como parte desse trabalho.

## Critério de conclusão do M3

A CLI deve converter a fixture fixed-record conhecida usando o layout compilado
e o decoder M2 em um arquivo Parquet local reabrível. A leitura deve preservar
exatamente schema, nomes e ordem dos campos, tipos lógicos Arrow, precisão/escala
Decimal128, contagem de linhas e valores. O teste de integração deve atravessar
ao menos uma fronteira de batch, comprovando processamento em batches limitados.
O teste usa três registros conhecidos, batch de dois e dois row groups (2 + 1).
Toda a suíte M0–M2, os testes M3 e doctests devem passar, junto de formatação e
Clippy sem warnings. M4 não começa como parte desse trabalho.
