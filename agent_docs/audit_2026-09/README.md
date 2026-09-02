# Auditoria de arquitetura — whatsapp-rust, whatsapp-rust-bridge, baileyrs

Data: 2026-09-02. Commits auditados: `whatsapp-rust@310b969`, `whatsapp-rust-bridge@2332118`, `baileyrs@441c7f4`.

Objetivo: oportunidades de melhorar arquitetura, performance, DRY, reduzir complexidade e reduzir código nas três camadas, respeitando a regra de dependência (`whatsapp-rust` não conhece o bridge; o bridge não conhece o baileyrs).

Este README é a síntese. Os seis apêndices em `appendix/` trazem cada achado com `arquivo:linha`, delta de linhas estimado, risco e se preserva comportamento:

| Apêndice | Escopo |
| --- | --- |
| `appendix/core-src.md` | `whatsapp-rust/src/` (client, send, retry, receipt, handlers, features, plugins) |
| `appendix/wacore.md` | `wacore/` (iq, stanza, store, binary, libsignal, noise, derive) e `waproto` |
| `appendix/core-peripheral.md` | VoIP, plugins/wam-catalog, sqlite-storage, transports, benches, matriz de features, dependências |
| `appendix/bridge-rust.md` | bridge `src/` (wasm_client, errors, result_types, js_* adapters, wire_batch, camel_serializer) |
| `appendix/bridge-ts.md` | bridge `ts/`, `codegen/`, `scripts/`, testes, pipeline de build |
| `appendix/baileyrs.md` | baileyrs `src/` (Bridge, Socket, Utils, Compatibility, WAProto), scripts, testes |

A visão transversal (seção 3) foi consolidada a partir desses seis; o agente dedicado a ela foi interrompido por limite de uso antes de escrever, e o caminho de dados que ele iria traçar já está evidenciado nos apêndices `bridge-ts.md` (achado 10) e `baileyrs.md` (achados 1, 3, 4, 5).

---

## 1. Números que enquadram o problema

| Camada | Linhas (`.rs`/`.ts`) | Geradas | Testes | Código "de verdade" |
| --- | ---: | ---: | ---: | ---: |
| whatsapp-rust | 589k | 163k | ~206k (inline) + 54k (arquivos) | ~170k |
| whatsapp-rust-bridge | 144k | 102k | ~13k | ~29k |
| baileyrs | 78k | 14k (`WAProto/index.d.ts`) | ~38k | ~26k |

Três conclusões saem só desses números:

1. **O volume dominante é gerado e não usado.** No core, `mex_operations.rs` (11k linhas, 947 structs) tem 79% nunca referenciado; `abprops.rs` (18.7k linhas, 2.664 constantes) tem 16 lidas em produção; `wam-catalog` (132k linhas) serve 9 eventos. No bridge, `ts/generated/whatsapp.ts` (81k) carrega `fromPartial`/`create` (~9.6k linhas) que a `.d.ts` publicada nem declara. O padrão `WANTED` que `wire_enums.rs` já usa resolve todos eles.
2. **Os arquivos "de 8k linhas" são um problema de posicionamento de teste.** Seis dos dez maiores arquivos do core são mais de 55% `mod tests` inline (`send/mod.rs` 63%, `retry.rs` 67%, `receipt.rs` 69%, `device_registry.rs` 65%, `appstate_sync.rs` 99,7%). Mover para `tests.rs` irmão (padrão que `src/client.rs` já usa) é mecânico, risco zero, e torna qualquer outra refatoração revisável.
3. **Cada camada re-implementa a semântica da camada de baixo em vez de consumi-la.** É a fonte de ~60% do código não gerado do bridge e do baileyrs (seção 3).

---

## 2. Achados por camada, ordenados por impacto

### 2.1 whatsapp-rust — `src/` (apêndice `core-src.md`)

