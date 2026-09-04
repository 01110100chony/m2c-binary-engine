# Recuperação local M4

## Contrato e fronteira

M4 acrescenta conversão síncrona local para partes Parquet determinísticas,
manifest, publicação atômica de arquivos, retomada explícita e injeção de falhas
em testes. Mantém o pacote Rust único, o layout compilado uma vez, o decoder M2
reutilizado e os batches limitados do source M3. `convert_file` e o comando
`convert` continuam oferecendo a saída única e o comportamento M3.

O alvo inicial de recuperação é Windows/MSVC sobre NTFS local. A garantia cobre
encerramento abrupto do processo enquanto sistema operacional e filesystem
permanecem operacionais. Não cobre perda de energia, reboot abrupto, falha do
kernel, filesystem de rede ou diretório sincronizado. Os dados da entrada e o
diretório administrado não podem ser alterados por outros programas durante
uma invocação.

## API e operação

```rust,ignore
pub enum RecoveryMode {
    Create,
    Resume,
}

pub fn convert_parts(
    layout: &CompiledCopybook,
    input: &Path,
    output_dir: &Path,
    batch_records: usize,
    mode: RecoveryMode,
) -> Result<(), RecoveryError>;
```

```bash
cargo run -- convert-parts --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output-dir sample-parts --batch-records 2
cargo run -- convert-parts --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output-dir sample-parts --batch-records 2 --resume
```

Os quatro argumentos são obrigatórios nos dois modos. `--batch-records` deve ser
positivo. Sem `--resume`, a conversão exige diretório de saída inexistente e pai
existente. Com `--resume`, exige diretório existente. Sucesso retorna status zero;
erros são escritos em stderr e retornam status não zero. Não há retry automático.

O chamador fornece o copybook/layout e o arquivo de entrada inclusive na retomada
de conversões concluídas. Entrada idêntica em outro caminho é aceita; mudança de
conteúdo, layout semântico ou tamanho de batch exige outro diretório de saída.

## Partes e memória

Para comprimento de registro `L`, limite de batch `B` e total de registros `N`:

```text
P = max(1, ceil(N / B))
start_record(i) = i × B
record_count(i) = min(B, N − start_record(i))
input_offset(i) = start_record(i) × L
```

Os índices começam em zero e toda aritmética é verificada. O caso vazio tem uma
parte de índice zero, início zero e contagem zero. Seu commit avança o índice de
parte, embora o offset da entrada permaneça zero.

Cada parte não vazia contém um row group. A parte vazia contém schema, zero linhas
e zero row groups. A configuração Parquet segue M3, sem compressão adicional.
Schema, nomes, ordem, nulabilidade, metadados, tipos e valores são preservados,
inclusive espaços e controles CP037 e precisão/escala Decimal128.

Os dados são limitados a um batch; hashes são calculados por streaming. A
retomada valida recibos individualmente, sem acumular registros ou batches.
Arquivos e metadados em disco crescem proporcionalmente à quantidade de partes.
O custo inclui uma passagem integral da entrada para identidade e a leitura das
partes para integridade; M4 não oferece uma meta específica de desempenho.

## Artefatos e identidade

```text
<output-dir>/
├── .m4.lock
├── manifest.json
├── .manifest.json.tmp
├── parts/
│   ├── part-00000000000000000000.parquet
│   └── .part-00000000000000000001.parquet.tmp
├── commits/
│   ├── part-00000000000000000000.json
│   └── .part-00000000000000000001.json.tmp
├── complete.json
└── .complete.json.tmp
```

Arquivos `.tmp` existem apenas durante staging ou após interrupção. Os nomes das
partes usam índice decimal de vinte posições: `part-{i:020}.parquet`. Sua identidade
é `(job_id, i)`. Caminhos são derivados dos índices, nunca lidos de campos
arbitrários do manifest. Não são aceitos symlinks/reparse points em artefatos
administrados; a entrada deve ficar fora do diretório de saída.

