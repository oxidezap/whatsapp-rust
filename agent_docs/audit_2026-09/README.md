# Auditoria de arquitetura — whatsapp-rust, whatsapp-rust-bridge, baileyrs (v2)

Data: 2026-09-02. Commits auditados: `whatsapp-rust@310b969`, `whatsapp-rust-bridge@2332118`, `baileyrs@441c7f4`.

**Esta é a segunda versão.** A primeira (seis relatórios de leitura de código, agora em `appendix/`) foi submetida a uma rodada adversarial: dois verificadores tentaram falsificar cada achado, e mais três agentes olharam por ângulos que leitura de código não dá: histórico git (o que é quente e frágil vs. grande e frio), drift real dos contratos e defeitos de correção nas fronteiras, e arquiteturas alternativas com consumidores externos medidos. Somam-se medições de `cargo check`. O resultado muda prioridades e derruba várias recomendações. A seção 1 lista o que estava errado.

| Apêndice | Conteúdo |
| --- | --- |
| `appendix/core-src.md`, `wacore.md`, `core-peripheral.md`, `bridge-rust.md`, `bridge-ts.md`, `baileyrs.md` | Primeira rodada: leitura de código por área, com `arquivo:linha`. **Ler junto com a errata correspondente.** |
| `appendix/verify-core.md` | Verificação adversarial dos três relatórios do core: veredito por achado (CONFIRMED / OVERSTATED / WRONG / UNSAFE) |
| `appendix/verify-bridge-baileyrs.md` | Idem para bridge e baileyrs |
| `appendix/history.md` | 12 meses de git: arquivos quentes, co-mudança, cascata entre repos, taxa de re-fix, cadência de release |
| `appendix/drift-and-defects.md` | Drift medido hoje entre as três camadas + 12 defeitos de correção com repro |
| `appendix/alternatives.md` | Sete arquiteturas alternativas avaliadas, consumidores externos medidos (crates.io, npm), impacto semver |
| `appendix/build-timings.txt` | `cargo check --timings` do workspace e tempos incrementais |

---

## 1. Errata da primeira versão

O que a rodada adversarial derrubou ou corrigiu. Os números da v1 estavam inflados em 30 a 60% de forma sistemática (contagens à mão), e sete recomendações quebravam invariantes documentados ou consumidores reais.

