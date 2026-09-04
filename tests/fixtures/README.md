# Fixtures M2

## CP037.TXT

Tabela pública CP037 → Unicode, versão 2.00 de 24/04/1996, mantida integralmente
com o cabeçalho original. Origem:
https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/EBCDIC/CP037.TXT

SHA-256: `794ec1593c5bc95c2df9efb388ba8943b770db163a1076002cde647a268566b1`.
O teste lê os pares numéricos da referência e compara todos os 256 caracteres
com o codec. Todos os bytes CP037 têm mapeamento, inclusive controles.

## sample_fixed.bin

105 bytes: três registros de 35 bytes para o copybook M1 `sample_fixed.cpy`.
Fixture artesanal do projeto, sob Apache-2.0, sem dados pessoais reais; não é
uma captura de mainframe. Os bytes abaixo foram especificados diretamente,
sem usar um encoder correspondente aos codecs implementados. Os resultados
esperados do teste são constantes independentes do decoder e do compilador.

SHA-256: `bc5083614c9c50322a78ea30b909fabb28d63a22f0d4bda87f77dfd49e47fb73`.

Cada linha é um registro. Os grupos separados por `|` seguem os campos:
texto (offset 0), FILLER (10), DISPLAY inteiro (12), DISPLAY decimal (16),
COMP assinado (23), BINARY decimal (25), COMP-3 assinado (29), FILLER (34).

```text
C1 D3 C9 C3 C5 40 40 40 40 40 | 00 FF | F0 F0 F4 F2 | F0 F0 F1 F2 F3 F4 F5 | FF 85 | 00 01 E2 40 | 12 34 56 78 9C | AA
D1 96 A2 51 40 40 40 40 40 40 | AB CD | F9 F9 F9 F9 | F9 F9 F9 F9 F9 F9 F9 | 27 0F | 00 98 96 7F | 00 00 00 12 3D | 00
00 15 25 9F BA BB 40 40 40 40 | 01 02 | F0 F0 F0 F0 | F0 F0 F0 F0 F0 F0 F0 | 00 00 | 00 00 00 00 | 00 00 00 00 0D | FF
```

| Coluna | Tipo | Registro 0 | Registro 1 | Registro 2 |
|---|---|---|---|---|
| CUSTOMER-NAME | Utf8 | `ALICE` + 5 espaços | `José` + 6 espaços | `\0\u{85}\n¤[]` + 4 espaços |
| ACCOUNT-NUMBER | Int64 | 42 | 9999 | 0 |
| INTEREST-RATE | Decimal128(7,2), inteiro armazenado | 12345 | 9999999 | 0 |
| BALANCE-BIN | Int64 | -123 | 9999 | 0 |
| RATE-BIN | Decimal128(7,2), inteiro armazenado | 123456 | 9999999 | 0 |
| AMOUNT-PACKED | Decimal128(9,2), inteiro armazenado | 123456789 | -123 | 0 |

Os nomes Arrow recebem o prefixo `SAMPLE-RECORD.HEADER-GROUP.` e nenhuma coluna
aceita null. Os inteiros armazenados dos decimais têm escala implícita 2:
por exemplo, 12345 representa 123,45. O terceiro COMP-3 codifica zero negativo,
normalizado para zero. FILLER não gera colunas nem validação de conteúdo.

Referências independentes das representações: a tabela Unicode acima para
CP037 e a documentação IBM de sinais packed decimal:
https://www.ibm.com/docs/en/cobol-linux-x86/1.2.0?topic=arithmetic-sign-representation-zoned-packed-decimal-data