O manifest é composto por documentos JSON imutáveis:

| Documento | Campos obrigatórios |
|---|---|
| `manifest.json` | `format`, `version`, `input_bytes`, `input_sha256`, `layout_sha256`, `record_length`, `batch_records`, `profile`, `job_id` |
| `commits/part-{i:020}.json` | `version`, `job_id`, `part_index`, `start_record`, `record_count`, `parquet_bytes`, `parquet_sha256` |
| `complete.json` | `version`, `job_id`, `part_count`, `total_records` |

Valores iniciais fixos:

```text
format = "m2c-m4"
version = 1
profile = "m2c-v0.1-cp037-parquet53-uncompressed-v1"
```

Cada documento tem limite de 4 KiB, aplicado antes da desserialização. Campos
desconhecidos, duplicados ou ausentes, valores numéricos inválidos, hashes fora do
formato e conteúdo residual são rejeitados. Versões e profiles desconhecidos
retornam erro; não há migração de manifest.

Antes de criar ou retomar, um único `RecordDecoder` valida o layout. Schema sem
colunas, capacidade inválida e entrada que não seja arquivo regular são
rejeitados. O SHA-256 integral da entrada e sua contagem de bytes são calculados
em buffer limitado. Comprimento não múltiplo de `L` causa erro com a posição do
registro incompleto. A identidade não usa caminho, timestamp ou tamanho isolado.

O fingerprint do layout inclui nome e comprimento do layout, todos os campos
físicos incluindo FILLER, paths, nomes, offsets, comprimentos, encoding,
signedness, precisão, escala e tipos lógicos. Inclui ainda nomes, ordem, tipos,
nulabilidade e metadados dos campos Arrow e os metadados do schema. Spans do
copybook e sua formatação são excluídos: copybooks semanticamente equivalentes
podem retomar a mesma conversão, usando os spans da invocação atual nos erros.

A representação de identidade é JSON compacto de DTOs internos, com chaves
ordenadas recursivamente, arrays em ordem original, números inteiros e nomes de
enums explícitos em `snake_case`. Não depende de `Debug`, `DefaultHasher` ou ordem
de iteração de `HashMap`. `job_id` é o SHA-256 da representação canônica do
descritor excluindo seu próprio campo `job_id`.

SHA-256 detecta diferenças de conteúdo para recuperação; não autentica origem nem
impede adulteração coordenada de dados e metadados. Não é proteção de payload.

## Commit e exclusão mútua

Cada invocação mantém `.m4.lock` aberto para leitura/escrita, sem truncamento,
e adquire `File::try_lock` antes de examinar ou modificar o estado recuperável.
Contenção retorna erro imediato. O arquivo permanece no diretório após a
execução; sua existência não significa lock ativo. O lock do sistema operacional
é liberado quando o processo termina. A API exige Rust 1.89 ou superior.

Uma parte está logicamente confirmada quando possui recibo final válido que
pertence ao job e integra um prefixo sem lacunas iniciado em zero. A parte deve
existir e corresponder a tamanho, hash, schema e contagem registrados/esperados.
Um Parquet sem recibo é órfão, ainda que perfeitamente legível. Um temporário
nunca autoriza avanço de cursor.

Para cada parte:

1. Criar staging exclusivamente, escrever e finalizar o Parquet.
2. Executar `sync_all`, fechar handles, verificar tamanho/hash e footer.
3. Verificar ausência do destino e publicar a parte por rename no mesmo filesystem.
4. Criar recibo em staging, escrever completamente, sincronizar e fechar.
5. Verificar ausência do destino e publicar o recibo por rename.
6. Só então avançar o cursor confirmado.

Nomes finais são imutáveis. A verificação de ausência ocorre sob o lock exclusivo,
no modelo que exclui escritores externos. Não há fallback por copiar/apagar,
remoção prévia de confirmado nem sobrescrita. O rename publica cada arquivo;
parte e recibo não constituem uma transação física conjunta.