| Afirmação da v1 | Veredito | O que é verdade |
| --- | --- | --- |
| 58 `map_err(BridgeError::from)` redundantes, −116 linhas | **Errado** | 56 dos 58 são expressões de cauda; `?` não economiza nada. Ganho: 0. |
| Eventos do bridge deveriam emitir JID como string `user@server` | **Inseguro** | `Jid::Display` é lossy: `write_jid!` nunca renderiza `integrator`, que o decoder preenche para `@interop` e `is_same_chat_as` compara. JID-string colapsaria chats interop distintos. O baileyrs também lê `.agent`/`.device` do struct. |
| `js_backend.rs` re-deriva a política de `in_memory.rs` (−2.000 no core) | **Errado / inseguro** | `in_memory.rs` são `HashMap`s tipados com enumeração nativa, sem self-index, sem chave composta, sem expiry scan. Não há −2.000. E o esquema de chaves é **contrato persistido lido diretamente pelo baileyrs** (`use-multi-file-auth-state.ts`, `legacy-store/constants.ts`) e documentado no README para usuários. |
| `@bufbuild/protobuf` sem uso no baileyrs; remover | **Errado** | É load-bearing: o `.d.ts` publicado do bridge importa dele, e o baileyrs subclassifica `BinaryReader`. Com `skipLibCheck: true` a remoção tornaria a classe base `any` silenciosamente. O problema real é o inverso: **o bridge publica um `.d.ts` que depende de uma devDependency.** |
| Estreitar `exports["./lib/*"]` do baileyrs | **Inseguro** | O `baileys@7.0.0-rc13` upstream não tem `exports` map: todo deep import em `lib/*` funciona. `./lib/*` é a promessa drop-in. |
| ~820 linhas de Signal/retry "não usadas pelo socket" (−820) | **Inseguro** | O upstream exporta todas (`extractE2ESessionFromRetryReceipt`, `makeCacheableSignalKeyStore`, `MessageRetryManager`...); o baileyrs re-exporta por compatibilidade; `check-layer-boundaries.ts` **proíbe** o socket de importá-las. É design, não código morto. Ganho real: −100 (consolidar LRU/cache). |
| Unificar 13 enums `features/*Error` num `FeatureError` | **Superestimado / inseguro** | Só ~5 têm o shape idêntico; `PollError`, `TcTokenError`, `BusinessError`, `MexError` são heterogêneos. O bridge faz `match` em **55** braços desses enums. Ganho: −40 a −60. |
| `MemoryReport` → tabela `(&str, u64)` (−200) | **Inseguro** | 66 campos `pub` lidos por nome no bridge (22 leituras em `connection.rs`), em `memory_soak.rs` e exemplos; `tests/report_coverage.rs` exige o nome de cada campo em `memory_report()`. Já existe `collections()` de 13 entradas e `Display` já itera. Seguro: só o `Display`, −40 a −60. |
| Apagar a família do meio de traits de store libsignal (−58 wacore, −900 src) | **Inseguro** | `impl SessionStore for Device` é o **caminho de bypass do cache** documentado em `signal_durability.md`, com `direct_store_incarnation()` próprio; `check_session_exists` passa por ele em produção. É reescrita de caminho de durabilidade, não remoção de cola. E `opencrabs` (crates.io) implementa `wacore::store::traits` e já foi quebrado uma vez por mudança nesse shape. |
| `SignalStoreCache` com placeholder `Loading` single-flight | **Inseguro** | `signal_cache.rs:86-96` documenta a janela de remoção como "livre de estado por leitor que uma leitura cancelada possa encalhar"; um slot `Loading` é exatamente esse estado. Manter só o `flush_store` genérico, e mesmo esse é −60 a −80, não −190: o bloco de sessões carrega a ordem de deleção de prekey consumida. |
| `history_sync.rs` sem teste diferencial entre walker e fallback | **Errado** | `differential_fast_path_matches_full_decode_oracle` existe em `history_sync.rs:2772`. |
| Extrair VoIP em `wacore-voip` é mecânico | **Inseguro como descrito** | `wacore::types::call::MediaOffer.relay: Option<voip::relay_parse::RelayData>` e `stanza/call.rs` chamam `voip::relay_parse`, enquanto `voip` usa `stanza::call`, `types::group_call`, `stats`. `RelayData` + `hbh_srtp` + `rtcp` precisam ficar em `wacore` ou os crates ciclam. E `subsystem_boundary.md` não "adiou" a decisão dos três helpers: registrou explicitamente que ficam. |
| 284 gates `voip-runtime` em `src/`, +66% vs. a doc | **Errado** | A doc conta só produção; produção hoje é **161** (abaixo dos 171 da doc). `handle` tem 28 braços, não 39. `CallConfig::for_group` tem 53 linhas, não 284. |
| ~30 campos do `Client` lidos por um único módulo violam `subsystem_boundary.md` | **Errado** | A doc aplica o teste 2 a *subsistemas* e diz que `src/message`/`src/send` "falham por construção". Várias alegações de leitor único são falsas (`pdo_requested` é lido por `message/retry.rs`, `undecryptable_dispatched` por dois módulos...). VoIP já está no seam. ~6 campos são genuinamente de leitor único. |
| Agrupar `Client` em sub-structs | **Inseguro sem preparo** | `tests/report_coverage.rs` parseia `struct Client` com `syn` e desce **um** nível; `Client -> OfflineSync -> Cache` fica invisível ao guard. Estender o guard primeiro. |
| `abprops.rs` custa compilação | **Superestimado** | `props.rs:62-66` já documenta que só as constantes lidas materializam no binário. Ganho é ruído de diff, não build. |
| `run() -> Promise<TerminalReason>` economiza −300 a −400 no baileyrs | **Parcial** | A *conclusão* do loop é exponível hoje no bridge (−150). A *razão* exige um `RunExit` no core, que não existe; `ClientLifecycle` não tem hook de saída. |
| `uploadEncryptedMediaStream` deveria usar `Client::upload_stream` e parar de bufferizar | **Parcial** | Dedupe vale (−170), mas `UploadSource` exige `Send + Sync` com `reader_from` síncrono; um `ReadableStream` JS não satisfaz. Continuaria bufferizando. Ganho de memória: 0. |
| Throttle de spawn do bridge compensa thundering herd no core | **Não verificado / obsoleto** | O comentário culpa um semáforo que "acorda todos"; `async-lock 3.4.2` acorda um. Medir antes de mexer no core. |
| Remover `scopeguard` (2 usos) | **Errado** | 7 usos em produção. Manter. |
| Aliases de feature economizam legs de `cargo hack` | **Errado** | Mantendo os aliases (como a própria v1 recomenda) economiza zero legs. `tokio-native` está no `default` e é usado por e2e/bench/voip-cli. |
| 26 de 58 adapters do baileyrs são `noop` | **Superestimado** | 66 entradas, 16 `noop` incondicionais. |
| 240 helpers em `message/tests.rs`; 145 campos no `Client`; 40 `IqSpec` com `Response = ()` | **Superestimado** | ~95 helpers; 137 campos (145 contando `cfg(test)`); 25 `IqSpec`. |
| `to_str = to.to_string()` no send é alocação só para log | **Errado** | É a chave `&str` de `SenderKeyName::from_parts` e de quatro tabelas, armazenada em campo de struct. |

