# Subconjunto COBOL Copybook v0.1

## Objetivo

Este documento é o contrato fechado do compilador de copybook do M1. Ele define exatamente o que é aceito, como os comprimentos físicos são calculados e como campos são mapeados para Arrow. O compilador não tenta recuperar, inferir ou ignorar extensões fora deste subconjunto.

Uma entrada fora do contrato deve retornar um erro explícito com linha, coluna e causa. Entrada inválida nunca deve causar panic.

## Formato de origem

Somente **COBOL fixed-format** é aceito:

O arquivo-fonte deve usar ASCII imprimível, além das quebras de linha LF ou CRLF. Tabs e outros controles ASCII são rejeitados porque tornam a posição das colunas ambígua; caracteres não ASCII também são rejeitados com diagnóstico explícito.

| Colunas | Regra |
|---|---|
| 1–6 | Área de sequência; ignorada |
| 7 | Indicador de linha |
| 8–72 | Área de código |
| 73 em diante | Ignorada |

Na coluna 7:

- espaço indica uma linha normal;
- `*` ou `/` indica uma linha inteira de comentário;
- `-` (continuação), `D` (debug) e qualquer outro indicador são rejeitados explicitamente.

O comentário inline `*>` é permitido dentro da área de código e descarta o restante daquela linha. Linhas vazias e linhas de comentário não produzem tokens. Uma declaração pode ocupar mais de uma linha normal; o ponto final encerra a entry COBOL.

Continuação pela coluna 7 não faz parte do subset. Quebrar uma declaração entre linhas normais, em fronteiras de tokens, não equivale ao indicador de continuação e é permitido até seu ponto final.

## Estrutura aceita

- Exatamente um root de nível `01`.
- Entradas subordinadas nos níveis `02` a `49`.
- Grupos sem cláusula `PIC`/`PICTURE`.
- Campos elementares com cláusula `PIC` ou `PICTURE`, opcionalmente seguida por `IS`.
- `FILLER` somente como campo elementar.
- Ordem hierárquica determinada pelos números de nível.
- Cada grupo deve possuir ao menos uma entrada subordinada que leve a um campo elementar.

Grupos não ocupam bytes independentemente; seu tamanho é a soma dos campos elementares descendentes. Campos elementares são posicionados em ordem de declaração.

Keywords não distinguem maiúsculas de minúsculas. Um data-name começa com uma letra ASCII, pode continuar com letras, dígitos ou hífens e não pode terminar em hífen. A AST preserva sua grafia de origem; o layout compilado normaliza nomes para maiúsculas. O v0.1 aplica apenas essa validação lexical: não mantém uma lista de palavras reservadas COBOL nem impõe o limite tradicional de 30 caracteres.

## Formas de `PIC`

As únicas pictures aceitas são:

```text
X
X(n)
9
9(n)
9(n)V9(m)
S9
S9(n)
S9(n)V9(m)
```

Regras:

- `n` e `m` são inteiros positivos;
- o comprimento de um campo `PIC X(n)` não pode exceder `2.147.483.647` bytes, limite imposto pelos offsets de Arrow `Utf8`;
- precisão é o total de dígitos numéricos, antes e depois de `V`;
- escala é zero sem `V`, ou `m` em `V9(m)`;
- precisão numérica deve estar entre 1 e 18, inclusive;
- `V` representa ponto decimal implícito e não ocupa byte;
- prefixo `S` é aceito somente com COMP/BINARY ou COMP-3/PACKED-DECIMAL;
- pictures alfanuméricas `X...` não aceitam `S`, `V` ou USAGE numérico.

Não são aceitos símbolos de edição ou outras categorias de picture, incluindo `P`, `Z` e `A`.

## Cláusula `USAGE`

As formas aceitas são:

```text
DISPLAY
USAGE DISPLAY
USAGE IS DISPLAY

COMP
COMP-4
BINARY
USAGE COMP
USAGE IS COMP
USAGE COMP-4
USAGE IS COMP-4
USAGE BINARY
USAGE IS BINARY

COMP-3
PACKED-DECIMAL
USAGE COMP-3
USAGE IS COMP-3
USAGE PACKED-DECIMAL
USAGE IS PACKED-DECIMAL
```

Ausência de `USAGE` significa DISPLAY. `COMP`, `COMP-4` e `BINARY` são equivalentes dentro deste subset. `COMP-3` e `PACKED-DECIMAL` também são equivalentes.

`PIC X...` aceita apenas DISPLAY implícito ou explícito. `PIC 9...` sem `S` aceita qualquer um dos usos suportados. As variantes com `S` exigem COMP/BINARY ou COMP-3/PACKED-DECIMAL; signed DISPLAY e overpunch estão fora do escopo.

A forma opcional `USAGE [IS]` não torna outras palavras válidas: qualquer usage diferente dos listados deve falhar.

## Comprimentos físicos

### Texto DISPLAY

```text
PIC X       -> 1 byte
PIC X(n)    -> n bytes
```

### Numérico DISPLAY

Um dígito ocupa um byte. `V` não ocupa byte:

