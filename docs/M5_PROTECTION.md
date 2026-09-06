# M5 — Proteção de artefatos

**Status:** CONGELADO — contrato de G0 para implementação do M5  
**Plataforma de publicação:** Windows/MSVC em volume NTFS local  
**Regra de mudança:** qualquer alteração normativa exige reabertura explícita do G0

## 1. Objetivo e critério de aceitação

O M5 adiciona uma camada opcional e autônoma de proteção de arquivos já produzidos pelo
M2C. A suíte usa ML-KEM-768 para estabelecimento de chave, HKDF-SHA-256 para derivação
da chave de conteúdo e AES-256-GCM em enquadramento STREAM-BE32 para confidencialidade e
integridade do payload e do cabeçalho.

O M5 é aceito quando:

1. um arquivo arbitrário, inclusive vazio, protegido para uma chave pública válida pode
   ser recuperado byte a byte somente com a chave secreta correspondente;
2. alteração do cabeçalho, ciphertext, tags, ordem, presença ou truncamento de frames é
   rejeitada de forma fechada, sem publicar plaintext parcial;
3. chave errada, formato inválido, limite excedido, entropia indisponível e estado de
   publicação inválido produzem erros tipados e não causam panic;
4. toda saída é publicada sem sobrescrever caminho existente e sem depender de
   <code>exists()</code> + <code>rename()</code>;
5. nenhuma operação M5 grava em namespace gerenciado pelo M4;
6. os limites formais e invariantes deste documento são demonstrados por testes
   determinísticos, independentes e adversariais;
7. M0–M4 continuam compatíveis e todas as verificações do G4 passam.

## 2. Escopo

### 2.1 Incluído

- feature Cargo opcional <code>pqc</code>;
- uma suíte fechada e versionada;
- geração local de um par de chaves ML-KEM-768;
- proteção de um arquivo para um único destinatário;
- desproteção de um envelope para um arquivo;
- processamento síncrono e de memória limitada;
- validação estrita de formato, tamanhos e aritmética;
- publicação atômica no-clobber no contrato Windows/MSVC + NTFS local;
- comandos CLI mínimos para gerar chaves, proteger e desproteger;
- erros tipados e testes independentes/adversariais.

O M5 pode **ler** um artefato M4 comprometido. Toda saída, inclusive staging, deve ficar
fora do namespace gerenciado pelo M4.

### 2.2 Excluído

- qualquer alteração ao protocolo, manifesto, receipts, recuperação ou layout do M4;
- escrita de envelopes pelo pipeline M4;
- proteção de colunas, Parquet Modular Encryption ou alteração do schema Arrow;
- múltiplos destinatários, rewrap, rotação automática ou descoberta de chaves;
- assinatura, ML-DSA, PKI ou autenticação de identidade do destinatário;
- proteção híbrida adicional, como X25519;
- KMS, HSM, nuvem, cofres de segredo ou armazenamento remoto;
- senha, criptografia ou selagem de chave secreta em repouso;
- async, Tokio, paralelismo, serviço, UI ou telemetria;
- suporte geral a POSIX, compartilhamentos de rede, FAT/exFAT/ReFS ou outros
  filesystems no M5;
- garantias contra comprometimento do processo, swap/pagefile, core dump, acesso
  administrativo ou recuperação forense;
- primitivas criptográficas implementadas pelo projeto.

## 3. Arquitetura e integração

~~~text
copybook + dataset -> pipeline M0–M4 -> arquivo comprometido
                                              |
                                              v
                                  protect -> envelope M5
                                  unprotect <- envelope M5
~~~

- Parser, layout compilado, codecs, batching, Parquet e recuperação M4 não conhecem o
  formato M5.
- O módulo M5 recebe caminhos de entrada e saída; não recebe nem modifica estado M4.
- Uma entrada somente-leitura pode estar em um root M4 comprometido. Destinos e
  temporários não podem estar nele.
- A API pública contém somente geração de chaves, <code>protect_file</code> e
  <code>unprotect_file</code>. Seleção arbitrária de algoritmos, RNG, nonce ou salt não
  é pública.