| # | Achado | Δ linhas | Risco |
| --- | --- | ---: | --- |
| 1 | Mover `mod tests` inline dos oito arquivos gigantes para `*/tests.rs` (~23k linhas saem dos arquivos de produção) | 0 | nenhum |
| 2 | `Client` tem **145 campos**; ~40 `Arc<Atomic*>`/`Arc<Mutex>`/`Arc<Event>` nunca são clonados (o `Client` já vive em `Arc`). Agrupar por domínio (`ConnectionState`, `OfflineSync`, `AppStateSync`, `Pairing`, `RetryState`) | −150 a −250 | baixo |
| 3 | ~30 campos do `Client` são lidos por um único módulo (`pdo_*`, `pending_lid_refreshes`, `pending_retries`, `app_state_*`, `chatstate_*`), violando `subsystem_boundary.md`. Viram structs de estado por módulo | −50 a −100 | baixo |
| 4 | `MemoryReport`/`CacheConfig`/`assemble`/`memory_report()` espelham cada cache à mão em seis lugares (57 campos, 57 `writeln!`). Tabela `(&str, u64)` + loop | −180 a −220 | baixo |
| 5 | Resolução PN⇄LID implementada **oito vezes** com semânticas levemente diferentes (`lid_pn.rs` ×5, `tctoken_lifecycle.rs` ×3, `blocking.rs`, `polls.rs`/`events.rs` idênticos). Um resolvedor com `Keep::{Device,Bare}` explícito | −120 a −160 | médio |
| 6 | `client/app_state.rs` (3.7k de código) são quatro módulos num arquivo; `sync_collections_batched_inner` tem 484 linhas | ~0 | baixo |
| 7 | `process_session_enc_batch` 749 linhas / 13 níveis de indentação; `handle_success` 578; `send_group_branch` 388 / 10 níveis. Extrair `classify_decrypt_failure`, `prepare_group_send()` | −100 | médio (hot path) |
| 8 | 13 enums `features/*Error` com o mesmo shape `{Iq, Mex, InvalidRequest(String), Internal(anyhow)}`; 68 arquivos usam `anyhow`. Um `FeatureError` + variantes de domínio só onde existem | −120 a −180 | médio (API pública) |
| 9 | `BotBuilder` redeclara 17 setters do `ClientBuilder` em campos `Option` paralelos | −150 a −250 | baixo |
| 10 | `update_device_list_guarded` duplica `update_device_lists_guarded` | −60 | baixo |
| 12 | `appstate_sync.rs`: 5 linhas de re-export + 1.777 de teste com um `MockBackend` próprio, quando `create_test_backend()` já existe; `message/tests.rs` tem outro trio `Mem*Store` | −300 a −400 | nenhum |
| 13 | Fixtures de teste: 240 helpers em `message/tests.rs`, três `MockHttpClient`, cinco `create_transport`, 31 `<iq>` falsos construídos à mão apesar de `test_utils::answer_iq` existir | −800 a −1.500 | nenhum |

Verificado e **não** é problema: locks `std` mantidos através de `await` (zero em produção), alocação em hot path (`receive.rs` tem 2 `to_string()` em 2.2k linhas), gates de feature fora do dono (dentro do teto documentado).

### 2.2 whatsapp-rust — `wacore/` (apêndice `wacore.md`)

| # | Achado | Δ linhas | Risco |
| --- | --- | ---: | --- |
| 1 | `mex_operations.rs`: 79% nunca referenciado (747 de 947 structs, cada uma com derive `Serialize`+`Deserialize`+`Debug`+`Clone`+`Default` expandido em toda compilação). Adicionar `WANTED` ao `emit/mex.rs` | −8.500 | baixo |
| 2 | `abprops.rs`: 2.664 constantes, 16 lidas (`props::WATCHED`). `WANTED` + mover o teste de cobertura para o codegen | −18.500 | baixo |
| 3 | **Três famílias de traits de store Signal** para os mesmos dados: `wacore::store::SignalStore` (bytes) → `wacore_libsignal::store::*` (glue puro, `get_sub_device_sessions` retorna `Vec::new()`) → `wacore_libsignal::protocol::storage::*` (o que os ciphers usam). Apagar a família do meio; `src/store/signal.rs` + `signal_adapter.rs` têm 1.907 linhas de cola | −600 a −900 | médio (durabilidade Signal) |
| 4 | O derive `ProtocolNode` só cobre atributos, então os nós mais ricos são escritos à mão: `GroupInfoResponse` = 56 campos, `into_node` 235 linhas + `try_from_node_ref` 290, com 24× `get_optional_child_by_tag(..).is_some()`. Adicionar `#[child(tag, flag)]`, `#[child(tag, text)]`, `#[children(tag)]` ao derive | −500 a −600 | médio |
| 5 | `wacore-binary`: `Node`/`NodeRef`/`OwnedNodeRef` triplicam a API de leitura; `AttrParser`/`AttrParserRef` duplicam 200 linhas; `marshal.rs` tem cada entry point duas vezes. Trait `AttrSource` + `NodeLike` + macro de forwarders | −470 | baixo (testes de byte-igualdade existem) |
| 6 | Mocks de store libsignal re-implementados **51 vezes** em testes/benches (`MemSessionStore` definido três vezes em `send/tests.rs`). Upstream tinha `InMemSignalProtocolStore`; o fork removeu. Recolocar sob `test-util` | −1.200 a −1.400 | nenhum |
| 7 | `SignalStoreCache`: 6 locks, 5 atômicos, protocolo "fui ultrapassado?" (`removal_seq`, `recent_removals`, `UNLOCKED_COLD_READ_ATTEMPTS`) só para soltar o lock durante I/O; `flush` de 190 linhas escrito três vezes. Placeholder `Loading` por chave (single-flight) simplifica; `flush_store<S: DirtyStore>` genérico é independente e seguro | −270 (+ −400 testes) | alto (single-flight) / baixo (flush) |
| 8 | `events.rs`: `EventKind`, `Event` e `Event::kind()` são três cópias manuais da mesma lista de 78 variantes | −90 | baixo |
| 9 | Superfície pública morta confirmada (zero uso no workspace e no bridge): `messages.rs` ×7 fns, `reporting_token.rs`, `usync.rs::parse_get_user_devices_response*`, `stanza/call.rs::build_{transport,relay_latency,heartbeat}`, `PlaintextContent`/`DecryptionErrorMessage` no libsignal, `define_simple_node!`/`define_empty_node!` | −1.000 a −1.200 | baixo (semver minor) |
| 10 | `history_sync.rs` tem duas implementações de extração de message-secret (walker manual de ~700 linhas + fallback de decode completo) sem teste diferencial. Medir; se ganho < 2×, apagar o walker | −80 a −700 | médio |
| 11 | 40 dos 69 `impl IqSpec` têm `type Response = ()` com `parse_response` idêntico; três toggles de grupo escritos à mão ao lado de uma macro que faz exatamente isso | −250 | baixo |
| 12 | Helpers de parse existem três vezes com semânticas diferentes (`optional_u64_attr` erra em número inválido; `AttrParserRef::optional_u64` devolve `None` silencioso). `GroupParticipantDetails::from_node` re-escaneia atributos duas vezes após `finish()` — 20k scans extras num grupo de 10k participantes | −110 | baixo |
| 14 | 99 pares `#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]` idênticos → um attribute macro em `wacore-derive` | −100 | nenhum |
| 15 | `waproto`: 752 mensagens geradas, ~140 nomeadas em qualquer lugar. Poda por fecho transitivo a partir de raízes `WANTED` no `build.rs` (que já reescreve o descriptor) | a medir | médio |