```text
PIC 9(n)          -> n bytes
PIC 9(n)V9(m)     -> n + m bytes
```

### COMP/BINARY

O tamanho segue a representação física IBM adotada para este subset, em função da precisão decimal total:

| Precisão | Byte length |
|---:|---:|
| 1–4 dígitos | 2 bytes |
| 5–9 dígitos | 4 bytes |
| 10–18 dígitos | 8 bytes |

O valor é big-endian. O tamanho não deve ser obtido ingenuamente usando o número de caracteres do `PIC` nem uma regra genérica diferente desta tabela.

### COMP-3/PACKED-DECIMAL

Para `d` dígitos de precisão:

```text
byte_length = (d + 2) / 2
```

A divisão é inteira. O nibble adicional é reservado ao sinal da representação packed decimal. A presença de `S` determina a signedness lógica registrada no layout.

## Encoding físico e mapeamento Arrow

O layout compilado resolve completamente encoding físico, signedness, precision, scale e tipo lógico:

| Declaração | Encoding físico | Tipo lógico Arrow |
|---|---|---|
| `PIC X...` DISPLAY | texto EBCDIC | `Utf8` |
| `PIC 9...` DISPLAY, escala 0 | numérico DISPLAY EBCDIC | `Int64` |
| `PIC 9...V9...` DISPLAY | numérico DISPLAY EBCDIC | `Decimal128(precision, scale)` |
| `PIC [S]9...` COMP/COMP-4/BINARY, escala 0 | inteiro binário big-endian | `Int64` |
| `PIC [S]9...V9...` COMP/COMP-4/BINARY | inteiro binário big-endian com escala implícita | `Decimal128(precision, scale)` |
| `PIC [S]9...` COMP-3/PACKED-DECIMAL | packed decimal | `Decimal128(precision, scale)` |

COMP-3 sempre mapeia para `Decimal128`, inclusive com escala zero. O decoder M2 usa CP037 para EBCDIC; o compilador M1 continua responsável apenas pelo layout. As políticas de bytes, sinais, preenchimento e limites numéricos estão no [contrato de decoding M2](DECODING.md), sem ampliação da sintaxe deste subset.

## Offsets, grupos e `FILLER`

- O primeiro campo elementar começa no offset zero.
- Cada campo elementar seguinte começa no fim do anterior.
- `record_length` é a soma verificada dos byte lengths de todos os campos elementares.
- `record_length` não pode exceder `isize::MAX`, pois o registro precisa ser representável por um slice Rust no decoder futuro.
- Grupos refletem hierarquia, mas não acrescentam bytes.
- Um `FILLER` elementar participa integralmente do layout físico, dos offsets e do `record_length`.
- Grupos e `FILLER` não aparecem no Arrow Schema.
- Campos não `FILLER` recebem um path em maiúsculas, qualificado pelo root e por todos os grupos ancestrais; esse path é também o nome do campo Arrow.
- Paths qualificados duplicados são rejeitados explicitamente.

## Construções rejeitadas

Devem produzir erro explícito:

- `OCCURS` e `OCCURS DEPENDING ON`;
- `REDEFINES`;
- `RENAMES`;
- níveis `66`, `77` e `88`;
- `COPY` e `REPLACING`;
- `COMP-1`, `COMP-2` e `COMP-5`;
- DISPLAY assinado, overpunch e cláusula `SIGN`;
- `SYNC`/`SYNCHRONIZED`;
- `JUSTIFIED`/`JUST`;
- `BLANK WHEN ZERO`;
- `VALUE`;
- `NATIONAL` e `USAGE NATIONAL`;
- pictures com `P`, `Z`, `A` ou edição;
- indicador de continuação `-` na coluna 7;
- registros variáveis, RDW/BDW;
- múltiplos layouts de registro no mesmo copybook;
- qualquer cláusula, token residual ou usage não listado neste documento.

Rejeição explícita é parte do contrato: o parser não pode aceitar apenas o prefixo conhecido de uma declaração e descartar o restante.

## Exemplos

Copybook válido:

```cobol
       01 CUSTOMER-RECORD.
          05 CUSTOMER-ID       PIC 9(9) COMP.
          05 CUSTOMER-NAME     PIC X(20).
          05 FILLER            PIC X(3).
          05 ACCOUNT-BALANCE   PIC S9(11)V9(2) COMP-3.
```

Layout físico esperado:

| Campo | Offset | Bytes | Arrow |
|---|---:|---:|---|
| `CUSTOMER-ID` | 0 | 4 | `Int64` |
| `CUSTOMER-NAME` | 4 | 20 | `Utf8` |
| `FILLER` | 24 | 3 | omitido |
| `ACCOUNT-BALANCE` | 27 | 7 | `Decimal128(13, 2)` |

`record_length = 34` bytes. O Arrow Schema contém três campos, sem `FILLER`.

Copybook inválido:

```cobol
       01 CUSTOMER-RECORD.
          05 ITEMS OCCURS 10 TIMES.
             10 ITEM-CODE PIC X(4).
```

O compilador deve apontar `OCCURS` na sua linha e coluna como cláusula não suportada; não pode gerar um layout parcial.