- Sem a feature <code>pqc</code>, nenhum comando/API criptográfica é compilado e M0–M4
  mantêm o comportamento atual.

## 4. Suíte criptográfica fechada

Identificador:

~~~text
M2C-M5-MLKEM768-HKDFSHA256-AES256GCM-STREAMBE32-1M-v1
~~~

O <code>suite_id</code> binário é 1. Não há negociação, downgrade ou fallback.

| Componente | Valor |
|---|---|
| KEM | ML-KEM-768 conforme FIPS 203 |
| Chave pública | 1184 bytes |
| Chave secreta | 2400 bytes |
| Ciphertext KEM | 1088 bytes |
| Segredo compartilhado | 32 bytes |
| KDF | HKDF-SHA-256 |
| AEAD | AES-256-GCM, chave de 32 bytes, nonce de 12 bytes, tag de 16 bytes |
| Enquadramento | STREAM-BE32 |
| Chunk plaintext | C = 2^20 bytes |
| Prefixo de nonce STREAM | 7 bytes aleatórios |

O <code>info</code> exato do HKDF é a sequência ASCII:

~~~text
M2C-M5-SUITE-0001-CONTENT-KEY
~~~

O salt HKDF tem 32 bytes aleatórios e fica no cabeçalho. A saída de 32 bytes do HKDF é
a chave AES-256-GCM. Versão ou suíte desconhecida é rejeitada.

## 5. Formatos de chave v1

Inteiros são unsigned e big-endian. Os arquivos são binários, sem trailing bytes,
metadata, extensão ou checksum.

Chave pública, total de 1200 bytes:

| Offset | Tamanho | Campo | Valor |
|---:|---:|---|---|
| 0 | 8 | <code>magic</code> | ASCII M2CM5PUB |
| 8 | 2 | <code>version</code> | 1 |
| 10 | 2 | <code>algorithm_id</code> | 1 = ML-KEM-768 |
| 12 | 4 | <code>payload_length</code> | 1184 |
| 16 | 1184 | <code>payload</code> | chave pública ML-KEM-768 |

Chave secreta, total de 2416 bytes:

| Offset | Tamanho | Campo | Valor |
|---:|---:|---|---|
| 0 | 8 | <code>magic</code> | ASCII M2CM5SEC |
| 8 | 2 | <code>version</code> | 1 |
| 10 | 2 | <code>algorithm_id</code> | 1 = ML-KEM-768 |
| 12 | 4 | <code>payload_length</code> | 2400 |
| 16 | 2400 | <code>payload</code> | chave secreta ML-KEM-768 |

Magic, versão, algoritmo, comprimento, tamanho real e codificação da chave são
validados antes do uso. O fingerprint do destinatário é calculado somente sobre os
1184 bytes do payload público, não sobre o cabeçalho do arquivo.

## 6. Envelope v1

Inteiros são unsigned e big-endian. O cabeçalho tem exatamente 1179 bytes:

| Offset | Tamanho | Campo | Valor/semântica |
|---:|---:|---|---|
| 0 | 8 | <code>magic</code> | ASCII M2CM5ENC |
| 8 | 2 | <code>version</code> | 1 |
| 10 | 2 | <code>suite_id</code> | 1 |
| 12 | 8 | <code>plaintext_length</code> | tamanho total original |
| 20 | 32 | <code>recipient_public_key_sha256</code> | SHA-256 do payload público exato |
| 52 | 32 | <code>hkdf_salt</code> | salt aleatório |
| 84 | 7 | <code>stream_nonce_prefix</code> | prefixo aleatório do nonce STREAM |
| 91 | 1088 | <code>kem_ciphertext</code> | encapsulamento ML-KEM-768 |

Todo o cabeçalho de 1179 bytes, sem transformação, é AAD de **cada** frame AEAD. Não
existem campos opcionais, extensão, padding ou metadados externos autenticados no v1.

<code>recipient_public_key_sha256</code> é somente um fingerprint/identificador da
representação binária exata da chave pública. Ele não autentica pessoa, origem ou posse
da chave, não é assinatura e não pode servir sozinho como decisão de autorização. Sua
integridade é fornecida pelas tags AES-GCM porque o cabeçalho inteiro compõe o AAD.
Antes da validação de uma tag, o campo é dado externo não confiável.

