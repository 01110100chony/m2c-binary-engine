# Arquitetura v0.1

## Status e propósito

Esta é a arquitetura congelada para a reconstrução do M2C Quantum-Safe Data Pipeline. O projeto é experimental, educacional e orientado a portfólio. A prioridade é demonstrar correctness, limites de memória, interoperabilidade de formatos e decisões de segurança justificadas; não construir uma plataforma enterprise.

O escopo implementado inclui M5: compilador de copybook, codecs, decoding de batches
para Arrow, conversão local síncrona para Parquet, conversão recuperável em partes e
proteção autônoma opcional de arquivos. As APIs M3 e M4 mantêm seus contratos.
Cloud continua uma etapa posterior.

## Forma do repositório

O projeto permanece em **um único pacote Rust**, contendo:

- uma biblioteca com tipos, parser, compilador e pipeline local;
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
       ou partes + manifest + retomada local         [M4]
    -> proteção híbrida opcional de artefato         [M5]
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

No M4, `convert-parts` recebe copybook, entrada, diretório de saída e limite de
batch, com `--resume` opcional. Chama `convert_parts` com `RecoveryMode::Create`
ou `RecoveryMode::Resume`. Não expõe controles de fault injection.

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
de artefatos pertencem à API adicional M4.

Entrada vazia preserva schema com zero linhas. Layouts somente FILLER recebem
erro explícito antes da criação da saída; o contrato M1/M2 para esses layouts
permanece intacto. EOF com registro parcial, capacidade inválida e erros de
decoding/I/O/Parquet interrompem a conversão. Causas originais são preservadas;
erros de decoding acrescentam o offset do batch no arquivo e traduzem o índice
do registro para o índice global, preservando o offset de byte relativo, campo,
span e causa M2. A CLI informa o erro em stderr e retorna status não zero.

Na API M3, um destino existente nunca é sobrescrito. Falhas podem deixar arquivos parciais;
não há transação, remoção automática, manifest, retry, checkpoint ou retomada.
A validação de saída ocorre por reabertura em testes, sem uma segunda passagem
obrigatória em runtime.

## Componentes do M4

- `manifest`: documentos JSON tipados, identidade por SHA-256, intervalos de partes
  e validação de descritor, recibos e conclusão.
- `recovery`: `convert_parts(&CompiledCopybook, &Path, &Path, usize, RecoveryMode)`
  retorna `Result<(), RecoveryError>`; administra lock local, staging, publicação,
  prefixo confirmado e retomada. Hooks de falha permanecem internos aos testes.
- `parquet_io`: compartilha a construção do writer com M3; M4 retém o arquivo para
  finalizar, sincronizar e fechar antes de publicar cada parte.

Os módulos permanecem internos. São exportados somente a nova conversão, modo e
erro. Source e decoder são reutilizados sem mudança semântica. O copybook é
compilado uma vez e um único decoder é validado/reutilizado por invocação.

Uma parte corresponde a um batch, com identidade e intervalo determinísticos.
O manifest combina descritor imutável, um recibo imutável por parte e marcador de
conclusão. Staging é finalizado, sincronizado e publicado no mesmo filesystem;
a parte precede seu recibo. Somente o recibo válido, dentro de um prefixo sem
lacunas e com parte íntegra, autoriza avanço de cursor. A retomada valida todo o
prefixo antes de apagar temporários ou regenerar o próximo órfão. Confirmados
ausentes/corrompidos causam erro, sem rollback nem regeneração automática.

Identidade inclui conteúdo integral da entrada, layout físico, schema Arrow e
batch configurado. Spans e formatação do copybook não integram a identidade.
Schema, valores e diagnósticos M2/M3 são preservados, incluindo índices globais
de registro após o seek de retomada. Entrada vazia produz uma parte vazia com schema.

`File::try_lock` exclui invocações simultâneas no mesmo diretório. A garantia
inicial cobre falha do processo sobre Windows/MSVC e NTFS local, com filesystem
operacional e sem escritores externos. `sync_all` dos arquivos não oferece
durabilidade dos renames após perda de energia: não há sincronização portátil
de diretórios. Não há transação conjunta de parte/recibo, retry automático,
coordenação distribuída nem promessa de recuperação em rede/cloud.

O contrato completo, inclusive bootstrap interrompido e validação de namespace,
está em [M4_RECOVERY.md](M4_RECOVERY.md).

## Componentes do M5

### `protection`

O módulo opcional, compilado pela feature Cargo `pqc`, protege e recupera arquivos
sem conhecer copybooks, Arrow, Parquet ou estado de recuperação. A API pública é
limitada a geração de chaves, `protect_file` e `unprotect_file`; algoritmo, RNG,
nonce e salt não são selecionáveis pelo chamador.

A suíte v1 fechada combina ML-KEM-768, HKDF-SHA-256 e AES-256-GCM em
STREAM-BE32. O cabeçalho integral é AAD de cada frame, o payload é processado em
chunks de 1 MiB e nenhuma saída final aparece antes da validação completa. Tamanhos,
contadores e o limite de `2^52` bytes usam aritmética verificada.