### 2.3 whatsapp-rust — periféricos (apêndice `core-peripheral.md`)

| # | Achado | Δ | Risco |
| --- | --- | ---: | --- |
| 1 | **VoIP é 85,6k linhas** (39,7k código, 45,9k testes) dentro dos dois crates publicados, e compila para nada num build default; o bridge não ativa nenhuma feature voip. O codec MLow (20k linhas, porte símbolo-a-símbolo do `smpl_audio_codec` da Meta) é autocontido, tem `build.rs` próprio que roda em **todo** build de `wacore` (incluindo wasm/ESP32) e embarca **16 MB de vetores de teste no pacote publicado** (`wacore` 0.7.0 = 22,1 MiB; 7,5 MiB comprimido). Extrair `wacore-mlow`, `wacore-voip`, `whatsapp-rust-voip`; sinalização (`reject_call`, `Event::IncomingCall`, `stanza::call`) fica no core | pacote 22 → ~6 MiB; sem build script no default | médio |
| 2 | `src/client/voip.rs` (156 gates) e `src/handlers/call.rs` (119 gates) somam 275 dos 284 `cfg(feature = "voip-runtime")` de `src/` porque sinalização não-gateada e mídia gateada dividem o arquivo. `CallHandler::handle` é **uma função de 980 linhas** com 39 braços. Dividir no gate | −270 atributos; gates 284 → ~15 | baixo |
| 3 | `wam-catalog`: 132k linhas geradas para servir 9 eventos; `cargo check` leva 24 s e roda em todo leg `--workspace` do CI. Emitir structs só para `WANTED`, resto como dados `const EVENTS: &[EventDef]` | −110k a −125k | baixo |
| 4 | `sqlite_store.rs`: o loop de retry está desenrolado à mão **oito vezes** ao lado de `with_retry` (27 usos), e **divergiu** (backoff sem teto e warn a cada tentativa em `put_identity_for_device`) | −350 | baixo |
| 5 | `sqlite_store.rs`: 20 `pub async fn *_for_device` + 20 forwarders de trait de uma linha; `SharedSqlite::store_for_device(id)` já modela "store ligado a um device" | −300 a −400 | médio (API pública) |
| 6 | `CallRegistry`: 99 métodos públicos, 26 são gêmeos `_if_current` com um `.filter(generation)` a mais | −300 | baixo |
| 7 | `voip/facade.rs`: quatro tipos Drop-guard de teardown com os mesmos campos e o mesmo `Drop`; três `start()` de 225–268 linhas com o mesmo pré-voo | −150 a −250 | médio |
| 10 | Matriz de features: `voip-encoded`, `tokio-native`, `voip`, `metrics` (raiz) **não gateiam nada**; `client-lifecycle` ⇔ `plugins` na prática, mas custa 72 gates; cada alias é um leg a mais de `cargo hack --each-feature` | −2 legs de CI | baixo |
| 11 | Dependências: `examples/voip-cli` é membro do workspace e linka `cpal`/ALSA em todo `--workspace --all-features`; `base64` duplicado (0.22 vs 0.23); `scopeguard` com 2 usos | −1 crate, CI mais leve | baixo |
| 12 | `HttpClient` tem seis métodos e dois deles (`execute_streaming`/`execute_upload`) são **síncronos bloqueantes num trait async**; só `ureq-client` implementa; 11 mocks de `HttpClient` e 14 de `Transport` espalhados | −250 | baixo |

### 2.4 whatsapp-rust-bridge — Rust (apêndice `bridge-rust.md`)

O bridge tem 25,3k linhas Rust; ~6.650 (26%) podem sair, a maioria movendo **política** para o core, onde qualquer outro binding (Python, C, uniffi) a reaproveitaria.