O que a rodada adversarial **confirmou** sem ressalva: testes inline nos mega-arquivos; `mex_operations` 79% sem referência (recontagem independente: 21 módulos, 189 structs, 2.126 de 11.022 linhas); resolução PN⇄LID ×8; `BotBuilder` ×17 setters; `update_device_list(s)_guarded` duplicado; derive `ProtocolNode` sem filhos; duplicações em `wacore-binary`; 51 mocks de store libsignal; código morto listado (com duas ressalvas: `wrap_device_sent` é oráculo de teste e `PlaintextContent` é re-exportado); `wam-catalog` 132k para 8 eventos; 7 loops de retry inline no sqlite; 23 `_for_device`; 26 `_if_current`; quatro teardowns Drop; `proto_gen.rs` morto; `fromPartial`/`create` gerados e não declarados (recontagem: **10.900** linhas, mais que a v1); `TSIFY_STRUCTS` obsoleto; arquivos mortos em `ts/`; fixtures copiadas; `camel-long.test.ts` testa a si mesmo; 106 a 123 sites `(await ctx.getClient())`; event-object `message` morto no baileyrs; three copies de Buffer.

---

## 2. O que as vistas novas mostram

### 2.1 Histórico: onde o custo de manutenção realmente está

12 meses, 1.277 commits no core (91% de um autor; bus factor 1 nos três repos). A tabela de tamanho da v1 apontava para os arquivos errados.

| Arquivo | Linhas | Commits | Fixes | Taxa re-fix ≤7d | Veredito |
| --- | ---: | ---: | ---: | ---: | --- |
| `src/client.rs` + `lifecycle.rs` + `accessors.rs` + `node_io.rs` | 8.8k | 334 | 105 (31%) | 0,89–0,90 | **Quente e frágil.** O split de junho moveu métodos; `client.rs` voltou de 914 para 2.185 linhas em 90 dias e ainda co-muda com seis arquivos a 0,67–0,82. Os últimos sete fixes de `lifecycle.rs` (29/07 a 02/09) são todos "reconexão vs. shutdown terminal", e cinco foram re-publicados como `deps!` no bridge. |
| `src/retry.rs` | 5.4k | 117 | 32 | 0,68 | Quente e frágil; cresceu 16× no ano; compartilha 21 commits com `sender_keys.rs` (0,70). |
| `src/send/mod.rs` | 8.2k | 222 (follow) | 51 | 0,91 | Quente e frágil. |
| `src/message*` + `handlers/message*` | — | 208 | 72 (35%) | 0,83 | `receipt.rs` tem a maior sequência de fixes consecutivos do repo (9). |
| `sqlite_store.rs` | 7.2k | 82 | 19 | 0,61 | Quente; co-muda com `store/traits.rs` em 40 dos 51 commits do trait (0,78). **O trait "agnóstico" não entrega evolução independente.** |
| `signal_cache.rs` | 6.2k | 40 | 10 | — | 6 fixes em 23 dias de gates de durabilidade, encerrados em 07/08; nenhum desde. **Não abrir agora.** |
| `src/plugins/mod.rs` | 6.6k | 8 | 0 | — | **Frio. Deixar.** |
| `wacore/src/iq/groups.rs` | 5.8k | 43 | 6 | — | Cresce, não quebra. |
| `history_sync.rs` | 4.8k | 33 | 8 | 0 | Estável. |
| `wacore/src/voip/driver.rs` | 4.3k | 11 | 3 | — | Frio. |
| VoIP total | 33.6k | 80 | 37 (46%) | — | Tem **três meses**; é curva de estabilização, não sinal de design. Refatorar agora é refatorar requisitos ainda em descoberta. |

No bridge: `wasm_client.rs` é o hub (65 commits); `result_types.rs` mudou em **17 de 17** commits junto com ele; `js_transport`, `runtime`, `camel_serializer`, `proto`, `js_backend` a 0,84–0,92. O split de agosto em `wasm_client/*.rs` moveu 4.664 linhas; desde então a raiz teve 8 commits e os onze submódulos 5 somados. O codec proto teve 8 fixes (3 breaking) em 12 dias, cada um regenerando 1.200 a 24.000 linhas no mesmo commit.

**A cascata entre repos custa edições, não dias.** Lag mediano zero em todos os saltos (o mesmo mantenedor faz os três no mesmo dia). O custo: 44 dos 62 pins do core no bridge exigiram edição manual de fonte (71%), sempre nos mesmos arquivos (`wasm_client.rs` 35×, `js_backend.rs`, `result_types.rs`, `errors.rs`); 21 dos 46 bumps do bridge no baileyrs idem (46%: `Socket/events.ts` 15×, `Bridge/types.ts`, `schema.ts`). 13 dos últimos 15 pins do bridge apontam para commits do core **não lançados**; 8 dos 12 `BREAKING` do bridge são `deps!` repassando o core; o core tem 158 commits não lançados com zero marcadores `!`. O marcador de breaking está sendo aplicado uma camada abaixo de onde a quebra nasce.

