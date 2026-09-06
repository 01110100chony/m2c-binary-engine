# M5 Independent Review

## Verdict

**PASS WITH MINOR FINDINGS**

---

## 1. Executive Summary

A auditoria independente do milestone M5 (`pqc-mainframe-db`) examinou exaustivamente a especificação congelada (`docs/M5_PROTECTION.md`), arquitetura, implementação e testes. Foram executadas baterias de testes adversariais para falsificar garantias em criptografia (ML-KEM-768, HKDF-SHA-256, AES-256-GCM / STREAM-BE32), integridade do envelope, limites matemáticos ($2^{32}$ frames / $2^{52}$ bytes), parsing sob inputs malformados sem OOM ou panic, atomicidade de publicação no-clobber via hard link NTFS, isolamento rigoroso de namespace M4 e zeroização de material sensível. A baseline completa do G4 (`cargo fmt`, `cargo clippy -D warnings`, testes default, testes all-features e doctests) passou sem ressalvas em ambiente Windows/MSVC + NTFS.

**No BLOCKER or IMPORTANT findings were identified after independent adversarial review.**

---

## 2. Findings

| ID | Severity | Component | Finding | Evidence | Recommended action |
|---|---|---|---|---|---|
| GAP-01 | TEST GAP | [`src/protection/publication.rs`](../src/protection/publication.rs#L658-L683) | Teste de reparse point / symlink skipped em ambiente sem privilégio | `tests/protection.rs` e `publication.rs:670` pulam criação de symlink devido ao erro `1314` (`SeCreateSymbolicLinkPrivilege`). | Manter como limitação ambiental documentada; verificação estática comprovou que `FILE_ATTRIBUTE_REPARSE_POINT` é inspecionado em todos os ancestrais. |
| GAP-02 | TEST GAP | [`tests/fixtures/README.md`](../tests/fixtures/README.md#L51-L64) | Direcionalidade do teste de interoperabilidade externo | A fixture OpenSSL/Python prova que o decoder do M2C lê envelopes de terceiros, mas não executa script Python no CI para decodificar saídas do `protect_file`. | Em milestone futuro com tooling Python no pipeline de CI, adicionar teste bidirecional (M2C encoder $\rightarrow$ Python cryptography decoder). |
| DOC-01 | DOCUMENTATION | [`src/protection/operations.rs`](../src/protection/operations.rs#L258-L270) | Staging de plaintext residual após crash não autenticado | Se o processo sofrer interrupção forçada (`SIGKILL` ou queda de energia) durante a decodificação de múltiplos frames antes de um frame adulterado falhar, bytes em staging permanecem até exclusão manual. | Registrar explicitamente no threat model do README que staging parcial não comprometido depende do ciclo normal de processo para cleanup via `Drop`. |

---

## 3. Spec Traceability

| Requirement | Código responsável | Teste existente | Evidência | Status |
|---|---|---|---|---|
| **R01**: Round-trip exato de arquivo arbitrário, inclusive vazio ($N=0$) | [`src/protection/operations.rs`](../src/protection/operations.rs#L202-L310), [`src/protection/stream.rs`](../src/protection/stream.rs#L69-L219) | `tests/protection.rs:93`, `src/protection/stream.rs:330` | Plaintext vazio resulta em 1195 bytes (1179 header + 16 tag), recuperado byte a byte. | **VERIFIED** |
| **R02**: Adulteração de cabeçalho, ciphertext, tags ou sequência rejeitada sem publicar | [`src/protection/codec.rs`](../src/protection/codec.rs#L46-L98), [`src/protection/stream.rs`](../src/protection/stream.rs#L144-L219) | `tests/protection.rs:184`, `tests/protection.rs:237` | Bit flip em cada campo do header, swap, duplicação e truncamento falham fechado sem criar arquivo final. | **VERIFIED** |
| **R03**: Chave errada, formato inválido e entropia indisponível produzem erros tipados sem panic | [`src/protection/crypto.rs`](../src/protection/crypto.rs#L19-L82), [`src/protection/mod.rs`](../src/protection/mod.rs#L65-L123) | `tests/protection.rs:184`, `tests/protection.rs:324` | `RecipientFingerprintMismatch`, `InvalidMagic`, `UnsupportedVersion`, `EntropyUnavailable` retornados sem panic. | **VERIFIED** |
| **R04**: Publicação atômica no-clobber sem depender de `exists()` + `rename()` | [`src/protection/publication.rs`](../src/protection/publication.rs#L307-L380) | `src/protection/publication.rs:473`, `tests/protection.rs:184` | Commit via `fs::hard_link` atômico; corrida e destino existente preservam vencedor e retornam `OutputAlreadyExists`. | **VERIFIED** |
| **R05**: Nenhuma operação M5 grava em namespace M4 (root, parts, commits, descendentes) | [`src/protection/publication.rs`](../src/protection/publication.rs#L128-L150) | `src/protection/publication.rs:512`, `tests/protection.rs:265` | Marcadores inequívocos (`.m4.lock`, etc.) ou manifest/complete M4 rejeitam staging e commit; snapshot M4 idêntico. | **VERIFIED** |
| **R06**: Limites STREAM-BE32: $F(0)=1$, $F(N)=1+((N-1)/C)$, $F_{\max}=2^{32}$, $N_{\max}=2^{52}$ | [`src/protection/codec.rs`](../src/protection/codec.rs#L195-L232) | `src/protection/codec.rs:332`, harness adversarial | Limites matemáticos verificados com aritmética checada; $N_{\max}+1$ e $u64::\text{MAX}$ rejeitados sem alocação. | **VERIFIED** |
| **R07**: Entropia de produção exclusivamente do SO; determinismo apenas sob `#[cfg(test)]` | [`src/protection/crypto.rs`](../src/protection/crypto.rs#L19-L63) | Inspeção estática + `tests/protection.rs:93` | `getrandom::fill` via `sys_rng`; `keypair_from_test_seed` protegido por `#[cfg(test)] pub(super)`. | **VERIFIED** |
| **R08**: Formatos v1 exatos: chave pública (1200 B), chave secreta (2416 B), envelope (1179 B header) | [`src/protection/codec.rs`](../src/protection/codec.rs#L3-L44) | `src/protection/codec.rs:249`, `src/protection/codec.rs:310` | Offsets, magics (`M2CM5PUB`, `M2CM5SEC`, `M2CM5ENC`), tamanhos e big-endian comprovados. | **VERIFIED** |
| **R09**: Cabeçalho integral de 1179 bytes compõe o AAD de cada frame | [`src/protection/operations.rs`](../src/protection/operations.rs#L226-L242), [`src/protection/stream.rs`](../src/protection/stream.rs#L69-L142) | `tests/protection.rs:184`, harness adversarial | Header de 1179 B é repassado sem alteração para `encrypt_next`/`encrypt_last` e validado em cada frame. | **VERIFIED** |
| **R10**: Suíte criptográfica fechada (ID 1), sem negociação ou fallback | [`src/protection/codec.rs`](../src/protection/codec.rs#L5), [`src/protection/mod.rs`](../src/protection/mod.rs#L99-L101) | `tests/protection.rs:184` | Qualquer `version != 1` ou `suite_id != 1` retorna erro tipado imediato. | **VERIFIED** |
| **R11**: Memória limitada em streaming (buffer máx $C = 2^{20}$ bytes) | [`src/protection/stream.rs`](../src/protection/stream.rs#L92-L135), [`src/protection/operations.rs`](../src/protection/operations.rs#L228-L244) | `tests/protection.rs:93`, harness adversarial | Buffers alocados por chunk (1 MiB + 16 B tag); arquivo completo nunca é carregado em memória. | **VERIFIED** |
| **R12**: Zeroização de melhor esforço restrita a buffers próprios do M2C | [`src/protection/crypto.rs`](../src/protection/crypto.rs#L14-L140), [`src/protection/stream.rs`](../src/protection/stream.rs#L174) | Inspeção estática de data-flow | Seed, secret key, shared secret, AES key e plaintext decodificado protegidos por `Zeroizing`. | **VERIFIED** |
| **R13**: Restrição de permissões DACL Windows em melhor esforço | [`src/protection/windows.rs`](../src/protection/windows.rs#L147-L198), [`src/protection/publication.rs`](../src/protection/publication.rs#L415-L430) | `src/protection/publication.rs:602` | SDDL `D:P(A;;FA;;;SY)(A;;FA;;;{sid})`; falha gera `PermissionRestrictionFailed` estruturado na CLI. | **VERIFIED** |
| **R14**: Isolamento de feature Cargo: dependências ausentes do build padrão | [`Cargo.toml`](../Cargo.toml#L9-L19), [`src/lib.rs`](../src/lib.rs#L16-L17) | `cargo tree`, `cargo test --all-targets` | 0 crates M5 no grafo default; 117 testes default passam sem carregar código M5. | **VERIFIED** |

---

## 4. Cryptographic Review

### 4.1 ML-KEM-768
* **Implementação:** Usa o crate oficial `ml-kem = 0.3.2` padronizado conforme NIST FIPS 203 (não o rascunho anterior Kyber).
* **Parâmetros:** Chave pública de 1184 bytes, chave secreta de 2400 bytes (`to_expanded_bytes` / `from_expanded_bytes`), ciphertext de 1088 bytes e segredo compartilhado de 32 bytes.
* **Comportamento em ciphertext inválido:** Obedece rigorosamente ao FIPS 203 Section 7.3 com *implicit rejection* ($K' = \text{J}(z, c)$). Um ciphertext adulterado não causa panic nem erro imediato em `decapsulate`, produzindo um segredo compartilhado pseudo-aleatório divergente que acarreta falha determinística de autenticação no frame 0 do AES-GCM.
* **Interoperabilidade:** A fixture `tests/fixtures/m5_mlkem768_openssl.txt` demonstra compatibilidade comprovada com par de chaves gerado pelo OpenSSL 3.6.1 (`genpkey`/`pkeyutl -encap`).
* **Veredito:** **APROVADO**.

### 4.2 HKDF-SHA-256
* **Implementação:** Crate `hkdf = 0.12.4` com `Sha256`.
* **Salt & Context:** Salt de 32 bytes aleatórios extraídos do cabeçalho; IKM = segredo compartilhado de 32 bytes do ML-KEM.
* **Info:** String ASCII estrita `b"M2C-M5-SUITE-0001-CONTENT-KEY"` (30 bytes). Não há possibilidade de colisão ou ambiguidade de contexto com derivação de outras chaves no sistema.
* **Veredito:** **APROVADO**.

### 4.3 AES-256-GCM / STREAM-BE32
* **Implementação:** Crate `aes-gcm = 0.11.1` e `aead-stream = 0.6.0` (`EncryptorBE32`, `DecryptorBE32`).
* **Construção do Nonce:** 12 bytes = prefixo aleatório de 7 bytes ($56$ bits) + contador big-endian de 4 bytes ($0 \dots 2^{32}-1$) + flag de frame final de 1 byte (`0x00` para frames normais, `0x01` para o frame final).
* **Nonce Uniqueness:** Impossível ocorrer colisão de $(key, nonce)$:
  1. *No mesmo envelope:* O contador é estritamente crescente; a flag final distingue o último frame de qualquer intermediário no mesmo índice; o encryptor é consumido via `.take()` no frame final, impedindo qualquer reuso de contador.
  2. *Entre envelopes distintos:* Cada envelope gera independentemente $32$ bytes de entropia para o seed KEM ($256$ bits de segurança) e $32$ bytes para o salt HKDF, garantindo chaves AES-256 independentes. Além disso, o prefixo de nonce de 7 bytes é gerado do CSPRNG do SO.
* **Veredito:** **APROVADO**.

### 4.4 AAD (Additional Authenticated Data)
* **Escopo:** O cabeçalho completo de 1179 bytes é fornecido integralmente como AAD para **todos** os frames da transmissão STREAM.
* **Integridade:** Qualquer adulteração em magic, version, suite, plaintext_length, recipient_public_key_sha256, hkdf_salt, stream_nonce_prefix ou kem_ciphertext resulta em falha de validação da tag AEAD.
* **Veredito:** **APROVADO**.

### 4.5 Randomness & Determinismo
* **Fonte:** Exclusivamente `getrandom::fill` via provedor de sistema (`sys_rng` $\rightarrow$ `ProcessPrng` / `BCryptGenRandom` no Windows).
* **Isolamento de Testes:** A injeção determinística de seeds (`keypair_from_test_seed`) é estritamente privada ao módulo de testes sob `#[cfg(test)] pub(super)` e inacessível por binários de produção ou chamadores externos.
* **Veredito:** **APROVADO**.

### 4.6 Zeroização
* **Escopo:** Aplicada com `zeroize::Zeroizing` a todas as estruturas sensíveis de posse do M2C:
  - Seed de 64 bytes da geração de chave (`Zeroizing<[u8; 64]>`);
  - Bytes da chave secreta serializada (`Zeroizing<Vec<u8>>`);
  - Segredo compartilhado ML-KEM (`Zeroizing<[u8; 32]>`);
  - Chave de conteúdo AES derivada via HKDF (`Zeroizing<[u8; 32]>`);
  - Buffer intermediário de decodificação de plaintext (`Zeroizing<Vec<u8>>`).
* **Veredito:** **APROVADO**.

---

## 5. Publication / Filesystem Review

### 5.1 Atomicidade e No-Clobber
* **Mecanismo:** Staging criado com `OpenOptions::create_new(true)` no mesmo diretório pai canonicalizado; gravação sequencial; `sync_all()`; fechamento explícito de handles; revalidação de namespace M4; e commit atômico via `fs::hard_link(staging, final_path)`.
* **Sem dependência de `exists()`:** O primitivo do sistema operacional (`CreateHardLinkW`) falha com `ERROR_ALREADY_EXISTS` (183 / `io::ErrorKind::AlreadyExists`) se o arquivo de destino existir, garantindo proteção contra sobrescrita mesmo em condições de corrida entre processos.

### 5.2 Commit Point e Máquina de Estados
1. `prepare_destination`: valida NTFS, ausência de reparse points e isolamento M4.
2. `create_new(true)`: cria staging privado `.m2c-m5-staging-<32 hex>`.
3. `write_all` / `encrypt_payload`: grava os dados.
4. `verify_staging_size`: valida tamanho em disco antes de commitar.
5. `file.sync_all()` e `drop(file)`: fecha todos os handles abertos.
6. Revalidação de isolamento M4 e canonicalização de diretório pai.
7. **COMMIT POINT:** `fs::hard_link(&self.path, &self.prepared.final_path)`.
   - Se falhar: `self.committed` permanece `false`. No `Drop`, o staging é removido.
   - Se suceder: `self.committed = true`. O destino final está permanentemente comprometido.
8. Limpeza pós-commit: `fs::remove_file(&self.path)` em melhor esforço. Se falhar, retorna `PublicationStatus::PublishedWithStagingResidue`, sem reportar erro espúrio de publicação.

### 5.3 Staging de Plaintext
* Em `unprotect_file`, o plaintext recuperado é escrito exclusivamente no staging temporário até que o último frame passe na verificação da tag AEAD e o tamanho final corresponda exatamente a `plaintext_length`.
* Se qualquer frame falhar (ex: frame 1 falha após frame 0 ser decodificado), o `Drop` de `StagedOutput` remove o arquivo de staging e nenhum byte é publicado no destino final.

### 5.4 Pressupostos NTFS e Reparse Points
* Validação via Win32 API: `GetVolumePathNameW`, `GetDriveTypeW == DRIVE_FIXED` e `GetVolumeInformationW == "NTFS"`.
* Verificação de ancestrais: `validate_ancestors_no_reparse` percorre a cadeia até a raiz do volume verificando `FILE_ATTRIBUTE_REPARSE_POINT`. Qualquer junction point ou symlink é rejeitado com `UnsafePath`.

---

## 6. Adversarial Tests Executed

Para falsificar as garantias declaradas, foi construído e executado um harness temporário contendo testes adversariais complementares aos testes de produção:

1. **`test_boundary_sizes_exact_roundtrip` (PASS)**
   - Plaintexts testados: $0$, $1$, $C-1$ ($1048575$), $C$ ($1048576$), $C+1$ ($1048577$), $2C-1$, $2C$, $2C+1$.
   - Round-trip byte a byte idêntico; contagem exata de frames e tags ($E(N) = 1179 + N + 16 \cdot F(N)$); ausência de resíduos de staging.
2. **`test_empty_envelope_tampering` (PASS)**
   - Envelope de tamanho zero ($1195$ bytes): bit-flips individuais nos offsets 0 (magic), 8 (version), 10 (suite), 12 (length), 20 (fingerprint), 52 (salt), 84 (nonce), 91 (kem), 1179 (tag); truncamento do tag; adição de byte trailing.
   - Todos falharam fechados, sem criação de arquivo de saída e com limpeza imediata de staging.
3. **`test_multiframe_adversarial_stream_mutations` (PASS)**
   - Reordenação de frames (swap frame 0 e 1); duplicação de frame 0; duplicação de frame 1; concatenação de envelopes válidos.
   - **Cenário crítico de staging:** adulteração de byte no frame 1 de envelope de 2 frames ($C + 1000$ B). O frame 0 decodificou com sucesso e gravou 1 MiB no staging; a falha de tag no frame 1 abortou a operação, o staging foi completamente removido e nenhum byte apareceu no destino final.
4. **`test_adversarial_lengths_no_oom` (PASS)**
   - Header com `plaintext_length = 2^52` sobre arquivo físico de 1195 bytes: rejeitado imediatamente por `InvalidLength` em tempo $O(1)$, com $0$ alocações de payload e sem OOM.
   - Header com `plaintext_length = u64::MAX`: rejeitado imediatamente por `InputTooLarge`.
5. **`test_adversarial_keys` (PASS)**
   - Chave truncada em 0, 1, 15, 16, 1199 bytes; trailing bytes (1201 bytes); magic incorreto; payload declarado divergente ($9999$ vs $1184$).
   - Todos retornaram erros tipados (`InvalidLength`, `InvalidMagic`) sem panic.
6. **`test_path_and_atomicity_adversarial` (PASS)**
   - Caminhos com trailing dot (`foo.`), trailing space (`foo `), traversals (`..`), alternate data streams (`bar.m5:stream`), prefixo de staging reservado (`.m2c-m5-staging-foo`), diretório pai inexistente, diretório pai sendo arquivo comum, e arquivo de destino pré-existente.
   - Todos rejeitados deterministicamente com integridade dos arquivos preexistentes preservada.
7. **Verificação em modo Release (PASS)**
   - O harness adversarial foi executado sob `--release --all-features`, confirmando que todas as checagens aritméticas e de limites comportam-se de forma idêntica em builds otimizados sem depender de debug overflow traps.

*O harness temporário foi completamente removido após a execução, preservando a integridade da árvore de código.*

---

## 7. Regression / Isolation

* **Isolamento de Código M0–M4:** A adição do M5 não alterou nenhuma linha dos módulos centrais de processamento (`copybook`, `schema`, `codec`, `decode`, `source`, `parquet_io`, `pipeline`, `manifest`, `recovery`).
* **Isolamento de API:** A API pública M0–M4 (`convert_file`, `convert_parts`, `parse_and_compile_copybook`) permanece 100% idêntica.
* **Isolamento de Build:** No build padrão (`cargo build` sem `--features pqc`), nenhum código M5 é compilado e nenhuma dependência criptográfica (`ml-kem`, `aes-gcm`, `aead-stream`, `hkdf`, `windows-sys`, `zeroize`) é incluída no binário.
* **Isolamento de Namespace:** Operações M5 rejeitam sumariamente qualquer destino ou staging dentro de diretórios controlados pelo M4 (identificados por `.m4.lock`, `.manifest.json.tmp`, `.complete.json.tmp`, estrutura `parts/`+`commits/`, ou manifestos M4).

---

## 8. Verification Commands

Todos os comandos foram executados diretamente no ambiente com sucesso:

```bash
cargo fmt --all -- --check
# Result: PASS (exit code 0, clean formatting)

cargo clippy --all-targets --all-features -- -D warnings
# Result: PASS (exit code 0, 0 warnings)

cargo test --all-targets
# Result: PASS (exit code 0, 117 passed; 0 failed)

cargo test --all-targets --all-features
# Result: PASS (exit code 0, 145 passed; 0 failed)

cargo test --doc
# Result: PASS (exit code 0, 1 passed; 0 failed)

cargo test --doc --all-features
# Result: PASS (exit code 0, 1 passed; 0 failed)

cargo test --release --test protection --all-features
# Result: PASS (exit code 0, 6 passed in tests/protection.rs)
```

---

## 9. Residual Risks

1. **Ausência de Transação de Sistema de Arquivos em Queda de Energia:** Conforme expressamente delimitado em `docs/M5_PROTECTION.md` (Seção 11.3), `sync_all` garante que os dados sejam descarregados nos buffers do disco antes do commit, mas NTFS não fornece garantia de atomicidade de metadados de diretório contra corte abrupto de energia elétrica sem journal commit flush especializado.
2. **Resíduo de Staging após `SIGKILL` / Crash de Processo:** Se um processo for abortado de forma anômala durante `unprotect_file` enquanto escreve dados parciais em staging, o arquivo temporário `.m2c-m5-staging-*` permanecerá no disco até ser limpo manualmente pelo operador. O arquivo de destino final não é criado.
3. **Privilégio de Criação de Symlinks:** O ambiente de teste local não possui `SeCreateSymbolicLinkPrivilege`, limitando a verificação de junctions/symlinks à análise estática rigorosa de `is_reparse` e das APIs Windows subjacentes.

---

## 10. Final Gate Assessment

| Gate | Gemini verdict | Reason |
|---|---|---|
| **G0** | **PASS** | O contrato congelado `docs/M5_PROTECTION.md` é completo, unívoco, sem pendências conceituais e sem vazamento de escopo M6+. |
| **G1** | **PASS** | M5 implementado como componente autônomo e opcional sob feature `pqc`; crates auditados (`ml-kem 0.3.2` FIPS 203, `aes-gcm 0.11.1`, etc.). |
| **G2** | **PASS** | Regressão comprovada: 117 testes default passam sem modificações em M0–M4; dependências criptográficas completamente isoladas do build padrão. |
| **G3** | **PASS** | Todos os invariantes demonstrados: publicação atômica no-clobber, limites matemáticos de $2^{32}$ frames / $2^{52}$ bytes, AAD integral, fail-closed sob adulteração e isolamento de namespace M4. |
| **G4** | **PASS** | Matriz de verificação executada integralmente (`cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo test --doc`, debug e release) com 100% de sucesso. |
| **G5** | **PASS** | Critérios de aceitação atendidos integralmente; sem achados BLOCKER ou IMPORTANT; documentação e contratos consistentes com o código implementado. |