| # | Achado | Δ bridge | Δ core | Onde a correção mora |
| --- | --- | ---: | ---: | --- |
| 1 | `js_backend.rs` (1.824) implementa os 81 métodos de `SignalStore`+`AppSyncStore`+`ProtocolStore`+`MsgSecretStore`+`DeviceStore` sobre três callbacks `get/set/delete`. Só ~200 linhas são JS; o resto é política de storage (self-index, expiry scan com TOCTOU, JSON do `Device`) que `in_memory.rs` (2.781) re-deriva sobre `HashMap`. → `wacore::store::kv::KvBackend<S: KvStore>` no core; `InMemoryBackend` vira `KvBackend<HashMapKv>` | −1.450 | +1.400 / −2.000 | core |
| 2 | ~24 `result_types` são cópias campo-a-campo de structs do core que não derivam `Serialize` (`GroupMetadataResult` ~50 campos + conversor de 91 linhas; `MemoryDiagnosticsResult` ~55 campos + 84 linhas de `x: d.x as f64`). → `derive(Serialize)` com `rename_all = "camelCase"` nos tipos de resultado do core | −900 | +60 | core |
| 3 | `signal_records.rs` + `legacy_session.rs` = 1.000 linhas de DTOs + `From`/`TryFrom` para tipos do core sem serde. → serde nos tipos do core sob o mesmo feature `legacy-session-interop` | −850 | +40 | core |
| 4 | `errors.rs` classifica erros do core em 11 kinds andando `source()` com downcast de 8 tipos; mapeia `IqError` **duas vezes** porque o core mantém `whatsapp_rust::request::IqError` e `wacore::request::IqError` como gêmeos; 753 das 1.574 linhas são testes de que o bridge re-derivou certo o que o core quis dizer. → `ErrorChainExt::classify() -> ErrorClass` no core (já existe para timeout/transport) | −650 | +400 | core |
| 5 | Gate de reconexão (`Parked`, `withdraw_parked`, `online_committed`) existe porque o modelo de cancelamento do core é "drope o future", que nenhum host FFI consegue. → `Client::reconnect_gate()` com `wait() -> Result<(), Withdrawn>` e `withdraw_all()` | −180 | +120 | core |
| 6 | 174 métodos exportados × 6–10 linhas da mesma cadeia (`parse_jid`, `online().await?`, `.await.map_err(BridgeError::from)`); 58 `map_err` redundantes onde `?` já converte; `pin/archive/mute_chat` duplicam a cadeia inteira nos dois braços do `if` | −400 a −600 | — | bridge |
| 7 | Seis cópias de "pegar função JS, aguardar maybe-promise, mapear erro" entre os adapters `js_*`; um objeto de config malformado chega como `kind: "internal"` (que o AGENTS.md chama de resposta errada) | −150 | — | bridge |
| 8 | `uploadEncryptedMediaStream` re-implementa o driver de upload do core (failover de host, refresh de auth, `?resume=1`) e ainda **bufferiza o stream inteiro num `Vec`**; o core tem `upload_media_with_retry` e `Client::upload_stream<S: UploadSource>` | −170 | 0 | bridge |
| 9 | `device_props.rs`/`client_profile.rs`: cópia do enum `PlatformType` (25 variantes) + política de merge que é regra do core | −250 | +80 | core |
| 11 | `camel_serializer.rs` (813) existe porque prost serializa snake_case e `serde_wasm_bindgen` não renomeia. 60% da razão de existir é uma linha em `waproto/build.rs` (`type_attribute(".", "#[serde(rename_all = \"camelCase\")]")`). Também: três caches de interning de chave JS fazendo um trabalho, e `parse_jid_fast` rodando em **toda string serializada, inclusive corpo de mensagem**, para decidir se interna | −350 | +30 | core |
| 12 | `audio.rs`, `image_utils.rs`, `sticker_metadata.rs` (923 linhas, 4 deps) nunca foram publicados (fora de `default`) e são conveniências de consumidor | −923 | — | consumidor |
| 13 | `runtime.rs`: o throttle de spawn (`SPAWN_BATCH_SIZE=16`) existe porque "centenas de workers por chat disputam um semáforo de 1 permit e cada release acorda TODOS" — thundering herd no fan-out de offline-sync do core, corrigido na camada errada | −70 | +20 | core |

Performance no boundary (não é tamanho de código): `js_to_node` e `stream_upload_via_js` usam o padrão `Uint8Array::from(...).to_vec()` / `resize+copy_to` que `js_bytes.rs` existe para evitar; `downloadMediaStream` "stream" é um chunker sobre um buffer inteiro porque o core não expõe `download_to_writer` (que `wacore::download::DownloadWriter` já tem); `enqueue` faz `try_send` num canal de 16.384 e **descarta o evento** com um `warn!` no overflow.

### 2.5 whatsapp-rust-bridge — TS, codegen, build (apêndice `bridge-ts.md`)