O corpo é a concatenação de frames STREAM-BE32. Cada frame contém no máximo C bytes de
plaintext e acrescenta tag de 16 bytes. Há exatamente um frame final. Plaintext vazio
usa um frame final vazio mais sua tag.

## 7. Limite exato do STREAM-BE32

O contador unsigned de 32 bits admite 0..=u32::MAX. Frames não finais usam
<code>encrypt_next</code>; o último valor é reservado a <code>encrypt_last</code>.
Logo:

~~~text
F_MAX = u32::MAX + 1 = 2^32 frames
~~~

Para tamanho plaintext N:

~~~text
F(0)   = 1
F(N>0) = ceil(N / C) = 1 + ((N - 1) / C)
C      = 2^20 bytes
~~~

Todo cálculo usa aritmética verificada; wrap, saturação e conversão truncada são
proibidos. O maior plaintext aceito é:

~~~text
N_MAX = C * F_MAX
      = 2^20 * 2^32
      = 2^52
      = 4.503.599.627.370.496 bytes
~~~

Em N_MAX, há 2^32 - 1 frames não finais nas posições 0..=u32::MAX-1 e um frame final
na posição u32::MAX. N_MAX + 1 é rejeitado antes de qualquer staging.

O tamanho esperado do envelope, também calculado com operações verificadas, é:

~~~text
E(N) = 1179 + N + (16 * F(N))
~~~

Na desproteção, o tamanho real deve ser exatamente E(plaintext_length). Diferença para
mais ou para menos é erro antes da publicação. Nenhum <code>usize</code> ou buffer é
derivado de tamanho externo sem conversão verificada e limite local explícito.

Testes de fronteira, sem alocar o payload:

- F(0) = 1;
- F(1) = 1;
- F(C) = 1;
- F(C + 1) = 2;
- F(C * 2^32) = 2^32;
- F(C * 2^32 + 1) retorna <code>InputTooLarge</code>;
- overflow em cada operação/conversão relevante é rejeitado.

## 8. Entropia e determinismo

Em produção, toda entropia de geração de chave, encapsulamento KEM, salt HKDF e prefixo
de nonce vem exclusivamente do gerador aleatório do sistema operacional. Falha de
entropia é erro tipado fatal, sem fallback.

- API pública, CLI, arquivo de configuração e variável de ambiente não aceitam RNG,
  seed, nonce, salt ou bytes de entropia fornecidos pelo chamador.
- Injeção determinística só pode existir em código privado sob <code>#[cfg(test)]</code>,
  ausente do artefato e da API de produção.
- Fixtures determinísticas podem ser geradas externamente e versionadas como bytes de
  teste; não criam um caminho determinístico no runtime.
- Duas proteções do mesmo input/chave devem normalmente gerar envelopes diferentes.
- A obtenção de todos os bytes aleatórios ocorre antes da publicação; falha fecha a
  operação.

## 9. Chave secreta e zeroização

### 9.1 Criação e permissões

A geração exige diretório de destino inexistente. Diretório e arquivos usam criação
exclusiva; nenhum caminho existente é truncado, removido ou sobrescrito. O staging da
chave secreta usa <code>create_new(true)</code> e sua publicação final usa o protocolo
no-clobber da seção 11.

Permissões restritivas são aplicadas em melhor esforço ao diretório novo, staging
secreto e arquivo publicado. No Windows, a implementação tenta aplicar DACL que limite
o acesso ao usuário atual e às identidades mínimas necessárias do sistema. Falha de
restrição gera aviso estruturado e visível na CLI; não é ocultada.

Privilégios administrativos, herança, software de backup, cache, pagefile e alteração
externa de ACL impedem garantia absoluta. Senha, DPAPI, TPM, KMS/HSM ou outra proteção
da chave secreta em repouso permanecem fora de escopo.

### 9.2 Zeroização

Zeroização é mitigação de melhor esforço, não garantia de eliminação física. Limpeza
RAII nos caminhos normais e de erro se aplica somente aos buffers secretos possuídos
diretamente pelo M2C:

