# Contrato de decoding M2

## API e fronteiras

`RecordDecoder::try_new(&CompiledCopybook)` valida o layout compilado uma vez e
o mantém emprestado imutavelmente. `decode_batch(&self, &[u8])` retorna
`Result<arrow_array::RecordBatch, DecodeError>`. Nenhuma chamada interpreta PIC,
tokens ou AST. As APIs e semânticas do M1 permanecem preservadas.

O batch contém registros completos e concatenados, sem delimitadores. O tamanho
deve ser múltiplo de `record_length`; um registro parcial provoca erro antes da
alocação dos builders. Entrada vazia é válida e preserva o schema com zero linhas.
Um layout contendo apenas FILLER produz zero colunas e a contagem correta de linhas.

O chamador controla o tamanho dos batches. O decoder é síncrono, não lê arquivos,
não particiona datasets e não retém registros ou builders entre chamadas. Não há
Parquet, CLI funcional, async, cloud ou criptografia no M2.

## Validação de layouts públicos

Os structs M1 têm campos públicos; portanto, não basta assumir que foram produzidos
pelo compilador. O construtor verifica nomes canônicos e paths qualificados únicos,
FILLER, comprimentos positivos, offsets contíguos, bounds, overflow, comprimento
total, signedness, precisão, escala, comprimentos físicos e tipos lógicos. Exige
precisão de 1–18 e escala de 0 até precisão menos 1, conforme o subset M1.

O schema deve conter exatamente as colunas não FILLER, na ordem física, com os
nomes e tipos do layout e sem nulabilidade. Metadados Arrow são preservados.
A validação também cobre a estrutura dos campos FILLER, mas seus bytes não são
decodificados ou validados como valores numéricos.

## Representações

| Encoding | Regra |
|---|---|
| CP037 | Mapeamento integral da tabela pública Unicode 2.00; todos os 256 bytes são definidos. Espaços iniciais/finais, controles e NUL são preservados. Não há trim, substituição ou normalização. |
| DISPLAY | Somente bytes EBCDIC `F0`–`F9`. Sinais, overpunch, espaços, ASCII e pontuação são rejeitados. |
| COMP/BINARY | Big-endian, 2/4/8 bytes segundo a precisão M1. Assinados usam complemento de dois; sem sinal usam magnitude unsigned. |
| COMP-3 | Um dígito decimal por nibble, exceto o último, reservado ao sinal. Precisão par exige nibble inicial zero. |

Em COMP-3 assinado, `A`, `C`, `E`, `F` indicam positivo e `B`, `D` negativo.
Campos sem sinal exigem `F`; outros sinais são rejeitados mesmo para zero.
Zero negativo assinado é normalizado para zero, pois Decimal128 armazena um inteiro.

Todos os valores numéricos devem satisfazer `-10^precision < valor < 10^precision`;
campos sem sinal são não negativos. Um binário que cabe fisicamente, mas excede a
precisão PIC, é rejeitado. Por exemplo, `PIC 9(4) COMP` rejeita `0x2710` (10000).
Não há truncamento, arredondamento ou emulação de opções TRUNC do compilador IBM.

Os decimais são inteiros i128 sem escala aplicada. Por exemplo, os dígitos `0012345`
com precisão 7 e escala 2 produzem `Decimal128(7,2)` com valor armazenado 12345.
DISPLAY e BINARY com escala zero produzem Int64; COMP-3 sempre produz Decimal128.
O código não utiliza ponto flutuante.

## Erros e memória

`DecodeError.kind` distingue layout inválido, comprimento de batch/campo, dígito
DISPLAY, dígito/sinal/preenchimento COMP-3, precisão excedida, capacidade e erro Arrow.
Falhas em valores carregam `DecodeContext`: índice do registro, path do campo,
offset no batch e `SourceSpan` original. Índices e offsets começam em zero;
linha e coluna do copybook começam em um. O offset aponta para o byte inválido
quando conhecido; erros de faixa/capacidade apontam para o início do campo.
Detalhes de nibble/dígito dentro de `kind` são relativos ao campo.

A primeira falha em ordem de registro/campo encerra a chamada. Não são retornados
batches parciais nem nulls substituindo dados inválidos. O erro Arrow original é
preservado como `Error::source`. Um novo batch pode ser decodificado normalmente
com a mesma instância após uma falha.

Os builders são alocados por coluna. Cálculos de capacidade consideram largura,
offset terminal, alinhamento e crescimento dos buffers Arrow. Em plataformas de
32 bits, o limite de texto também reserva espaço para a duplicação de capacidade
do builder e pode ser menor que `i32::MAX`. O tamanho UTF-8 real é calculado antes da
conversão, e a soma por coluna deve caber em `i32::MAX`; não basta verificar bytes
EBCDIC, pois alguns caracteres usam dois bytes UTF-8. O decoder retorna erro de
capacidade quando necessário; o chamador pode fornecer batches menores. Os testes
de overflow exercitam os cálculos sem alocar gigabytes. A memória adicional é
proporcional ao resultado Arrow, mais uma string temporária reutilizada por batch.

## Evidência e referências

- Fixture binária artesanal de 105 bytes, três registros de 35 bytes, para o
  copybook M1 existente; comparação exata com schema e valores constantes.
- Tabela CP037 pública completa, vetores numéricos explícitos e limites de 18
  dígitos; dados gerados para propriedades são oráculos complementares.
- Teste exaustivo dos 65536 padrões de dois bytes para BINARY signed/unsigned.
- Quatro propriedades, 256 casos cada, seed fixa `0x4D3243`, sem subprocessos:
  valores/escala, bytes arbitrários, particionamento e layouts públicos arbitrários.
- Casos adversariais de layout, schema, truncamento, sinais, capacidade e recuperação.

Origem, hexdump e hashes: [fixtures M2](../tests/fixtures/README.md).
Fontes: [CP037 Unicode](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/EBCDIC/CP037.TXT),
[sinais packed decimal IBM](https://www.ibm.com/docs/en/cobol-linux-x86/1.2.0?topic=arithmetic-sign-representation-zoned-packed-decimal-data).