| # | Achado | Δ | Risco |
| --- | --- | ---: | --- |
| 1 | `codegen/src/proto_gen.rs` (785 linhas) é um segundo gerador de tipos proto **morto**: nenhum script o invoca, só funciona com clone irmão, zero testes, compilado e lintado em todo CI | −785 | nenhum |
| 2 | Codec gerado ~12% maior do que qualquer consumidor pode usar (`fromPartial` 7,3k + `create` 2,3k linhas, ausentes da `.d.ts` publicada) e `proto-namespace.ts` embrulha os 755 codecs **eagerly no import**. `docs/proto-codec-memory.md` mediu o namespace lazy por tipo em −1,9 MiB por processo e terminou com "não implementado aqui"; `benches/codec-memory/equivalence.mjs` (481 linhas) já prova a equivalência | −9.600 geradas; −110 KB de bundle; −1,9 MiB RSS | baixo / médio |
| 3 | `codegen/src/main.rs` (2.355) re-implementa o modelo de dados do serde e do `WireEnum` raspando fontes do core com `syn` — e **já divergiu**: `TSIFY_STRUCTS` pula cinco nomes que não existem mais em `result_types.rs`, então cinco tipos do core são silenciosamente omitidos de `generated_types.rs`. → `ts-rs` atrás de feature `ts` no core; o bridge consome o `.ts` exportado | −4.000 | médio-alto |
| 4 | Quatro parsers textuais independentes de `whatsapp.ts` (`gen-protobufjs-dts.ts` com compiler API, regex em `gen-ts-proto.ts`, `proto-wire-type-guards.ts`, `benches/slice.ts`) quando o `FileDescriptorSet` já está em mãos e é percorrido duas vezes com recursão duplicada | −150 | baixo |
| 5 | `proto-namespace.encode` ignora os `fns` capturados e re-resolve por string a cada chamada; `REGISTRY` tem 31 entradas, 26 idênticas ao fallback; `decode` faz `Object.defineProperty(toJSON)` por raiz decodificada, e o baileyrs paga um `hydrate` de árvore inteira **por mensagem** justamente para desfazer isso | −35; −1 walk por mensagem | baixo |
| 6 | Arquivos mortos em `ts/` (`index.d.ts` stale, `macro.ts`/`macro.d.ts`, `proto-types.ts`) com steps de build que só existem para apagar a saída deles | −30 | nenhum |
| 7 | Pipeline: strings shell de cinco comandos, `tsc` compilando 81k linhas geradas só para `rm -rf` a saída, dois lockfiles (`bun.lock` nomeia o workspace `whatsapp-binary-protocol`), scripts `bench*` apontando para arquivos apagados, `types` depois de `import` em `exports` | +40 / −6 fragmentos | baixo |
| 8 | Testes: `offlineClient()` copiado em 8 arquivos e `rejection()` em 10 enquanto `tests/helpers.ts` existe; constantes privadas de `wire-info.ts` espelhadas à mão em dois testes; `camel-long.test.ts` testa seu próprio espelho e nunca chama o bridge | −220 | nenhum |
| 10 | Mensagem inbound é **prost-decodificada no core → re-encodada pelo bridge (`wire_batch.rs:511`) → ts-proto-decodificada no JS**. A re-codificação é custo puro de transporte e perde fidelidade de bytes do peer. → core guarda o plaintext decifrado (`InboundMessage { raw: Bytes }`, campo aditivo) | −20 bridge / +15 core | médio |
| 11 | `benches/codec-memory` (1.945 linhas) é uma investigação concluída com recomendação não implementada | −1.945 | nenhum |
| 12 | `ws` importado em `tests/helpers.ts` e no exemplo sem estar declarado (funciona só pelo shim do Bun); três tabelas de alias `Adv*` inconsistentes (a do `.d.ts` é type-only, então `proto.AdvDeviceIdentity.decode()` não tipa em lugar nenhum) | — | baixo |

### 2.6 baileyrs (apêndice `baileyrs.md`)

Mais de 60% do código não-proto do baileyrs é tradução entre **três formas do mesmo dado**: DTO do bridge → DTO "canônico" → DTO Baileys.