### 2.2 Defeitos de correção encontrados (não são arquitetura; são bugs)

| # | Sev. | Onde | Defeito | Correção mínima |
| --- | --- | --- | --- | --- |
| D1 | **Alta** | baileyrs `use-bridge-store.ts:231,242` | `touchCache` antes de `writeCritical`; `set` pula quando o cache é igual. Uma escrita Signal que falha (ENOSPC) não é retentada: o re-flush do core com bytes idênticos é "igual ao cache" e pulado, o gate é liberado sem nada no disco, e o ciphertext é publicado sob lease não durável (viola `signal_durability.md`). | Marcar cache só após a escrita resolver; evict no throw. |
| D2 | **Alta** | baileyrs `use-bridge-store.ts:82-104` | `writeCritical` é `writeFile` in-place sem fsync/rename; `flushWrite` engole todo erro. SIGKILL no meio → sessão truncada → peer indecifrável. ENOSPC no history sync → 20k `msg_secret` perdidos em silêncio. | tmp + fsync + rename; propagar erros não-ENOENT. |
| D3 | **Alta** | core `lifecycle.rs:757-761, 965-1003` | Com `setAutoReconnect(false)`, uma desconexão *esperada* (ex.: 515 após pareamento) sai do loop sem evento. O bridge `run()` é `void`, baileyrs fica em `connecting` para sempre; o watchdog de 60 s nunca arma. | Core: despachar evento terminal no `break`; ou bridge expor conclusão do loop. |
| D4 | **Alta** | baileyrs `Socket/index.ts:297,442,905` vs. core `node_io.rs:1990` | baileyrs espelha `enable_auto_reconnect` localmente; o core o limpa sozinho em 402/405. Espelho `true` → `connecting` espúrio após close terminal; espelho `false` → dois `connection.update {close}`. | Consultar `reachability()` do bridge; guard de "uma vez" global. |
| D5 | Média | bridge `wasm_client.rs:648,1301` | Canal de eventos `bounded(16_384)` com `try_send`; overflow = `warn!` e **evento descartado**, invisível ao host. O handler do core é síncrono, então back-pressure exige mudança de trait no core. | Contador `events_dropped` em `getMemoryDiagnostics` já; back-pressure depois. |
| D6 | Média | bridge `js_backend.rs:1574-1603` | `Device`/`account` gravados em duas chaves sem atomicidade; `account = None` nunca apaga a chave antiga → re-pareamento na mesma pasta ressuscita o `ADVSignedDeviceIdentity` antigo. | Deletar em `None`; `js_put_many`. |
| D7 | Média | bridge (11 sites) | Input do chamador reportado como `internal` (versão, `wantedPreKeyCount`, callbacks de store, tag em `sendNode`, bytes em `sendMessageBytes`...); `media.rs:430` mapeia `NotFound/DecryptionError → internal`; `From<JidError>` fixa `field: "jid"`. | `invalid_arg(field, …)` + parser de JID nomeado. |
| D8 | Média | core `src/request.rs:240-242` | `from_response` copia 6 variantes de `wacore::IqError` com `_ => InternalChannelClosed`; qualquer variante nova vira "canal fechado". | Envolver (`Core(wacore::IqError)` + `response`) em vez de copiar. |
| D9 | Baixa | sqlite `sqlite_store.rs` ×7 | Confirmado: dois loops usam backoff sem teto e avisam a cada tentativa; cinco **nunca logam**. Uma falha em `remove_prekey` é silenciosa, tornando invisível o hazard de prekey consumida. | Rotear por `with_retry`. |
| D10 | Baixa | bridge `js_backend.rs:504-575` | TOCTOU entre `confirm_expired` e `js_delete_many`. | Compare-and-delete no store. |
| D11 | Baixa | bridge/baileyrs | Atomicidade de `setMany` é promessa do host não declarada; seguro só porque o core re-flusha, o que D1 quebra. | Documentar no `JsStoreCallbacks`. |
| D12 | Baixa | bridge | `ReceiptType::Other` serializa como `{Other: s}` no evento-objeto e flag+string no packed; baileyrs mapeia o objeto para `undefined`. `Receipt.timestamp` é RFC-3339 no objeto, `f64` no packed, inteiro em `IncomingCall`. | Um formato. |

Também: `pair_success.id` declarado `Jid` em `generated_types.rs:1136` mas emitido como string (a interface gerada está morta e errada); `TemporaryBan.expire` é duração declarada como `number`; `toUnixSeconds` do baileyrs devolve **0** em falha de parse (epoch silencioso).

### 2.3 Drift medido hoje