- bytes de chave secreta lidos ou gerados pelo M2C;
- cópias M2C do segredo compartilhado KEM;
- material intermediário e chave de conteúdo derivados pelo M2C;
- buffers plaintext temporários da desproteção pertencentes ao M2C.

O projeto não afirma zeroização de objetos internos das dependências, cópias do
compilador, registradores, stack, allocator, buffers do sistema, cache, pagefile ou
core dump, salvo se uma garantia específica for demonstrada e documentada. Suporte
aparente de descarte/zeroização em um tipo externo não autoriza alegação mais ampla.

## 10. Memória limitada

- Cabeçalho é lido/escrito uma vez.
- Payload é processado sequencialmente em buffer de no máximo C bytes, além de overhead
  pequeno e constante.
- Arquivo, plaintext ou ciphertext inteiro nunca é carregado em memória.
- Tamanho de entrada é validado antes de criar staging.
- EOF prematuro, bytes adicionais ou mudança de tamanho observável falham fechados.
- Desproteção escreve somente no staging. Plaintext final não fica visível antes de
  todos os frames serem autenticados e o tamanho total ser validado.

Mutação concorrente deliberada do arquivo de entrada está fora do modelo de suporte,
coerentemente com o contrato local do M4.

## 11. Publicação no-clobber

### 11.1 Contrato de plataforma

O M5 v1 publica somente em Windows/MSVC, em diretório local de volume NTFS, com staging
e destino no mesmo diretório. Outro alvo retorna
<code>UnsupportedPublicationPlatform</code> antes de gravar bytes. Não há fallback por
<code>rename</code>, <code>copy</code>, remoção do destino ou sequência
<code>exists()</code> + operação destrutiva.

Uma plataforma futura só pode ser habilitada se oferecer criação atômica do nome final
**se e somente se ausente**. Essa extensão exige novo G0/ADR.

### 11.2 Protocolo

Para cada saída de proteção/desproteção:

1. validar plataforma, filesystem, caminho resolvido, ausência de reparse point/symlink
   no caminho de escrita e isolamento M4;
2. exigir diretório pai existente; o M5 não cria sua árvore;
3. criar staging imprevisível no mesmo diretório com <code>create_new(true)</code>,
   repetindo com entropia nova apenas em colisão;
4. escrever/finalizar todo o conteúdo, executar <code>sync_all</code> e fechar o
   arquivo;
5. repetir imediatamente a validação de isolamento M4;
6. criar atomicamente o nome final como hard link NTFS para o staging; essa é a
   operação de commit e falha se o nome final já existir;
7. após commit, remover somente o nome de staging, em melhor esforço.

<code>exists()</code> pode melhorar o diagnóstico, mas nunca fundamenta a propriedade
no-clobber. A correção depende da criação atômica do hard link com falha quando o
destino existe. Se hard links não forem suportados, o volume não for NTFS, os nomes não
estiverem no mesmo volume ou a semântica não puder ser confirmada, a operação falha
antes do commit.

### 11.3 Resultados

- Antes do commit, erro remove o staging próprio em melhor esforço e não cria o final.
- Se o destino já existir ou surgir numa corrida, retorna
  <code>OutputAlreadyExists</code>; o objeto existente permanece intocado.
- Após a criação do hard link, o final está comprometido e não é removido por falha de
  limpeza.
- Falha ao remover staging após commit retorna sucesso explícito
  <code>PublishedWithStagingResidue(path)</code>; a CLI emite aviso. Não retorna erro
  ambíguo.
- Sucesso sem resíduo retorna <code>Published</code>.
- <code>sync_all</code> reduz risco de perda dos dados; não se promete durabilidade do
  diretório contra queda de energia.

Stagings têm prefixo reservado e só são removidos pela invocação que os criou. Não há
varredura/limpeza automática de resíduos de outras execuções.

Geração de chaves aplica o mesmo protocolo a cada arquivo e criação exclusiva ao
diretório. Queda pode deixar diretório parcial; ele é inválido, nunca é adotado ou
sobrescrito por outra execução e exige tratamento manual.