| # | Achado | Δ baileyrs | O que o bridge precisa expor |
| --- | --- | ---: | --- |
| 1 | `Bridge/schema.ts` (1.175) + `types.ts` (769) + `primitives.ts` existem porque os eventos-objeto do bridge são inconsistentes com os próprios wire-batches do bridge: JID como struct vs string; chaves snake_case vs camelCase; timestamp RFC-3339 (com parser manual no baileyrs) vs unix; `Duration` como `{secs,nanos}`; `ReceiptType` em PascalCase porque `#[serde(from = "String")]` desliga o rename (mapa de 26 entradas com as duas grafias); `.action` tipado como `any`; `pair_success.id` declarado `Jid` mas é string. `adaptGroupAction` é um `switch` de 170 linhas fazendo `not_announce → notAnnounce` | −1.500 a −1.700 | Um shape de evento único: camelCase, JID string, unix seconds, `ReceiptType` correto, tipos de action exportados |
| 2 | `run()` devolve `void`, então o baileyrs re-deriva "o socket morreu" de padrões de eventos: `terminal-close.ts` espelha `ConnectFailureReason::should_reconnect()`, `DISPATCHERS.disconnected`/`streamError` têm casos especiais, `terminal-close-reporter.ts` (158 linhas) tem watchdog de 60 s, `logout()` conta claims, `bridge-client-owner.ts` (206) é uma quarta máquina de estados | −300 a −400 | `run(): Promise<TerminalReason>` que resolve exatamente uma vez |
| 3 | Hot path de mensagem aloca três objetos intermediários por mensagem (`hydrate` → `CanonicalMessage` de 18 campos → `WebMessageInfo` + `Long`); `adaptMessage`/`adaptMessageParts` (60 linhas) é código morto porque o bridge nunca emite `message` como evento-objeto | −120; −2 alocações/msg | nada (fusão local) |
| 4 | History sync faz três passagens e um evento sintético falso antes de chegar em `messaging-history.set` — quatro cópias dos metadados, duas dos arrays, nos maiores payloads do sistema | −80; −1 cópia de grafo/batch | nada |
| 5 | Três camadas de proto sobre o codec do bridge: `proto-types.d.ts` (19,6k) → `Compatibility/proto-runtime.ts` (898, o facade realmente mínimo) → `WAProto/index.d.ts` (14k, 790 KB copiados verbatim do Baileys upstream, único motivo de `protobufjs` ser dependência de **runtime** com zero imports em `src/`) | −150 src; −900 KB publicados; −1 dep runtime | schema compacto de campos como JSON; hook decode-onto-prototype |
| 6 | `(await ctx.getClient()).x(...)` aparece **119 vezes** em `Socket/*.ts`; `privacy.ts` são nove métodos idênticos; um `forward(ctx, method, {map, check})` + tabelas | −250 a −300 | — |
| 7 | `Socket/index.ts` (1.032) mistura lifecycle, init e ~25 métodos de feature inline; cinco mutexes construídos por socket que nada usa | ~0 (moves) | — |
| 8 | ~820 linhas de maquinaria Signal/retry exportada e nunca ligada ao socket (`messageRetryManager: null`; `identity-change-handler` cujo evento é `noop`; `parseAndInjectE2ESessions` sem caller; media-retry HKDF em JS enquanto o socket usa `client.requestMediaReupload`) | −100 agora / −820 se a API permitir | — |
| 9 | `legacy-store/device.ts` e `codecs/basic.ts` re-implementam em TS os formatos de bytes persistidos do core (`noise_key` como array de 64 números, LTHash de 128, `tc_token`, `device_list`, `lid_mapping`) — quando para sessões e sender keys o bridge **já** expõe codecs neutros (`importLegacySessionRecordV1`, `decodeSenderKeyRecordComponents`), que é o padrão certo | −350 | `encodeDeviceRecord/decodeDeviceRecord` + pares para os cinco stores JSON |
| 10 | `exports["./lib/*"]` torna **todo módulo interno público** (cada refatoração acima é semver-visível); `@bufbuild/protobuf` sem uso; segundo TypeScript pinado só para `audit-core.ts` | — | — |
| 11 | `scripts/compatibility/` (4,5k linhas): `audit-core.ts` e `type-contracts.ts` respondem a mesma pergunta; `KNOWN_WIRE_GAPS` duplica `NOT_ENCODED_FIELDS` do fuzz; `check-layer-boundaries.ts` faz grep em checkouts irmãos `../whatsapp-rust` — lint cross-repo que pertence ao CI daqueles repos | −300 a −500 | — |
| 12 | Sete `makeHarness` e cinco `makeCtx` copiados verbatim entre testes | −200 | — |
| 13 | `__fuzz__/harness/divergence.ts` (1.796) tem 25 entradas de registro e 1.000 linhas de matcher que duplicam `compare.ts` | ~0 (moves) | — |

Quick wins de performance: `Socket/index.ts:943` e `Utils/messages.ts:1032` copiam **todo** Buffer de upload/download quando uma view zero-copy basta; `useBridgeStore.set` copia os dois lados para comparar igualdade enquanto `setMany` já usa `Buffer.compare`.

---

## 3. Visão transversal: o que está na camada errada

### 3.1 O caminho de uma mensagem inbound hoje

```
peer bytes ──Signal──▶ core: prost decode ─▶ Arc<wa::Message> em InboundMessage
                       bridge: message_encode_into (re-encode prost)  ◀── custo puro
                       bridge: wire batch (string table, packed header)
                       JS:     ts-proto decode (whatsapp.ts)
                       JS:     Object.defineProperty(toJSON) por raiz
                       baileyrs: hydrate (re-parenta a árvore inteira)   ◀── desfaz a linha acima
                       baileyrs: adaptBridgeMessageWire → CanonicalMessage (18 campos)
                       baileyrs: canonicalMessageToWAMessage → WebMessageInfo + Long
                       baileyrs: emitMessageUpsert
```

Sete transformações, das quais três são pares que se anulam (re-encode/decode, `toJSON`/`hydrate`, canonical/WAMessage). O maior custo evitável isolado é o par re-encode/decode; o segundo é o `hydrate`.

Outbound: `encodeProto('Message')` em JS → `waproto::codec::message_decode` no bridge → core re-encoda para o Signal. Aqui a decodificação dupla é inerente ("JS é dono do modelo de objeto"), mas a resolução por string em `encodeProto` a cada chamada não é.

### 3.2 Quantas vezes cada contrato é declarado