- **Eventos:** core 71 variantes → bridge 67 tipos (4 deliberadamente não despachados) → baileyrs 66 adapters, 16 `noop` incondicionais. Dois são lacunas reais: `self_push_name_updated` deveria alimentar `creds.update` (`me.name`) e `user_about_update` deveria alimentar `contacts.update` (`status`). Sete eventos do Baileys upstream o baileyrs nunca emite (`blocklist.*`, `newsletter-participants.update`, `chats.lock`, `messages.media-update`...).
- **Erros:** dos 11 kinds do bridge, o baileyrs trata **2** por kind (`invalid-argument`, `no-recipient-device`). `serverCode`/`backoffSeconds`/`operation` não são lidos em lugar nenhum. De `DisconnectReason`, `restartRequired`, `multideviceMismatch`, `badSession`, `connectionLost` são **inalcançáveis**; `timedOut` só via QR esgotado; e `405` é emitido sem ser membro do enum.
- **Ações de grupo:** 44 no core → 43 no `.d.ts` (falta `Unknown`) → 44 no canonical → **25 de 44 perdidas** no último salto (`group-notifications.ts:135-191` com `default: return null`): delete, link/unlink, suspended, revokeInvite, changeNumber...
- **`IqError` gêmeos:** 6 variantes copiadas, `is_timeout`/`is_transport_unavailable` duplicados, dois braços de mapeamento no bridge. O split é real (wacore não tem transporte), a cópia é acidente.
- **Proto:** `proto-types.d.ts` do bridge tem 1.637 nomes; `WAProto/index.d.ts` do baileyrs (cópia congelada de 23/07, perdeu duas regenerações de agosto) tem 1.127. 512 só no bridge; 2 só no baileyrs, ambos usados só em fuzz.
- **Auditoria de declarações do baileyrs não é gate de CI.** `ci.yml` não roda `compat:audit:strict`. O README diz "checked rather than assumed"; para exports é manual.

### 2.4 Tempo de compilação, medido

`cargo check --timings` do workspace default (4 cores, deps cacheadas), unidades mais caras:

| Unidade | Tempo |
| --- | ---: |
| `waproto` (check) | 54,4 s |
| `wacore` (check) | 30,2 s |
| `whatsapp-rust` (check) | 20,6 s |
| `syn` | 16,6 s |
| `diesel` (check) | 14,0 s |
| `waproto` build-script (run) | 13,4 s |

Incremental: `wacore` após `touch lib.rs` 111 s de parede; após `touch mex_operations.rs` 36 s; `whatsapp-rust` após `touch src/lib.rs` 40 s.

Conclusão que a v1 não tinha: **`waproto` é a unidade mais cara do workspace**, quase o dobro de `wacore`. Podar as 752 mensagens do `.proto` para o fecho transitivo das ~140 usadas (o `build.rs` já reescreve o `FileDescriptorSet`) vale mais para o ciclo de desenvolvimento do que `mex_operations` e muito mais que `abprops` (que não custa build). No artefato wasm, `waproto` também é o maior dono da seção de código (17,5%).

### 2.5 Consumidores externos, medidos

- crates.io: `whatsapp-rust` tem dois dependentes reversos: `mendia` 1.16 (`^0.3`, usa 8 símbolos: `Bot`, `Client`, `Event`, `Message`...) e `opencrabs` 0.3.83 (`^0.6`, usa `wacore::store::traits::{Backend, SignalStore, DeviceStore}`, `appstate::*`, 39 tipos `waproto`; já foi quebrado por mudança no `Backend`). Mais `whatshell` (binário Rust via npm, `= 0.5.0`). Nenhum nomeia `features::*Error`, `_for_device`, nem os `pub fn` mortos do wacore. Todos atrasam de 1 a 4 minors.
- npm: `@oxidezap/whatsapp-rust-bridge` 4.017 dl/mês; `@oxidezap/baileyrs` 3.320 e é o **único consumidor conhecido da API de cliente** do bridge. O `whatsapp-rust-bridge` sem escopo (0.5.5, repo antigo) tem **4,3 milhões** dl/mês porque é dependência de runtime do `baileys@7.0.0-rc14` upstream para exatamente dois símbolos (`LTHashAntiTampering`, `expandAppStateKeys`) que o pacote atual **não exporta mais**.
- O bridge é dois produtos sob um nome: utilitários (até 03/2026, o que o upstream adotou) e motor de cliente (desde 03/2026, um consumidor). O README do bridge ainda descreve o primeiro.

---

## 3. Achados que sobrevivem, re-priorizados

Critério: confirmado na verificação × quente no histórico × sem quebra de invariante. Estimativas já descontadas.

### Prioridade 1 — onde os fixes se acumulam