Após o último recibo cobrir exatamente a entrada, `complete.json` é publicado
por staging, `sync_all`, fechamento e rename. O diretório fica visível durante a
conversão; a conclusão lógica é indicada por esse marcador.

`sync_all` é explícito e erros são propagados, mas não há sincronização portátil
de diretórios. Logo, não se promete persistência de renames após falha do sistema
operacional ou perda de energia.

## Bootstrap e retomada

Create valida entrada/configuração antes de criar o diretório, lock,
subdiretórios e descritor. O descritor é publicado antes de qualquer parte.

Resume adquire o lock, valida o descritor e compara conteúdo da entrada, layout e
configuração. Valida todo o prefixo confirmado e o estado de conclusão **antes**
de apagar staging ou órfãos. Calcula o cursor exclusivamente pelo prefixo,
executa seek e continua com o source M3. Erros M2 preservam campo, span, causa e
offset relativo ao batch; M4 acrescenta offset absoluto e índice global de
registro, inclusive após o seek.

Sem `manifest.json`, somente um bootstrap é recuperável: lock, subdiretórios
vazios (possivelmente ainda ausentes) e temporário do descritor. Resume valida
esse namespace, descarta o temporário e conclui a inicialização com os argumentos
atuais. Não existe identidade persistida antes de publicar o descritor. Parte,
recibo, conclusão final ou arquivo desconhecido nesse estado causa erro. Se a
interrupção ocorreu antes de criar o diretório, é necessário reexecutar Create.

Com descritor válido, após validar todos os confirmados:

- Apagar temporários reconhecidos, sem seguir links.
- Apagar e regenerar o órfão do próximo índice esperado, sem adotá-lo por conteúdo.
- Recusar partes finais além do próximo índice, recibos com lacunas e nomes desconhecidos.
- Recusar parte confirmada ausente/corrompida e recibo inválido; nunca reduzir o prefixo para contornar corrupção.

A limpeza remove somente arquivos individuais reconhecidos. Interrupção durante
limpeza é retomável porque o prefixo confirmado não muda. Incompatibilidade de
identidade ou integridade não autoriza limpeza ou progresso.

Resume de conversão concluída revalida entrada, layout, recibos e partes e retorna
sucesso sem reescrever artefatos confirmados. Erros não desfazem commits anteriores.

| Estado observado após interrupção | Ação de retomada |
|---|---|
| Diretório ausente | Resume falha; executar Create. |
| Bootstrap sem descritor final | Validar namespace e concluir inicialização. |
| Staging Parquet parcial ou finalizado | Apagar temporário e refazer a parte. |
| Parquet publicado sem recibo | Apagar órfão e refazer a parte. |
| Recibo parcial/completo em staging | Apagar staging e órfão; refazer a parte. |
| Recibo final válido, cursor em memória ainda não atualizado | Preservar parte, reconstruir cursor e continuar na próxima. |
| Todos os recibos presentes, sem conclusão | Publicar somente a conclusão. |
| Conclusão em staging | Descartar temporário e republicar conclusão. |
| Conclusão final válida | Validar e retornar sucesso. |
| Lacuna, recibo inválido, parte confirmada ausente/corrompida | Retornar erro sem limpeza ou progresso. |
| Identidade/configuração divergente | Retornar erro sem alterar artefatos. |

O resultado lógico é a concatenação, na ordem de índice, das partes confirmadas.
Consumidores devem seguir os recibos: fazer glob de todos os Parquets em um job
incompleto pode incluir órfãos. Remoção externa do último recibo de um job
incompleto pode ser indistinguível de um órfão e está fora do modelo suportado.

## Aceite e fault injection

A fixture independente de três registros de 35 bytes, com `B=2`, deve produzir
partes nos intervalos `[0,2)` e `[2,3)`, com dois e um registros. Com `B=1`, deve
produzir três partes. A reabertura e concatenação preservam exatamente schema e
valores esperados independentes do decoder. Entrada vazia produz uma parte vazia.