| Contrato | core | bridge | baileyrs | Total |
| --- | --- | --- | --- | --- |
| Shape de `GroupMetadata` | `features/groups.rs` | `GroupMetadataResult` + conversor de 91 linhas + `.d.ts` gerada | `Bridge/types.ts` canonical + `Compatibility/group-metadata.ts` → `Types/GroupMetadata.ts` | **5** |
| Taxonomia de erro | 39 enums `*Error` (13 com shape idêntico) + `IqError` gêmeo | 11 kinds via downcast de 8 tipos + `classify!` de 19 enums | `Boom`/`DisconnectReason` + `terminal-close.ts` | **3 tabelas que podem derivar** |
| Lista de eventos | `EventKind` + `Event` + `kind()` (3 cópias à mão) | `WhatsAppEvent` union gerada por scraper `syn` | `AdapterMap` (58 entradas, 26 `noop`) + `DISPATCHERS` | **5** |
| Ação de grupo (45 variantes) | `GroupNotificationAction` | `.d.ts` gerada | `CanonicalGroupAction` (camelCase) + `adaptGroupAction` 170 linhas | **3** |
| JID | `Jid` struct | evento = struct `{user,server,...}`; resultado = string `user@server` | `asJidString` em todo adapter + `jidStr` duplicado | **2 representações no mesmo boundary** |
| Codec protobuf | prost (`waproto`) | ts-proto 81k linhas + `proto-types.d.ts` 19,6k | `proto-runtime.ts` + `WAProto/index.d.ts` 14k copiado do upstream | **3 codecs, 4 declarações de tipo** |
| Formato persistido do `Device` / stores JSON | `Device` serde + `in_memory.rs` | `js_backend.rs` (JSON + side-channel de `account`) | `legacy-store/device.ts` re-escreve o JSON à mão | **3** |
| Política de reconexão / terminalidade | `ConnectFailureReason::should_reconnect()` | `Parked`/`withdraw` | `terminal-close.ts` espelha a tabela do core | **3** |

### 3.3 Regras violadas, em cada direção

**Política do core implementada no bridge** (deveria descer): backend KV com self-index e expiry (`js_backend.rs`), classificação de erros (`errors.rs`), gate de reconexão cancelável, merge de `DevicePropsOverride`, driver de upload com failover, throttle de spawn compensando thundering herd do offline-sync, serialização camelCase de prost (`camel_serializer.rs`), serde dos records Signal legados.

**Política do core/bridge implementada no baileyrs** (deveria descer dois níveis ou um): terminalidade do run loop, formatos de bytes de `Device`/`sync_key`/`tc_token`, normalização de JID/timestamp/duração/receipt-type, `RECONNECTABLE_CONNECT_FAILURE_REASONS`.

**Assunções do consumidor vazando para baixo** (o bridge não deve conhecer o baileyrs): `audio.rs`/`image_utils.rs`/`sticker_metadata.rs` são conveniências do Baileys (`generateWaveform`, `extractImageThumb`, EXIF de sticker) e nunca foram publicadas; `Defaults::{Skip,Keep,KeepPresent}` no `camel_serializer` são semântica do protobufjs. O primeiro grupo deve sair; o segundo é legítimo como decisão do boundary JS, mas deve ficar isolado em ~150 linhas, não em 813.

**APIs do core que só existem por causa do bridge**: `legacy-session-interop` (correto como feature; o que falta é serde nos tipos, não a feature), `danger-skip-cert-chain-verify` (correto), `js` em `wacore` (só encaminha `getrandom/wasm_js`, que o bridge já seta direto — remover).

### 3.4 Arquitetura-alvo

Princípio: **cada regra em um só lugar, e esse lugar é a camada mais baixa que a entende.** O bridge vira um transportador de ~15k linhas; o baileyrs vira um adaptador de shape Baileys de ~20k.

```
whatsapp-rust (core)
  + serde (camelCase, u64→f64, i64→string) nos tipos de resultado, records Signal, DevicePropsOverride
  + ErrorChainExt::classify() -> ErrorClass (11 classes)
  + Client::reconnect_gate() { wait() -> Result<(), Withdrawn>, withdraw_all() }
  + Client::run() -> TerminalReason; ConnectFailureReason::should_reconnect() já existe
  + wacore::store::kv::KvBackend<S: KvStore>  (InMemoryBackend = KvBackend<HashMapKv>)
  + InboundMessage.raw: Option<Bytes>  (plaintext decifrado, aditivo)
  + Client::download_stream / upload_stream já existe
  + waproto/build.rs: rename_all = "camelCase" + serialize_with 64-bit/bytes sob feature `js-serde`
  + ts-rs sob feature `ts` para os payloads de evento (substitui o scraper syn do bridge)
  + Event/EventKind/kind() de uma macro
  − mex_operations/abprops/wam-catalog só WANTED; VoIP em crates próprios; família de traits libsignal do meio

whatsapp-rust-bridge
  = adapters JS finos (JsKvStore ~250 linhas, js_fn.rs, transport, http, crypto, time)
  = BridgeError como shape de fio + From<ErrorClass> de 30 linhas
  = um shape de evento (camelCase, JID string, unix seconds), eventos e resultados pela mesma serialização
  = wire batches (mantidos; uma StringTable só)
  = codec ts-proto sem fromPartial/create, namespace lazy por tipo, tipos gerados a partir do descriptor
  − js_backend policy, result_types espelhados, signal_records/legacy_session DTOs, errors.rs classify,
    Parked, camel_serializer (→ ~150 linhas), upload loop, device_props merge, audio/image/sticker,
    codegen/main.rs + proto_gen.rs, benches/codec-memory

baileyrs
  = shape Baileys: makeWASocket, forward() + tabelas, DISPATCHERS direto de WhatsAppEvent['type']
  = proto-runtime.ts como único facade (schema compacto vindo do bridge)
  = await client.run() como única fonte de "socket terminou"
  − Bridge/types.ts, ~80% de schema.ts e primitives.ts, terminal-close.ts, WAProto/index.d.ts (790 KB),
    protobufjs em runtime, legacy-store byte formats, exports["./lib/*"]
```

### 3.5 Estimativa consolidada