## 12. Isolamento do namespace M4

Proteção, desproteção e geração de chaves nunca criam diretório, staging, hard link ou
arquivo final em namespace gerenciado pelo M4.

Um diretório e seus descendentes são considerados M4-managed quando ele contém qualquer
marcador reservado inequívoco (<code>.m4.lock</code>,
<code>.manifest.json.tmp</code> ou <code>.complete.json.tmp</code>) ou quando:

- <code>manifest.json</code> ou <code>complete.json</code> declara formato/versão M4
  reconhecível, mesmo que o restante esteja inválido; ou
- existem simultaneamente <code>parts/</code> e <code>commits/</code> e pelo menos um
  dos nomes de controle <code>manifest.json</code> ou <code>complete.json</code>.

A validação percorre o pai resolvido e seus ancestrais até a raiz do volume. Reparse
points/symlinks no caminho de escrita são rejeitados. A validação ocorre antes do
staging e imediatamente antes do commit. Marcador M4 surgido entre as verificações
aborta a operação, remove o staging em melhor esforço e não publica.

O contrato de concorrência M4 permanece: mutação externa deliberada fora dos pontos
verificáveis está fora do modelo. Estado não classificável com segurança é rejeitado,
nunca aceito por fallback.

Testes obrigatórios:

- rejeitar destinos no root, <code>parts/</code>, <code>commits/</code> e
  subdiretórios de root M4;
- rejeitar root ativo identificado apenas por <code>.m4.lock</code>;
- rejeitar root parcialmente corrompido com marcadores suficientes;
- impedir commit quando marcador surge antes dele;
- provar por snapshot que todos os arquivos M4 ficam byte a byte idênticos;
- permitir saída em diretório adjacente fora do root;
- provar que nenhum staging M5 é criado no namespace M4.

## 13. Falhas e estados inválidos

Erros públicos são tipados, preservam etapa/caminho relevante e não expõem segredo. No
mínimo, distinguem:

- plataforma/filesystem não suportado;
- destino existente;
- destino em namespace M4;
- reparse point/symlink ou caminho inseguro;
- I/O de entrada, staging, sincronização, commit e limpeza;
- entropia indisponível;
- magic, versão ou suíte inválida/desconhecida;
- chave com formato, algoritmo ou tamanho inválido;
- fingerprint incompatível;
- comprimento inválido, overflow ou input acima do limite;
- falha de autenticação/desproteção;
- truncamento, trailing bytes ou sequência de frames inválida.

Falhas criptográficas capazes de atuar como oracle são apresentadas externamente numa
categoria uniforme de falha de autenticação/desproteção. A CLI não diferencia chave
errada, encapsulamento inválido e tag manipulada.

Nunca se publica saída parcial. Dado malformado não pode causar panic, alocação
proporcional a tamanho não confiável ou slicing sem validação.

## 14. Dependências

O M2C não implementa primitivas criptográficas. A implementação deve usar bibliotecas
estabelecidas, com versões fixadas no <code>Cargo.lock</code>, e confirmar antes do
código:

- ML-KEM-768 padronizado por FIPS 203, não Kyber pré-padronização;
- tamanhos e comportamento de decapsulação compatíveis com este contrato;
- suporte a Windows/MSVC sem runtime externo não documentado;
- vetores oficiais/independentes disponíveis para verificação.

Tipos de dependências ficam privados. A escolha concreta de crate/versão é decisão do
G1, não altera o wire format e não reabre o G0 se satisfizer integralmente este
contrato. Dependências adicionais só se justificam para ML-KEM, HKDF/SHA-256,
AES-GCM/STREAM-BE32, entropia do SO, zeroização de buffers próprios e APIs Windows
necessárias a NTFS/no-clobber/ACL.

## 15. Testes de aceitação e adversariais

- round-trip byte a byte para 0, 1, C-1, C, C+1, vários chunks e dataset representativo;
- duas proteções do mesmo input/chave resultam normalmente em envelopes diferentes e
  ambos recuperam corretamente;