1. **Máquina de estados de reconexão/terminal (core `lifecycle.rs`, bridge `run()`, baileyrs `terminal-close*`/`bridge-client-owner`).** 105 fixes no hub, re-fix 0,9, D3 e D4 são bugs abertos hoje. Não é layout de arquivo: é decidir uma vez quem pode anunciar reconexão e quem detém o lock terminal, e expor a **conclusão** do loop com razão (`RunExit` no core; `run(): Promise<TerminalReason>` no bridge). Isso apaga `terminal-close.ts`, o espelho de `enable_auto_reconnect`, os casos especiais de `disconnected`/`streamError`, e a contagem de claims em `logout()`.
2. **Durabilidade do store JS (D1, D2, D6, D11).** Três bugs de alta severidade no caminho que `signal_durability.md` protege com mais cuidado no core. Correção é pequena e local ao baileyrs/bridge.
3. **`retry.rs` + `send/mod.rs` + recepção (`process_session_enc_batch` 747 linhas / 13 níveis).** Extrair `classify_decrypt_failure` e `prepare_group_send()`, preservando a disciplina de drop do `session_guard`. Retry possui estado de sender-key que pertence à camada Signal (21 commits compartilhados com `sender_keys.rs`).
4. **`Client` hub.** Remover `Arc<>` dos ~35 campos nunca clonados (confirmado: 0 clones) encurta `assemble()`; sub-structs por domínio **depois** de estender `report_coverage.rs` para descer mais de um nível.

### Prioridade 2 — volume gerado e tempo de compilação

5. **`waproto` podado por `WANTED` transitivo** no `build.rs`: maior unidade de compilação (54 s) e maior dono do wasm.
6. **`mex_operations` `WANTED`** (−8.900 linhas, 21 módulos usados). `abprops` idem, mas só por ruído de diff. `wam-catalog` como dados (−110k+; 8 eventos usados; membro do workspace pago em todo leg `--workspace`).
7. **`fromPartial`/`create` fora do codec ts-proto** (−10.900 linhas geradas, ~−110 KB de bundle) e namespace lazy por tipo (a doc do próprio bridge mediu −1,9 MiB/processo e escreveu "não implementado aqui").
8. **`wacore/Cargo.toml`: `exclude` dos 16 MB de `mlow/testdata`.** Uma linha; o crate publicado saiu de 289 KB (0.6.0) para 7,29 MB (0.7.0). Verificado que todo `include_*!` está sob `cfg(test)` e `tables.desc` fica fora da pasta.

### Prioridade 3 — DRY mecânico, risco zero

9. Mover `mod tests` inline (core: 8 arquivos ~23k linhas; wacore: `signal_cache`, `iq/groups`, `history_sync`, `stanza/call`, `usync`; bridge: `wasm_client.rs` −1.556).
10. Fixtures compartilhadas: `InMemProtocolStore` sob `test-util` em `wacore-libsignal` (51 mocks), `TestClientBuilder` + `iq_result/iq_error` no core, `tests/helpers.ts` no bridge, `socket-harness.ts` no baileyrs. Para `appstate_sync.rs`, o alvo é `InMemoryBackend` + wrapper de falha, não `create_test_backend()` (é SQLite real, não injeta `fail_clear_macs`).
11. Código morto confirmado: `proto_gen.rs`, `ts/{index.d.ts,macro.*,proto-types.ts}`, `benches/codec-memory` (após promover `equivalence.mjs` a teste), os `pub fn` do wacore listados (menos `wrap_device_sent`, que vira `cfg(test)`), `adaptMessage` no baileyrs.
12. Duplicações locais: PN⇄LID ×8 → um resolvedor com `Keep::{Device,Bare}` explícito; `BotBuilder` sobre `ClientBuilder`; `update_device_list_guarded` → plural; `_if_current` ×26 → `entry_mut(id, Option<gen>)`; quatro teardowns → um; 7 loops de retry → `with_retry`; `js_fn.rs` para as seis cópias de "pegar função JS"; `forward()` no baileyrs para os 106 sites uniformes; `AttrSource`/`NodeLike` no `wacore-binary`; derive `ProtocolNode` com `#[child]`/`#[children]`.

### Prioridade 4 — só com release major coordenado

13. `_for_device` ×23 no sqlite (0.8 do storage; zero chamadores externos encontrados).
14. Um shape de evento no bridge: camelCase + timestamps unix + `ReceiptType` correto + tipos de action exportados + `pair_success` corrigido. **JID continua struct** (Display é lossy). Isso reduz `schema.ts`/`types.ts`/`primitives.ts` em ~800 linhas, não 1.700.
15. VoIP em crates próprios **com** `RelayData`/`hbh_srtp`/`rtcp` permanecendo em `wacore`, e só quando a curva de fixes achatar.

---

## 4. Arquitetura-alvo revisada