| Camada | Δ linhas (código) | Δ artefato publicado |
| --- | ---: | --- |
| whatsapp-rust | −30k a −35k geradas (mex, abprops, wam-catalog) + −5k a −7k código + ~25k testes movidos | `wacore` 22 → ~6 MiB; sem `build.rs` no default; VoIP fora do caminho crítico de clippy/doc |
| whatsapp-rust-bridge | −6,6k Rust (−26%) + −11k TS geradas + −4,7k codegen/bench | −110 KB bundle; −1,9 MiB RSS/processo; −4 deps |
| baileyrs | −4k a −4,5k (−20%) | −900 KB de `.d.ts`/schema; −1 dep runtime; hot path 3 → 1 alocação/msg |

---

## 4. Roteiro sugerido

Ordem por razão de valor/risco. Cada fase é independente e entregável sozinha.

**Fase 0 — mecânica, risco zero (1–2 dias, todas as camadas)**
1. Mover `mod tests` inline: core (8 arquivos, ~23k linhas), `wacore` (`signal_cache`, `iq/groups`, `history_sync`, `stanza/call`, `usync`), bridge (`wasm_client.rs` −1.556), `plugins/mod.rs`.
2. `WANTED` em `emit/mex.rs` e `emit/abprops`; `wam-catalog` como dados + `call_sites.rs` só no teste `parity`.
3. `wacore/Cargo.toml`: `exclude = ["src/voip/mlow/testdata/*"]`.
4. Apagar código morto listado (`wacore` achado 9; bridge `proto_gen.rs`, `ts/{index.d.ts,macro.*,proto-types.ts}`, `benches/codec-memory`; baileyrs `adaptMessage`, `@bufbuild/protobuf`).
5. `outputPartialMethods=false` no ts-proto; `REGISTRY` de 5 entradas; `encode` usa `fns`.
6. Fixtures de teste compartilhadas nas três camadas (`InMemProtocolStore`, `TestClientBuilder`, `tests/helpers.ts`, `socket-harness.ts`).
7. Os 58 `map_err(BridgeError::from)` redundantes; `js_fn.rs`; `js_bytes::to_vec` nos dois sites; sete loops de retry do sqlite → `with_retry`.

**Fase 1 — core expõe o que os bindings precisam (release minor do core; 1–2 semanas)**
1. `Serialize` camelCase nos tipos de resultado, `ResourceReport`, records Signal, `DevicePropsOverride`.
2. `ErrorChainExt::classify()`; unificar os dois `IqError`.
3. `reconnect_gate()`; `run() -> TerminalReason`.
4. `KvBackend<S: KvStore>`; `InMemoryBackend` reescrito sobre ele (fixture pinando o esquema de chaves byte a byte).
5. `InboundMessage.raw`; `download_stream`.
6. `waproto/build.rs`: `rename_all = "camelCase"` sob feature.
Depois disso o bridge apaga ~4.500 linhas em uma passagem.

**Fase 2 — bridge emite um shape só (release major do bridge)**
1. Eventos via a mesma serialização dos resultados: camelCase, JID string, unix seconds, `ReceiptType` correto, action types exportados, `pair_success` corrigido.
2. Namespace proto lazy por tipo; tipos `.d.ts` do descriptor; schema compacto JSON exportado; codecs de `Device`/stores JSON.
3. `ts-rs` no core substitui `codegen/main.rs`.
Depois disso o baileyrs apaga `Bridge/types.ts`, 80% de `schema.ts`, `terminal-close.ts`, `WAProto/index.d.ts`.

**Fase 3 — estrutura interna do core (sem mudança de API; contínuo)**
1. VoIP em `wacore-mlow`, `wacore-voip`, `whatsapp-rust-voip`; dividir `client/voip.rs` e `handlers/call.rs` no gate.
2. `Client` em sub-structs por domínio; campos de módulo único para structs de estado.
3. Derive `ProtocolNode` com filhos; `AckOnlyIq`; `AttrSource`/`NodeLike` no `wacore-binary`.
4. Apagar a família do meio de traits libsignal.
5. `FeatureError` unificado; `BotBuilder` sobre `ClientBuilder`; resolvedor PN⇄LID único.
6. `SignalStoreCache` single-flight (só com a matriz de chaos verde).

---

## 5. O que foi verificado e não precisa ser re-verificado

- Locks `std` mantidos através de `await` no core: nenhum em produção.
- Alocações no hot path de recebimento/envio do core: já limpas; os itens restantes são `to_string()` de JID para chaves de log/cache.
- Gates de feature fora do subsistema dono: dentro do teto documentado em `subsystem_boundary.md`.
- Sqlite: sem N+1; batch usa `IN (...)`; `pool.get()` dentro de `spawn_blocking` é o shape correto do r2d2.
- A divisão facade/engine/driver/registry/session do VoIP é principiada (um `CallPhase`, engine dono do estado de mídia). A duplicação é **dentro** das camadas, não entre elas.
- `wire_batch.rs` do bridge não duplica protobuf nem `wacore-binary`; é framing de eventos. Usar `wacore-binary` ali seria errado.
- Transport trait (`send/disconnect/resource_report`) é do tamanho certo para os três implementadores.
- O padrão "layered socket" do Baileys upstream não existe no baileyrs; a composição por `make*Methods(ctx)` é boa.