- vetores oficiais/independentes de ML-KEM-768, HKDF-SHA-256 e AES-256-GCM;
- fixture de envelope gerada por oracle independente quando viável;
- bit flip em cada região do cabeçalho e amostras de frames/tags;
- chave errada, reordenação, duplicação, remoção e truncamento de frames;
- trailing bytes e <code>plaintext_length</code> inconsistente;
- falha em pontos injetáveis de I/O/entropia somente sob
  <code>#[cfg(test)]</code>;
- nenhuma falha publica plaintext/ciphertext parcial;
- corrida de destino preserva integralmente o vencedor;
- hard link indisponível, não NTFS, cross-volume e reparse point falham fechados;
- resíduo pós-commit retorna sucesso explícito;
- chave secreta tem criação exclusiva/no-overwrite e aviso testável de ACL;
- todos os limites da seção 7 e casos de isolamento da seção 12;
- corpus malformado não causa panic nem alocação não limitada.

O encoder M2C não pode ser o único oracle do decoder. Testes de publicação/ACL rodam em
Windows/MSVC sobre NTFS e só podem ser ignorados com motivo explícito quando o ambiente
não satisfizer esse contrato.

## 16. Gates

### G0 — Specification Gate

- Objetivo, formatos persistentes, suíte, limites, falhas, publicação e isolamento M4
  estão definidos neste documento.
- Ambiguidades conhecidas foram resolvidas pelo menor contrato coerente.
- Não há vazamento de escopo M6+.
- **Status: PASS.**

### G1 — Architecture Gate

- M5 permanece opcional e separado do pipeline/recovery M4.
- Recuperação, determinismo, memória limitada, integridade e erros de M0–M4 não são
  enfraquecidos.
- Crate ML-KEM e versões concretas devem ser aprovadas segundo a seção 14 antes do
  primeiro código criptográfico.

### G2 — Regression Gate

- Builds sem <code>pqc</code> preservam API e comportamento M0–M4.
- Todos os testes preexistentes continuam passando.
- Nenhum formato ou namespace M4 é alterado.

### G3 — Correctness Gate

- Invariantes das seções 4–13 são determinísticos e testáveis.
- Estado inválido e falha de autenticação fecham a operação.
- Não há overwrite, publicação parcial ou fallback ambíguo.

### G4 — Verification Gate

~~~text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test --doc
~~~

Todos os testes M5 das seções 7, 12 e 15 devem demonstrar o critério da seção 1.

### G5 — Close Gate

M5 só pode ser declarado completo quando:

- o critério de aceitação estiver demonstrado;
- nenhum achado BLOCKER ou IMPORTANT permanecer;
- regressões M0–M4 passarem;
- documentação, ajuda CLI e limitações refletirem o contrato final;
- não houver desvio arquitetural não aprovado.

## 17. Invariantes congelados

1. A suíte e os formatos v1 não são negociáveis nem extensíveis implicitamente.
2. O cabeçalho completo é AAD de todo frame.
3. <code>recipient_public_key_sha256</code> é identificador, não autenticação de
   identidade.
4. Todo tamanho, offset, contador e tamanho esperado usa aritmética verificada.
5. Produção não aceita entropia controlada pelo chamador.
6. Zeroização é melhor esforço e limitada a buffers secretos pertencentes ao M2C.
7. Plaintext/ciphertext final só aparece após validação completa e commit no-clobber.
8. Caminho existente nunca é removido, truncado ou substituído.
9. M5 nunca grava em namespace M4.
10. Estado inseguro, ambíguo ou não suportado falha fechado.

## 18. Questões não resolvidas

Não há questão de produto/arquitetura pendente para G0. A seleção concreta e revisão das
versões das dependências pertencem ao G1 e devem obedecer integralmente a este contrato,
sem alterar algoritmo, formato, entropia, limites ou semântica de falha.

## 19. Referências normativas

- NIST FIPS 203 — Module-Lattice-Based Key-Encapsulation Mechanism Standard.
- NIST SP 800-38D — Galois/Counter Mode (GCM) and GMAC.
- Documentação da implementação STREAM-BE32 selecionada; a reserva do último contador
  para o frame final deve ser confirmada pelos testes da seção 7.