A v1 assumia três repositórios e propunha ensinar o core a servir "qualquer binding" (serde camelCase, `classify()`, `ts-rs`) para preservar a fronteira. A avaliação de alternativas (`appendix/alternatives.md`) mostra que essa fronteira é a causa direta de ~6.500 linhas do bridge, via **regra do órfão**: o bridge não pode derivar `Tsify` em tipos que não possui, então raspa os fontes do core com `syn` (2.355 linhas, já divergiu), espelha 80 structs à mão, e reclassifica 15 enums de erro por downcast. E ninguém mais consome essa generalização: nenhum outro binding existe nem é pedido (`plugin_architecture.md`: "começa só com consumidor concreto").

**Recomendação: dois repositórios, não três.** O bridge vira `bindings/wasm/` dentro do workspace do core, com `publish = false`, fora de `default-members`, e o npm `@oxidezap/whatsapp-rust-bridge` publicado de lá. **A direção de dependência não muda**: o core continua sem depender do binding. O que muda é que os tipos do core ganham `#[cfg_attr(feature = "ts", derive(Tsify))]` sob uma feature que só o binding liga, e o PR que quebra o wasm descobre isso no próprio PR, não um repo e um release depois.

```
oxidezap/whatsapp-rust  (um workspace, um lock, um CI)
├── wacore*, waproto, whatsapp-rust, adapters          crates publicados, como hoje
├── (depois) wacore-mlow, wacore-voip, whatsapp-rust-voip   com RelayData/hbh_srtp/rtcp em wacore
├── plugins/, tools/whatspec-codegen                   não publicados, como hoje
└── bindings/wasm/   publish = false; npm @oxidezap/whatsapp-rust-bridge
    ├── src/   ~15k linhas: wasm_client/* (141 dos 174 métodos são tabela; ~23 à mão),
    │          js_* adapters, wire_batch, BridgeError como shape de fio, um shape de evento
    ├── ts/    codec ts-proto sem fromPartial/create, namespace lazy, wire-info
    └── sem codegen/, sem generated_types.rs, sem result_types espelhados,
        sem signal_records/legacy_session DTOs, sem errors.rs classify
    + entry point utilitário: LTHashAntiTampering, expandAppStateKeys (o que o upstream pede)

oxidezap/baileyrs  (referência: Baileys upstream, não o core)
├── Socket/* com forward(ctx, method, {map, check}); DISPATCHERS direto no tipo do bridge
├── proto-runtime.ts como único facade; WAProto .d.ts gerado do descriptor do bridge
│   com naming protobufjs (compat:audit:proto prova a paridade); protobufjs fora de runtime
├── legacy-store/*, event-buffer, messages.ts, exports lib/*        promessa drop-in, ficam
└── fuzz diferencial + audit de .d.ts                               custo permanente; fuzz do codec vai para bindings/wasm
```

O que isso custa ao core: +1–2 runners numa matriz de 25–30 (o core já roda quatro builds wasm32 por PR e carrega 305 cfgs `wasm32` e 113 pares `?Send` para um consumidor que não vê). O que isso abandona da v1: `ts-rs` e `ErrorChainExt::classify()` como **promessas públicas** do core (viram conveniências in-tree, livres para mudar).

Alternativas rejeitadas com evidência: fundir bridge no baileyrs (o motor mudaria por razões do core, dono errado); attribute macro `#[export_binding]` no core (coloca o modelo do wasm-bindgen e a política de gate `online()/unwaited()` num proc-macro do core); uniffi/Python (nenhum consumidor; runtime e `Send` incompatíveis com a configuração wasm); split de plugins/app-state/history-sync em crates (zero efeito no artefato: são alcançados por `connect()`).

### Regra para o que WA Web tem e não temos, e vice-versa

O repo já aplica bem "o que carregamos e WA Web não constrói" (`schemas_unlisted.rs`, `props::stale` com gatilho de remoção). Falta a direção oposta, e a regra proposta em `alternatives.md` §G resolve os 163k de linhas geradas com um princípio: **o que WA Web tem e não usamos é filtro de codegen (`WANTED`), não fonte; o que temos e WA Web não tem precisa de dono e data de expiração** (ex.: `legacy-session-interop` expira quando o baileyrs fechar a janela de migração).

---

## 5. Roteiro revisado

Cada passo é entregável sozinho. Ordem por risco-por-linha e pelo que o histórico diz que dói.

**Passo 0 — bugs (dias, patch releases).** D1, D2, D6 (store JS); D3/D4 (evento terminal no core + guard global no baileyrs); D5 (contador de drops); D7 (`internal` → `invalid-argument`); D8 (`IqError` por envolvimento); `pair_success`; as duas lacunas de eventos (`creds.update` name, `contacts.update` status); `@bufbuild/protobuf` para `dependencies` do bridge; `compat:audit:strict` no CI do baileyrs. **Antes de qualquer refatoração**: os quatro testes que raspam fonte (`report_coverage.rs`, `subsystem_boundary.rs`, `ab_prop_watch_coverage.rs`, o self-scan em `sqlite_store.rs:6903`) são o primeiro arquivo a atualizar em cada move.