Interrupções seguidas de retomada devem convergir para esses mesmos resultados
sem perda, duplicação ou reordenação. Partes confirmadas mantêm seus bytes. Partes
regeneradas não precisam ser binariamente idênticas entre execuções independentes;
seus recibos devem conferir com os arquivos efetivamente escritos. Nomes,
intervalos, identidade e decisões de recuperação são determinísticos.

Fault injection usa hooks privados identificados por operação e índice da parte,
adaptadores `Write` limitados por bytes e subprocessos que encerram sem executar
destructors Rust. Não há flags ou variáveis de ambiente de falha na CLI de produção.
Não se usa sleep para estimar o momento da interrupção.

| Grupo | Fronteiras exercitadas |
|---|---|
| Bootstrap | Criação do diretório; escrita e publicação do descritor. |
| Parte | Antes de staging; escrita após 1 byte, no meio e antes do último byte. |
| Finalização | Antes/depois do footer e de `sync_all`. |
| Publicação | Antes/depois do rename da parte e do recibo. |
| Recibo | Criação, escrita parcial, sync e publicação. |
| Transição | Commit confirmado antes de avançar para a próxima parte. |
| Conclusão | Criação, escrita e publicação do marcador. |
| Limpeza | Antes/depois de remover órfão ou staging. |
| I/O | Erros de escrita, finalização, sync e publicação. |

A matriz cobre partes inicial, intermediária e final das configurações de aceite,
inclusive repetição de falhas durante Resume. Rename em andamento é representado
pelos estados observáveis antes/depois da publicação; arquivo final parcialmente
publicado não é um resultado normal do filesystem suportado.

Testes adversariais cobrem alterações de entrada de mesmo tamanho, caminhos
alternativos, diferenças físicas de layout com mesmo schema lógico, metadados
Arrow e ordem dos mapas, batch incompatível, corrupção e ausência de confirmados,
lacunas, recibos inválidos, conclusão prematura, órfãos completos/truncados,
nomes desconhecidos, links, capacidade inválida, FILLER e entrada truncada.
Concorrência no mesmo destino deve falhar por lock; interrupção deve liberá-lo.
O caso de diagnóstico retomado do registro 2 preserva byte absoluto 83 e byte
relativo ao batch 13, com a entrada inválida presente desde o fingerprint inicial.

Gates obrigatórios para declarar o milestone concluído:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo test --doc
```

Todos os testes M0–M3 permanecem obrigatórios. A especificação não é evidência de
aprovação dos gates; resultados de execução devem acompanhar o relatório de entrega.

## Dependências e limitações

M4 acrescenta `serde` com derive, `serde_json` e `sha2` 0.10.9. Desserialização
tipada substitui um parser próprio de estado externo; SHA-256 incremental usa
biblioteca estabelecida em vez de implementação manual. Arrow/Parquet continuam
na série 53. Lock e filesystem usam a biblioteca padrão.

Não há proteção PQC ou criptografia de payload, assinatura, cloud/object storage,
async/Tokio, workers, filas, coordenação distribuída, retry, telemetria operacional,
benchmark framework, campanha ampliada de fuzzing, compressão configurável,
compactação/fusão de partes, expansão COBOL, registros variáveis, UI ou database.
M4 não corrige cosmética da CLI M3 nem configurações redundantes sem impacto no
contrato. Não há migração de manifest, recuperação automática de confirmado
corrompido ou rollback de commits anteriores.

A entrada deve permanecer imutável durante cada invocação: o fingerprint detecta
diferenças entre invocações, mas não cria snapshot. Não há promessa de durabilidade
contra perda de energia, interoperabilidade de recuperação com outros profiles,
desempenho específico ou prontidão para produção. Outros filesystems exigem
validação própria antes de receber uma garantia de recuperação.