Publicação M5 é restrita a Windows/MSVC e volume NTFS local. O staging fica no
mesmo diretório e o commit no-clobber cria atomicamente um hard link somente se o
nome final estiver ausente. Caminhos com reparse points e qualquer destino dentro
de namespace M4 são rejeitados antes do staging e revalidados antes do commit.
O M5 pode ler artefato M4, mas não o modifica nem participa de sua recuperação.

O contrato completo está em [M5_PROTECTION.md](M5_PROTECTION.md).

A atomicidade de `keygen` é por arquivo: o commit de `public.key` precede o de
`secret.key`. Falha no segundo retorna erro sem rollback do primeiro; o diretório
parcial exige tratamento manual e nunca é reutilizado automaticamente. Isso não
constitui uma transação da keypair inteira.

## Evidência local M6 e componentes posteriores

M6 adiciona um resumo JSON privado na fronteira da CLI, opcional via
`--report-json`, sem instrumentação do hot path ou alteração das APIs da biblioteca.
Campanhas, verifier externo e runner são tooling de teste/medição. Formatos M4/M5
e semântica de publicação permanecem iguais. Em erro de keygen sem outcome,
`publication` é null mesmo após commit parcial, sem reconstrução pelo filesystem.
Contrato e limitações: [M6_EVIDENCE.md](M6_EVIDENCE.md).

Estes componentes permanecem futuros:

- `sink`: adaptadores de destino e object storage opcional; M3 escreve diretamente no filesystem local;
- infraestrutura de `telemetry`: serviços de coleta, tracing e monitoramento contínuo.

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

No M4, cada parte finaliza seu próprio footer e os hashes são calculados por
streaming. A retomada valida recibos individualmente, sem reter batches ou lista
de registros. O número de artefatos e metadados em disco cresce com as partes.
A entrada é relida integralmente para identidade e as partes confirmadas para
integridade; esse custo é deliberado no contrato M4.

No M5, cabeçalho e material de chave têm tamanho fixo e o payload é lido e escrito
sequencialmente em buffers de no máximo 1 MiB mais overhead constante de AEAD. O
arquivo completo nunca é materializado em memória. Plaintext desprotegido permanece
em staging até autenticação, tamanho e sequência de frames serem integralmente
validados.

Frames já autenticados podem deixar plaintext parcial no staging da desproteção.
Em erro normal, o descarte RAII tenta removê-lo em best-effort. Crash, encerramento
forçado ou perda de energia antes do commit podem impedir esse descarte e deixar
resíduo de plaintext, sem destino final publicado. Não há cleanup/recovery após
crash, resume ou garantia adicional contra acesso local ao staging no M5.

## Segurança e cloud

PQC é uma camada experimental opcional do M5. ML-KEM é usado somente para
estabelecimento de chave e AES-GCM protege o conteúdo; o projeto não implementa
primitivas criptográficas. O fingerprint da chave pública é um identificador, não
uma assinatura ou autenticação de identidade. A entropia de produção vem somente do
sistema operacional. Zeroização e permissões restritivas são melhores esforços
limitados, e proteção de chave secreta em repouso não é oferecida.

O primeiro sink funcional será o filesystem local. Integrações cloud serão adaptadores opcionais depois que o pipeline local estiver correto, testado e demonstrável. O mainframe é a origem lógica dos formatos, não uma dependência de infraestrutura: nenhum IBM Z real é necessário.

## Non-goals da arquitetura v0.1

Não fazem parte do trabalho atual:

- integração automática entre proteção M5 e o pipeline/namespace M4;
- assinatura, múltiplos destinatários, rotação, KMS/HSM ou proteção de chave secreta em repouso;
- Azure, outro cloud provider ou object storage;
- Tokio, canais `mpsc` ou pipeline assíncrono;
- Prometheus, Grafana ou AIOps;
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

## Critério de conclusão do M4

A fixture independente de três registros de 35 bytes deve produzir duas partes
(2 + 1) com batch de dois e três partes com batch de um. Reabertura e concatenação
devem preservar schema e valores exatos. Interrupções de processo e erros de I/O
nas fronteiras de staging, finalização, sync, publicação, commit, limpeza e
conclusão devem retomar sem perdas, duplicação, reordenação ou reescrita de partes
confirmadas. Identidades incompatíveis, corrupção e estados inconsistentes devem
falhar explicitamente antes de limpeza/progresso. Entrada vazia, retomada concluída,
diagnósticos globais e exclusão mútua fazem parte do aceite.

A matriz de testes e os gates completos estão em [M4_RECOVERY.md](M4_RECOVERY.md).
Toda a suíte M0–M3, os testes M4 e doctests devem passar, junto de formatação e
Clippy sem warnings. M5 não começa como parte desse trabalho.

## Critério de conclusão do M5

Um arquivo arbitrário, inclusive vazio, deve fazer round-trip byte a byte somente
com a chave secreta correspondente. Alteração de cabeçalho, frames, ordem, tags,
truncamento, chave errada, formato inválido, limite excedido e publicação insegura
devem falhar fechado sem saída parcial. Publicação deve ser no-clobber em NTFS local,
e nenhum destino ou staging pode entrar em namespace M4.

O aceite exige fixtures independentes, testes adversariais, regressões M0–M4 e toda
a matriz G4. O contrato normativo e os limites estão em
[M5_PROTECTION.md](M5_PROTECTION.md). M6 não começa como parte desse trabalho.