**Passo 1 — mecânico, risco zero (1–2 semanas).** Prioridade 3 inteira; `exclude` do testdata (patch de `wacore`); `WANTED` em mex e wam; `fromPartial`/`create` fora; arquivos mortos; `Arc<>` supérfluos no `Client`. Separar commits de comportamento de commits de regeneração no bridge (o core já faz: 14 de 14 regens são puros).

**Passo 2 — bridge para `bindings/wasm` (um PR, sem mudança de superfície).** `git subtree add`; `publish = false`; job de CI filtrado por path; apagar `codegen/`; publicar 0.20.0 de lá; arquivar o repo antigo com ponteiro.

**Passo 3 — feature `ts` no core (minor).** `Tsify`/serde nos tipos de resultado, payloads de evento, records Signal, `DevicePropsOverride`. Apagar `generated_types.rs`, ~75% de `result_types.rs`, `signal_records.rs`, `legacy_session.rs`, `device_props.rs`, `client_profile.rs`.

**Passo 4 — core 0.8 + bridge major, em lote.** `RunExit`/`run() -> TerminalReason`; um shape de evento (JID continua struct); `waproto` podado; `FeatureError` só nos 5 que cabem; `_for_device` fora; `pub fn` mortos fora (com blanket impl de uma release para `opencrabs` se algum trait de store mudar). Um único ciclo de migração para consumidores que já atrasam 1–4 minors.

**Passo 5 — baileyrs.** `forward()`; facade WAProto gerado; `protobufjs` fora de runtime (major do baileyrs, junto com qualquer estreitamento de `exports` que se decida); fuzz do codec para o bridge; `check-layer-boundaries.ts` vira lint do workspace.

**Passo 6 — estrutura interna do core, contínuo.** `Client` em sub-structs (após o guard); `app_state.rs` em módulos; derive `ProtocolNode` com filhos; `AttrSource`/`NodeLike`; `BotBuilder`; PN⇄LID único; `flush_store` genérico (só o flush). VoIP e `signal_cache.rs` **só quando a taxa de fix achatar**, e VoIP com o desenho de ciclo resolvido.

### Semver por pacote

| Pacote | Passos 0–3 | Passo 4 | Passo 5 |
| --- | --- | --- | --- |
| `whatsapp-rust` | 0.7.x patch/minor (feature `ts` aditiva) | **0.8.0** (lote) | 0.8.x |
| `wacore`, `wacore-libsignal`, `waproto` | 0.7.x (`exclude` é patch) | **0.8.0** | — |
| `whatsapp-rust-sqlite-storage` | 0.7.x | **0.8.0** (`_for_device`) | — |
| `@oxidezap/whatsapp-rust-bridge` | 0.20.0 (move de fonte, sem mudança) | **major** (shape de evento, `TerminalReason`) | — |
| `@oxidezap/baileyrs` | patch (bugs) | minor | **major** (`protobufjs`, exports) |
| `baileys` upstream (pina `whatsapp-rust-bridge@0.5.4`) | não afetado; opcionalmente oferecer os dois símbolos no entry point utilitário | — | — |

### Estimativa consolidada (descontada)

| Camada | Δ código | Δ artefato |
| --- | ---: | --- |
| whatsapp-rust | −30k geradas (mex, wam, waproto) + −3k a −4k código + ~25k testes movidos | `wacore` 7,3 MB → ~0,3 MB; `waproto` check 54 s → a medir após poda |
| bridge | −6,5k Rust (a maioria pela regra do órfão, não por API nova no core) + −11k TS geradas + −3,1k codegen/bench | −110 KB bundle; −1,9 MiB RSS/processo |
| baileyrs | −2k a −2,5k (a v1 dizia −4,5k; a metade era API drop-in) | −900 KB `.d.ts`; −1 dep runtime |

---

## 6. Verificado e que não precisa ser re-verificado

Mantido da v1 (locks através de `await`, alocação em hot path, gates fora do dono, sqlite sem N+1, layering do VoIP principiado, `wire_batch` não duplica protobuf, trait `Transport` do tamanho certo, socket do baileyrs sem padrão layered). Acrescentado nesta rodada:

- Ordem mensagem→recibo é preservada no bridge (canal FIFO, lookahead `pending_event`, flush de envelope antes de callbacks); reordenação só se um callback devolver thenable, e os do baileyrs são síncronos.
- `withdrawParkedCalls` toca só waiters de `online()`, nunca eventos.
- O bridge lê `reachability()` do core em vez de espelhar; o único lag é `setAutoReconnect(false)` escrever o atômico sem notify.
- `AttrParserRef::optional_u64` não é silencioso: adia o erro para `finish()`.
- `HttpClient`: os métodos de streaming têm corpo default; um implementador wasm já implementa só `execute`.
- `abprops` não custa binário nem build; só diff.
- `differential_fast_path_matches_full_decode_oracle` cobre o walker de history sync.
