# Derivação dos vetores MLOW direto do wasm (Tier 1)

> **Objetivo**: aposentar os ~16 MB de fixtures JSON em
> `whatsapp-rust/wacore/src/voip/mlow/testdata/` (gerados pelo fork C
> `edgardmessias/opus_mlow@84b076e`, metade com receita perdida) e derivar os
> vetores do próprio módulo VoIP embarcado, via `oracle derive`, de forma
> determinística e reproduzível. Ground truth = WA Web, como manda o AGENTS.md.
>
> **Status em 2026-09-04: implementação e validação local concluídas.**
> Decode J/S em 60/120 ms byte-idêntico; DTX-off confirmado por SET/GET;
> front-end, VAD/spact, pitch, LSF quant, parâmetros/range coder,
> excitação/gennoise e postfilters derivados do stream wasm e testados.
> 11 derivações reproduzidas em duas execuções independentes; locks e CI
> por captura implementados. Auditores C preservados em formato compacto.
> Consumidor: 2261 testes wacore aprovados (131 mlow; 2 ignorados existentes),
> clippy workspace e clippy voip-mlow limpos. Tool: 17 testes lib e clippy
> workspace limpos. CI remota da ferramenta (J/S) e do consumidor aprovadas;
> execução e evidências registradas na seção 7. Escopo concluído e commitado.

## 1. O módulo pinado

Tudo roda contra estes bytes exatos; spec com hash divergente é recusada
antes de instanciar (como `cargo xt oracle fetch`).

| Arquivo | Tamanho | SHA-256 |
|---|---|---|
| `wasm/JgwtTQVeWPm.wasm` | 10.650.934 B | `97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db` |

`S_ivh1PriOA.wasm` (10.856.103 B) é a segunda captura pinada do whatspec; `carry`
(`oracle carry`) resolve a renumeração quando migrarmos.

## 2. Mapa do cluster `wa_opus.cc` (índices nesta captura)

Levantado com `oracle abi --body` + strings `__func__`. **Slots ≠ índices**:
`call_indirect` usa slots; `abi --index N` lê função N (confundir os dois
custou uma tarde — ver §5.7).

| Índice | Slot | Função |
|---|---|---|
| 6655 | 5057 | `opus_alloc_codec` (precisa factory+pool globais — hoje zerados) |
| 6658 | 5061 | `opus_codec_open(codec, attr)` — o alvo atual |
| 6666 | 5065 | `opus_codec_encode` |
| 6670 | 5068 | `opus_codec_decode` / `_normal` |
| 6668 | 5066 | `opus_codec_encode_with_secondary` (único chamador direto do encode) |
| 6659 | — | `wa_opus_get_application_mode(x)`: `x<3` → assert + retorna 2048; senão `tabela[1045776+x*4]`. Chamado com `*(attr+1116)` |
| 6693 | 8598 | `init_media_endpt_and_codecs` (endpoint inteiro — rota pesada, evitada) |
| 9981 | 8581 | bootstrap pj (`pj_init`-like, 0 args, retorna 0; mas NÃO registra a factory opus) |
| 9848 | 8583 | lazy init do mutex do pj_log (0 args) |
| 11920 | — (sem slot) | init preguiçoso que chama 6693; **não chamável** via `call_table` |

Vtable que `opus_alloc_codec` grava no state de 824 B (lido do corpo de
#6655 — são **slots**):

```text
state+0=pool  +4=0  +8=5053 +12=5052 +16=5051 +20=5050 +24=5049 +28=5048
+32=5047 +36=5046 +40=5045 +44=5044 +48=5043 +52=5042 +56=5041
+60=5040 +64=5039 +68=5038 +72=5037 +76=5036 +80=5035 +84=5034 +88=5033 +92=5032
+96=5031 +100=5030 +816=1        (layout antes de opus_codec_open)
codec+8=state  codec+12=1377164 (factory)  codec+20=1312704
```

Slots → funções (via `oracle abi --slot`): 5053→#10670, 5052→#10671,
5051→#10684, 5050→#10681, 5039→#10694, 5031→#10651, 5030→#10562.
#4313(a,b) = `pj_pool_calloc_no_trace(a, 1, b)`.
**Correção importante**: o `5039` do alloc vai em **state+64**, não +0 —
`store32(l0-64, 0, 5039)`. **state+0 = pool** (de
`f6320_pjmedia_endpt_create_pool`, forjado aqui) e **state+92 = 5032**
(este faltava na primeira forja).

Layout do `attr` (lido do corpo do open): +0 = sample rate (logado `%d`),
+4 = ?, +8 = ?, +26 (u16) = frame ms (`state+108 = rate*ms/1000`),
+36 (u8) ≠ 0 → caminho MLow (`"Using MLow codec!"`, `state+116=1`,
`state+117=256`), +65/+72 (u8), +1116 = índice p/ #6659 (**≥ 3**; usamos 3).
Layout do state: +8/+12/+16 = índices p/ `call_indirect`,
+100/+104/+108, +112, +116/+117, +119/+124 (guardas u8, 0 passa),
+120 (retorno do create), +240=100, +250(u8)=9, `memset(state+192,0,64)`.

## 3. A tool: `oracle derive`

`oracle derive --spec spec.json -o out/` — arquivos novos/alterados
(ver `git status`):

- `tools/oracle-core/src/derive.rs` (novo): schema da spec, resolvedor,
  executor, `manifest.json` (sem timestamps: mesmos bytes + mesma spec =
  mesmos bytes de saída).
- `tools/oracle-core/src/abi.rs`: `table_slots_of` (slot→func inverso),
  `function_count`.
- `tools/oracle-cli/src/main.rs`: subcomando `Derive`.
- `tools/oracle-core/README.md`: uma linha no índice de comandos.
- Deps novas: `serde`/`serde_json`/`sha2`/`hex` (já estavam no lock).

Invariantes: **recusa > chute** (pin, seletor ambíguo, slot ambíguo,
hex malformado) e **single-thread** (sem `--threads`: scheduler é o que
torna runs irreprodutíveis).

Ops: `instantiate`, `run_ctors`, `log_ring`, `malloc`, `fill`, `write`
(hex), `write_file`, `store` (u32 de registrador → guest; linka structs),
`call` (export), `call_table` (seletor resolvido), `read`, `read_u32`,
`dump_data` (fatia de data-segment, estático), `assert_sha256`,
`print`, `log` (host logs + engine ring), `calls` (trace gravado +
`read_cstr`), `watch` + `markers` (compõe com `oracle instrument`),
`add` (registrador + offset; fins de bloco, campos pós-base dinâmica),
`at` (offset em todos os ops de ponteiro).

Limite conhecido: imports com handler `func_wrap` (ex.
`env::loggingCallback_js_sync`) **não entram no trace gravado** — o op
`calls` não os vê. Observabilidade nesses casos = `instrument` + `markers`.

## 4. Specs (`tools/oracle-core/specs/`)

| Spec | Papel | Estado |
|---|---|---|
| `mlow_probe.json` | resolução do cluster + round-trip malloc/write/read | ✅ verde, self-check travado (`8ddaed4c…`), 2 runs byte-idênticos |
| `mlow_factory_probe.json` | lê globais da factory pós-ctors | ✅ prova que factory vem zerada (motivou forjar) |
| `mlow_open.json` | **canônica**: TLS + mutex + pj_init + smpl_init + forja + pool, chama open | ✅ `rc 0`, `smpl_flag 1` |
| `mlow_encode.json` | canônica + encode frame 0 + decode (round-trip 1 frame) | ✅ `encrc 1` pacote 137 B, `decrc 0` energia 1.147× |
| `mlow_3frames.json` | **vetor**: 3 frames c/ `read_reg` (pacotes exatos) + decodes + `assert_sha256` | ✅ pacotes 137/139/138 B, energias 1.147/1.260/1.158×, manifest idêntico |
| `mlow_110frames.json` | **vetor stream**: 110 frames (2706 steps) + 220 asserts | ✅ TOC 11/3/96, 14–147 B; correl 0.9628 vs libopus; DTX 17–19+62–69 |
| `mlow_120ms.json` | **vetor 120 ms**: attr frame-ms 120, 8 pacotes + 16 asserts | ✅ todos TOC `0x58`, 218–277 B; energia 1.147–1.240; correl **0.9874** vs `ref_120ms` commitado |
| `frame0.raw` / `frames3.raw` / `synth_mic.raw` / `synth120_head.raw` | slices/cópia de `synth_mic.raw` (shas registrados) | fixtures de entrada |

## 5. Diário trap-driven (cada tentativa → evidência → conclusão)

1. **open com structs zerados** → trap OOB em `0xfffffffc` dentro de
   `6658→9856→14839→9851→9847→13580` (`pj_log_get_tag` com tag table NULL).
   Conclusão: log pj precisa de init.
2. **`log_ring` não bastou** (mesmo trap). Conclusão: o anel do engine ≠
   anel do pj_log.
3. **`_emscripten_tls_init` + `call_table 9848`** → saiu do log, novo trap:
   `uninitialized element` (indireta p/ slot vazio). Conclusão: progrediu;
   `state+8` zerado = índice nulo.
4. **Leitura do corpo do open**: `state = *(codec+8)`; `call_indirect`
   usa `*(state+8/+12/+16)` como índice. Conclusão: forjar a vtable.
5. **Vtable extraída de #6655** (tabela no §2), forjada via 24 `write`s +
   `store` p/ `codec+8` (op `store` e campo `at` criados p/ isso) →
   **open retorna `70004` limpo**. Conclusão: indiretas resolvem; falha é
   lógica adiante.
6. **Prova por marcadores**: cópia instrumentada
   (`oracle instrument JgwtTQVeWPm --calls-in 6658`) + ops `watch`/`markers`
   (criados p/ isso) → `markers: [200000, 200002, 200003, 200004, 200005]`
   = log MLow, log rate, `4313`, `6659`, **log `"encoder_init error"`**.
   Conclusão: a 2ª indireta (slot 5052 = #10671) retornou ≠ 0.
7. **Slots ≠ índices**: `abi --index 5052` mostrava SRTP (função 5052);
   `abi --slot 5052` = #10671 (o encoder-init real). Forja estava correta
   o tempo todo.
8. **`attr+4 = 1`** (de `opus_default_attr` #6653) + `codec+20 = 1312704`
   (correção de misread) → saiu do `encoder_init error`, novo trap:
   **fuel esgotado** em `pj_pool_allocate_find` (#9884←#9883←#9887←#4313).
   Conclusão: caminho do pool com pool inválido = loop sobre lixo; `refuel`
   é inútil (budget é 20B).
9. **`pj_init` (#9981) retorna 0 mas globais da factory seguem zeradas.**
   Conclusão: registro da factory opus mora no fluxo #6693 (endpoint),
   não no bootstrap pj.
10. **Decompile decide**: `unwasm decompile --only` (lê Rust em vez de
    adivinhar dataflow) revelou: `state+0` = pool (de `f6320`), vtable
    correta tem **+64=5039 e +92=5032** (misread anterior), `f4313` =
    calloc, `#10670(attr+4)` = tamanho do estado do encoder
    (`attr+4=1` → 322008 B), `#10671` = init que zera/valida.
11. **Pool mínimo forjado** (flags 0 = sem mutex, 1 bloco, sem growth;
    layout de `pj_pool_alloc_from_block` #9882): `calloc` funciona →
    `open` retorna **0**. slab 1 MB (322008 + 304 + 304 + 48×1920 ≈ 415 KB).
12. **O flag `-112`**: `conv_convert` (#10740) falha se `*(1719408)==0`
    (`SMPL_ENC_NO_GLOBAL_DATA` — o mesmo do harness C!). Init = #10747,
    alcançável pelo trampolim void **#10566 (slot 7242)**. Chamado →
    flag = 1.
13. **Encode funciona**: `encrc 1` (1 = sucesso, convenção pjmedia, NÃO
    contagem de bytes), pacote no buffer de `out+8`, tamanho em
    `outframe+16` (29). Pacote `50 24 70 1f …` (TOC `0x50`!), idêntico
    entre runs e entre specs com layouts de malloc diferentes.
    (Armadilha no caminho: com `out+8 = 0` o pacote vai p/ o endereço 0
    — escreve sem trapar; sempre apontar `out+8` p/ buffer próprio.)
14. **Decode falha (fronteira atual)**: `opus_decode` → `70001` (core
    negativo). Via marcadores: `#10681` toma o ramo `#10719` (pois
    `buf+292 = 1` pós-open — mesmo no fluxo real), `#10719` roda centenas
    de iterações DSP e chama `conv_convert` (#10740), que **entra mas
    não chega à primeira chamada** → falha no prólogo de validação.
    Próximo: mapear quais args do `conv_convert` vêm zerados (l10 = frame
    ms? l14/l12 de `*(l1+0/+4)`?).
15. **Encode FECHADO**: relendo o dataflow exato, `l5 = state+112 * samples
    * 2` (não usa `in+8`!) e o indireto recebe `l6 = *(in+8)` como p1 =
    **ponteiro do PCM** (`#10681` lê `s16` de p1). Layout medido:
    frame `+8` = ponteiro, `+16` = tamanho; `state+112` = canais (1).
    Com `in+8 = pcm`: pacote **137 B** `50 e5 63 8c …`, determinístico.
    (No caminho: probe com `write_file` no endereço 960 provou que o
    encoder lê `[p1, p1+2·960)` — mas escrever em endereço baixo
    **envenena dados estáticos** (o pacote de 134 B saiu com tabelas
    corrompidas). Nunca escrever abaixo do heap em runs canônicos.)
    `encrc 1` = sucesso (convenção pjmedia); tamanho em `outframe+16`;
    pacote vai p/ `out+8` (com `out+8 = 0` ele vai p/ o endereço 0 —
    escreve sem trapar!).
16. **Decode FECHADO**: `#6671(state, infr, outfr, p3, p4, …)` exige
    `p3 ≥ p4·state+112·2`; `#6670` passa `p3 = arg2` do decode e
    `p4 = *(state+108) = 960`. Logo **arg2 do decode = tamanho PCM de
    saída (1920), não o tamanho do pacote** (137 → falhava). Com 1920:
    `decrc 0`, PCM de energia 1.147× a entrada. **Round-trip completo**:
    synth frame → 137 B TOC `0x50` → PCM, byte-idêntico entre runs.
17. **Multi-frame**: 3 frames (0 e 50 loud, 90 quiet) numa instância —
    3 pacotes **distintos** (`50e5…`, `50e4…`, `5034…`), frame 0 idêntico
    ao do run single-frame (histórico não vaza p/ trás). Codec com
    estado evolui como o real. (Probe em /tmp — spec com loops ainda
    não existe; derive é single-shot, N frames = passos repetidos.)
18. **Vetor 3-frame travado**: op `read` ganhou `Len` (`$reg` ou literal;
    literais antigos intactos) p/ pacotes exatos sem saber o tamanho
    antes; `tools/oracle-core/specs/mlow_3frames.json` = open + 3×(encode→`read_reg`→decode)
    + `assert_sha256` nos 6 outputs. Pacotes 137/139/138 B, PCM
    1.147/1.260/1.158×, manifest byte-idêntico entre runs.
19. **Vetor 110 travado** (`tools/oracle-core/specs/mlow_110frames.json`, 2706 steps):
    110 pacotes (14–147 B, TOC 11/3/96) + 110 PCM, 220 asserts, manifest
    idêntico. Correlação **0.9628** vs libopus `useSmpl`; frames codificados inativos
    17–19+62–69 em DTX-off (11× `0x10` casa exato).
20. **Integração whatsapp-rust**: par derivado adicionado, ainda sem commit, como
    `wacore/src/voip/mlow/testdata/wasm_derived_frames.json` (23 KB) +
    `wasm_derived_ref.raw` (211 KB); teste
    `quality_tests::wasm_derived_frames_decode_to_wasm_reference` ✅
    (média > 0.90, pior > 0.70, energia 4x, DTX curtos pulados, quietos
    por envelope); receita no `PROVENANCE.md`. Clippy/fmt limpos.
    Nada existente foi tocado — substituição dos 16 MB fica p/ a revisão.
21. **Fase 1, VAD fechado (worktree `mlow-wasm-vectors`)**: âncora por
    constantes SILK (`konst 7788` + `konst -29322` → só #10772; #10773 é
    feature-extraction p/ ML, não o VAD clássico). Decisão: não replicar
    `spact` — vetor de **routing** (`{frame,toc,cav,len}`, cav 0 só p/
    `0x10`) derivado dos 110 pacotes + contadores internos
    (`state+152` ativo / `+156` inativo, observados). No caminho, uma
    divergência real: pacote 16 (`0x12`, hangover) — primeira leitura
    dizia inativo, Rust dizia ativo; `0x12` conta ativo (routing do
    decoder + semântica de hangover) → **110/110 bit-exato** em
    `smpl_vad::vad_matches_wasm_routing`. `spact` segue no JSON do C.
22. **Persistência entre captures (`oracle migrate` + prova S)**:
    `migrate --spec --from --to --new-sha --new-size` carrega hints por
    fingerprint, re-ancora por string, preenche `expect_fingerprint`,
    e **recusa** o resto (trampolim `smpl_init` #10566: 2 instr, abaixo
    do floor — recusado honestamente). Prova `JgwtTQVeWPm→S_ivh1PriOA`
    (8741 carry 1:1): open/encode/decode/pj/mutex migrados
    mecanicamente; smpl_init re-derivado à mão (carry 10747→11036 +
    trampolim #10855 slot 7457). Descoberta: **vtable são slots e slots
    renumeram** — re-ler o corpo do alloc na captura nova (slots S:
    +8=5280 … +100=5257, factory 1398252) em vez de carregar.
    **Encode em S: rc 1, 137 B, byte-idêntico ao pacote de J.**
    Resultados persistem sem ajuste; índices, não.
23. **Aberto: decode em S** — trap `integer divide by zero` em
    #10932 ← #10935 ← #6963 ← #6964 ← #6962. Ferramenta nova no caminho:
    `call`/`call_table` agora anexam `markers so far` ao erro, e
    `--value` por local mediu a cadeia do zero: wrapper recebe
    (decfr, 1920) certo; 6963 recebe (state, decfr) certo; mas #10935 é
    chamado com p0 = 0 (deveria ser struct) e uma 2ª vez com (0,0,0) via
    recover (6964); #10932 recebe zeros e divide por `*(0+8)/400`.
    Ou seja: em S o wrapper desvia p/ o recover e o core nunca recebe
    structs válidos; em J vai direto ao core. `state+128` (fonte do p0
    do core) nunca foi forjado — em J o zero passa, em S não. Falta:
    ler o fluxo #6963/#6964 em S (diverge de J) e achar o campo que
    desvia p/ o recover.
24. **Histórico S-decode (resolvido pelo item 26).** Estado anterior: com
    `decctx{+8:16000,+96:0}` forjado em `state+128` o trap some e a
    síntese (#10932) roda até recursão após #11007 e falha limpo
    (marcadores `[entry,10850,10853,11007,entry,10933]`). Tentativas que
    NÃO resolveram (p/ não repetir): buffers maiores, zerar `*(96)`,
    `out+8` como capacidade vs ponteiro. Para retomar: decompilar S#11007
    (`--only 11007`, 1310 instr, prólogo lê `l0+{0,8,92,128}`) e mapear
    suas entradas contra o que #10932 passa; depois forjar. Encode em S
    segue válido e byte-idêntico — é a prova de persistência que importa.
25. **TOC: downgrade por desenho, não gap.** `parse_mlow_toc` são ~20
    linhas de bit-ops puras (sem estado, sem tabelas, sem floats):
    `f10561` (duração) e `f10650` (nº frames) mapeados no wasm confirmam
    o layout, mas derivar os 256×10 campos nada acrescenta além de
    re-confirmar shifts — o trio C-table + Go + writer-test in-repo já
    cobre (inclusive a caveat dos escapes, com teste próprio). Se um dia
    fizer sentido: composição está em `toc.rs`, helpers no wasm.

26. **Decode em S resolvido; causa na forja, não na síntese.** Leitura nova
    de S#6950 mostrou que o flag que pula decoder-init está em `attr+1200`
    (J#6658: +1176). A alocação antiga tinha 1184 bytes: lia o próximo
    bloco do heap, pulava init e deixava `state+128=0`. Com 1216 bytes
    zerados, o próprio `opus_open` cria o contexto válido; não se forja
    `decctx`. A migração também precisa corrigir TODAS as escritas de slots,
    inclusive escritas duplicadas em +64/+92 que restauravam valores de J.
    `tools/oracle-core/specs/mlow_encode_s.json`: rc/encrc/decrc = 0/1/0; pacote 137 B
    (`e4b19307…`), PCM 1920 B (`7b3552b8…`), iguais a J.
    `cargo xt mlow specs` gera `mlow_110frames_s.json` e
    `mlow_120ms_s.json` a partir das specs J, conservando os hashes esperados.
    **110-frame S executado: 220/220 hashes aprovados**, sem trap.
    **120 ms em S executado: 16/16 hashes aprovados** (8 pacotes + 8 PCM).
27. **DTX-off confirmado por controle explícito.** `mlow_dtx_off.json`
    chama J#10684 (slot 5051), SET 4016 com 0, depois GET 4017 e exige
    retorno 0 e valor 0. Todos os 220 hashes do stream anterior passam.
    Logo o par anterior já reproduz DTX-off; a afirmação anterior de
    "DTX ligado" estava errada. No teste Rust, decodificar TODOS os pacotes
    antes de selecionar os inativos mantém o histórico igual ao do wasm.
    Rodada local: 143 testes selecionados passaram (inclui 22 fora do módulo
    mlow); havia dois warnings de `n_quiet`, removidos em seguida.
28. **Gennoise derivado; integração em andamento.** J#10777 identificado
    pelos chamadores #10726/#10737, ABI de 10 argumentos e layout observado
    de NoiseGenerator (11 floats + 3 i32; seed em +52). Entrada: 1320 casos
    sintéticos preservados do corpus C, sem seus valores esperados, em
    `tools/oracle-core/specs/mlow_inputs/gennoise_input.bin`. Receita executável:
    O replay inicial foi substituído por `cargo xt mlow verify`; os dados
    atuais vêm diretamente dos snapshots do stream wasm.
    **1320 chamadas executadas**, com 2640 saídas de ruído/estado. Primeiro
    caso: diferença máxima de ruído vs C = 3.7253e-9 e seed bit-idêntica.
    **Integração concluída: testes Rust contra wasm e C passaram (1320 casos cada).**
    Não é captura de intermediários do stream J: é replay das mesmas entradas
    sintéticas através da folha original do wasm. O auditor C ainda está completo; pode ser reduzido após a revisão da cobertura.
29. **Tooling para folhas sem slot.** `call_function` adiciona apenas exports
    para seletores resolvidos; corpos/índices/tabelas permanecem intactos.
    Argumentos f32 são explícitos e `assert_reg` verifica status/configuração.
    Teste novo cobre memória f32, preservação das seções, trap OOB e recusa
    de status divergente/float como ponteiro. **5/5 testes derive passaram**.
    Specs agora exigem os retornos de open/encode/decode, não apenas imprimem.

30. **Empacotamento sem perda validado.** JSON volumoso convertido em CBOR
    (inteiros e f64 preservados) + zstd, leitores de teste ciborium/ruzstd.
    **121/121 testes mlow passaram** após trocar os consumidores. Os JSON
    originais ainda não foram removidos. Manifest registra hashes e tamanhos.
31. **Divergência de versão no front-end.** J#10736 chama #10749 (window),
    #10797 (LPC, reg=5e-7) e expande A num loop com gamma bits `0x3f7f9db2`
    (=0.9984999895). C 84b076e/Rust usam 0.9999. Nova derivação separa
    `A_before_bwe` para medir o kernel e registra a política do chamador
    explicitamente; não ajustar o oráculo para fazê-lo concordar. Efeito
    sobre o encoder Rust ainda em investigação.

32. **Captura de rotinas inlined.** `capture_memory` observa um local i32
    antes de uma instrução da função original e copia um span no host. Não
    escreve no guest; exige número exato de hits e limita o total a 64 MiB.
    A memória é lida pelos accessors existentes (sem novo unsafe). Necessário
    porque pitch/signal-mode estão inlined em J#10736, sem função exportável.
    `cargo xt mlow spec signal` registra entradas/saídas do VUV nas
    instruções 4280/4584. **330 frames capturados com os 220 hashes originais
    aprovados**. Integração Rust desses vetores ainda em andamento; testes
    unitários adicionais da ferramenta também pendentes.

33. **A captura revelou divergência de tuning, não erro de snapshot.** O
    teste signal-mode falhou no frame 0: Rust 0.14687705 vs wasm 0.120677054
    (delta 0.0262). J#10736 usa bias bits `0xbe051eb8` = -0.13; C/Rust
    usavam -0.1038. O encoder Rust foi atualizado para bias -0.13 e BWE
    0.9985, ambos lidos do chamador wasm. Os auditores C continuam usando
    explicitamente o perfil antigo (-0.1038/0.9999), sem editar seus outputs
    nem afrouxar tolerâncias. **5/5 testes focados passaram:** front-end wasm/C, signal-mode wasm/C e
    gennoise wasm. Qualidade e golden do encoder ainda precisam revalidar.

34. **LSF quantização: 330/330 aprovados.** `cargo xt mlow spec kernel` captura J#10804
    na entrada e no retorno implícito (op1099), com A, RD weight, seleção de
    tabelas, centroides condicionais e outputs. O teste Rust roda os wrappers
    completos `lsf_quant`/`lsf_quant_cond`, encadeando qlsf anterior, e compara
    índices exatos e qlsf com a tolerância anterior. **219 casos condicionais**.
    A observação conservou os 220 hashes do stream.
35. **Pitch: extraído, divergência em investigação.** 330 registros de
    LTP/F2/estado anterior e saída extraídos nos ops1691/4266 de J#10736.
    O teste passou frame 0 e falhou na correlação do frame 1; não reduzir a
    tolerância sem explicar a diferença. Estado e tuning sendo conferidos.
36. **Postfilters extraídos.** `cargo xt mlow spec postfilter` captura HP em
    J#10726 ops5550/5820 (330 frames, estado 1332B) e harmônico em
    ops7728/8368 (110 pacotes, estado 9260B). Os 220 hashes continuam iguais.
    Integração Rust em andamento; o harmônico também tem tuning diferente:
    feedback wasm 0.4 vs C/Rust 0.4734, força com bits `0x3f36872b` vs
    C/Rust 0.6438. Confirmar diferenças no teste antes de alterar.
37. **Golden re-medido após tuning do encoder.** 23040 amostras, RMS
    0.153243, pico 0.455261, clip 0; PCM checksum `f535e428a5a1e641`,
    frames `8af5211b8d4e38da`. Constantes atualizadas apenas após os testes
    independentes de front-end/signal-mode aprovarem o novo perfil.
    Suite completa de qualidade ainda pendente.

38. **HP postfilter: 330/330 aprovados.** Os estados de entrada foram
    semeados no Rust e as saídas concordam abaixo de 1 LSB i16.
    O harmônico falhou no primeiro pacote; os dois parâmetros de tuning
    lidos de J#10726 (feedback=0.4, strength=0.713) foram aplicados ao Rust,
    mantendo perfil antigo explícito apenas no auditor C. Teste em execução.
39. **Decode de parâmetros extraído.** J#10758 gera 330 LbQuantParams
    e contextos de range coder. A primeira comparação de pulsos leu o campo
    denso legado (sempre zero): o wasm escreve posições/magnitudes esparsas
    em +760/+1080 com contagem em +1400. A montagem foi corrigida para
    reconstruir esses pulsos, sem mudar o decoder Rust. Reteste em andamento.
40. **Captura validada na ferramenta.** Teste mínimo agora verifica
    memória antes/depois da mesma instrução, transporte dos bits f32,
    equivalência da memória observada vs não observada e recusa de hits
    ausentes. 5/5 testes derive aprovados; gates completos ainda pendentes.

41. **Wire decode completo: 330/330 aprovados.** LSF, interpolação, pulsos
    (posições/magnitudes/contagens), ganhos ACB/FCB, lags/contorno, energia Q14
    e os 11 words do range decoder concordam exatamente. A segunda falha
    era do teste: passava CAV=1 ao decoder de pulsos mesmo para pacotes
    inativos; corrigido para usar o TOC, como o runtime já fazia.
42. **SPACT saiu do parked: 330/330 bit-exatos.** O `vad_results` do Rust
    concorda exatamente com as probabilidades capturadas pelo signal trace.
    Não é mais necessário depender do prefixo C para essa propriedade.
43. **Gennoise e excitação agora partem do stream wasm.** `cargo xt mlow spec gennoise`
    observa somente o chamador decoder J#10726 (ops2631/2632), evitando
    misturar as chamadas do encoder. 1320 casos derivados com entradas e
    saídas originais; teste de gennoise e teste da excitação ACB/LTP ambos
    passaram, incluindo frames inativos. Substitui o replay de entradas C;
    os antigos seed files/geradores de replay podem sair depois da receita final.
44. **Postfilter harmônico aprovado** após adotar feedback=0.4/strength=0.713
    lidos do wasm (110 pacotes); HP também aprovado (330 frames). Os auditores
    C mantêm a parametrização antiga explicitamente.
45. **Front-end também capturado ao vivo.** `cargo xt mlow spec fe` observa os 330
    frames, incluindo A antes e depois do loop BWE original (op1422). Nenhum
    cálculo host passa a ser parte do resultado esperado. Integração substitui
    os 40 replays C usados inicialmente; teste ainda a reexecutar.
46. **Pitch: configuração e pesos identificados.** O open forjado seleciona
    4 sobreviventes e low-complexity=1; Rust usava 24/false. O teste agora
    reproduz a configuração capturada sem reduzir o orçamento de qualidade
    do encoder Rust. Pesos intermediários: previous=0.7 (C 0.7981), hipótese delta=0.15 (corrigida no item 47)
    (C 0.1439). Resta divergência de lag no frame 12, investigada comparando
    H/E2/sobreviventes; E2 e os blocos H usados concordam bit a bit.

47. **Pitch fechado: 330/330 aprovados.** E2 e H bit-idênticos levaram à
    seleção dos sobreviventes: o 4º era 132 no Rust e 185 no wasm, com delta
    de score em múltiplos de 0.0525. O código divide o peso por BLOCKSIZE=64,
    não 32: os fatores wasm 0.105 (coarse) e 0.0046875 (fine) correspondem
    a **delta_weight=0.3**, não à hipótese intermediária 0.15. Corrigido,
    todos os lags/contornos passaram. Auditor C 120 casos também passou com
    seus pesos antigos; dumps temporários de diagnóstico removidos do Rust.
48. **Precisão LPC corrigida e validada.** No frame 181, mesmo alimentando
    R exato do wasm, o solve Rust divergia. J#10797 promove `reg` para f64
    ANTES de somar 1; Rust somava em f32 e só depois promovia. Corrigido.
    Teste front-end ao vivo 330 casos e auditor C passaram. O teste agora
    também compara o solve sobre R exato em TODOS os casos, incluindo silêncio;
    diferenças de FFT em LPC mal-condicionado são limitadas por erro de predição
    <1 LSB i16, sem simplesmente ignorar os coeficientes silenciosos.
49. **Receita completa executada.** `cargo xt mlow verify` reconstrói e
    verifica specs geradas, valida/faz fetch dos módulos pinados, executa 11
    derivações e verifica TODOS os outputs por hash de árvore e resoluções.
    `tools/oracle-core/specs/mlow.lock.json` trava inputs, módulos, specs, seletores e outputs.
    Todas as 11 execuções passaram; os artefatos montados ficam sob `--out`.
    `.github/workflows/mlow-derive.yml` divide J/S em jobs e arquiva logs e
    manifests. O job remoto ainda não foi executado nesta rodada.
50. **Retirada dos JSON grandes em andamento.** No consumidor,
    `cargo xt mlow pack-legacy` reconstrói auditores C a partir do commit histórico
    pinado, mantendo uma amostra determinística de gennoise/param/EXC e os
    outros testes C. `cargo xt mlow regenerate` verifica o tool pin, TODOS os hashes
    da derivação e compara CBOR canônico (independente da versão do compressor).
    JSON grandes e postfilter RAW substituídos por CBOR/zstd e RAW/zstd.
    Reteste conjunto e pin final do commit da ferramenta ainda pendentes.

## 6. Receita canônica e critérios de manutenção

```sh
cargo build --release --locked -p oracle-cli
cargo xt mlow verify --out .derive-mlow
cargo test --release -p oracle-core --lib
cargo clippy --workspace --all-targets --release -- -D warnings
```

`cargo xt mlow verify` funciona de qualquer diretório. Faz fetch dos módulos pinados
quando necessário, recompõe as specs geradas e recusa drift de inputs,
seletores, specs ou de qualquer output. `--capture JgwtTQVeWPm` e
`--capture S_ivh1PriOA` são os shards usados pela CI. `--update-lock` é
somente para uma re-derivação intencional com revisão dos resultados; CI
nunca usa essa opção. As duas capturas mantêm hashes de packets/PCM iguais.

Os artefatos JSON intermediários e PCM montados ficam em `--out/artifacts`.
No whatsapp-rust, `cargo xt mlow regenerate` verifica o commit
pinado da ferramenta e o hash de `mlow.lock.json`, executa a derivação,
empacota CBOR+zstd e compara os bytes CBOR no modo `--check`. As versões do
compressor podem escolher uma representação zstd diferente; os valores
binários descomprimidos precisam ser idênticos. Os helpers de leitura Rust
são dev-dependencies e não entram no runtime.

**Cobertura primária, a partir exclusivamente de `synth_mic.raw`:**

| Spec | Prova |
|---|---|
| `mlow_110frames{,_s}` | 110 packets + 110 PCM por captura |
| `mlow_120ms{,_s}` | 8 packets + 8 PCM por captura |
| `mlow_dtx_off` | SET/GET DTX=0 e mesmos 220 hashes |
| `mlow_fe_trace` | 330 windows, espectros, R, A antes/depois do BWE |
| `mlow_signal_trace` | 330 VUV I/O + spact; 110 decisões de routing |
| `mlow_kernel_trace` | 330 pitch I/O; 330 LSF quant, 219 condicionais |
| `mlow_params_trace` | 330 parâmetros wire + estados completos de range coder |
| `mlow_gennoise_trace` | 1320 excitações/noises/estados/energias |
| `mlow_postfilter_trace` | 330 HP e 110 harmônicos, entradas/saídas/estados |

Os auditores C permanecem independentes: `cargo xt mlow pack-legacy` no consumidor
reconstrói a seleção a partir de um commit histórico imutável. Nenhum output
C foi corrigido para concordar com Rust. Onde a captura usa tuning diferente,
o teste C explicita o perfil antigo e o teste wasm verifica o perfil atual.
TOC conserva o auditor compacto e o teste do writer: a decisão do item 25
não limita a cobertura dos kernels nem é requisito pendente.

**Retomada de investigação:** decompilar os índices listados no diário com
`unwasm decompile wasm/JgwtTQVeWPm.wasm --only INDICES --bare -o leitura.rs`;
para S, usar o módulo S e os índices S. Os pontos de captura usam ordinais
zero-based dos operadores do corpo ORIGINAL (`oracle abi --body N`), antes
de inserir marcadores. Nenhuma receita depende dos antigos arquivos `/tmp`.
As strings que unwasm anota em constantes numéricas podem ser coincidências;
ABI/dataflow/chamadores e a execução, juntos, decidem a identidade do kernel.

**Revisão da ferramenta:** a migração valida também o hash da captura antiga
antes de carregar hints. Seletores que não migraram são removidos do resultado
parcial, e `derive` recusa referências ausentes antes de instanciar. Isso
impede executar acidentalmente o índice antigo sob o hash da captura nova.
A captura de memória/f32 é testada quanto à neutralidade, aos limites e à
contagem obrigatória de hits. Nenhum `unsafe` foi acrescentado.

## 7. Integração remota

Em 2026-09-04, a execução 33938093701 passou os testes e a re-derivação
das duas capturas em runners limpos. Falhou somente ao publicar evidências,
porque upload-artifact exclui diretórios ocultos por padrão. O workflow
agora habilita arquivos ocultos apenas nos paths de logs/manifests listados.
O consumidor também isola os rustflags do build stable do oracle: um clone
aninhado sob whatsapp-rust herdava as flags nightly do `.cargo/config`.

### Execuções remotas aprovadas

- [unwasm: J/S e 17 testes da ferramenta](https://github.com/oxidezap/unwasm/actions/runs/33938645892).
- [whatsapp-rust: re-derivação dos dois módulos e 131 testes MLOW](https://github.com/oxidezap/whatsapp-rust/actions/runs/33938854330).

Os problemas de infraestrutura da primeira tentativa foram corrigidos e
as novas execuções concluíram com sucesso, incluindo publicação das evidências.
A comparação PCM de 60 ms foi fortalecida para exigir 960 amostras em TODOS
os 110 pacotes. A antiga afirmação de que estes pacotes DTX decodificavam
curto era incorreta; essa checagem adicional também passou localmente.
Os arquivos grandes antigos foram aposentados, mantendo auditores C compactos;
o testdata do consumidor caiu de 15.8 MB para aproximadamente 9.1 MB mesmo
com toda a nova cobertura. Não restam itens de decode S, DSP ou spact parked.

## Migração para cargo xt (2026-09-05)

Pedido novo: substituir todos os scripts Python/Bash dos dois repositórios
por tarefas Rust. Bibliotecas compartilhadas serão somente de tooling; os
dois ambientes host continuam independentes. A validação precisa preservar
os hashes binários dos oráculos e as recusas de dados divergentes.

### Progresso cargo xt

As tarefas nativas reproduziram os hashes de árvore de todas as derivações
J/S. Somente os hashes de serialização das specs mudaram ao passar para o
writer Rust; `--refresh-spec-hashes` recusaria qualquer alteração dos módulos,
resoluções ou outputs. O consumidor confirmou CBOR e streams byte-idênticos.
Os 14 arquivos Python/Bash próprios foram removidos. Os patches diagnósticos
mantêm as recusas por captura divergente e padrão ambíguo; export-globals foi
comparado byte a byte com a implementação anterior na captura J.

A implementação Rust também passou nas verificações do consumidor sem mudar
nenhum byte CBOR ou packet/PCM. `cargo xt mlow verify --from-derived` compara
o hash da spec atual com o lock antes de reutilizar snapshots. Os utilitários
compartilhados têm testes para tipos numéricos, snapshots truncados, status de
processos e captura de arquivos sem aceitar hashes/caminhos do arquivo baixado.

Validação daquela etapa: 17 testes oracle, 6 testes das tarefas/utilitários,
clippy do workspace com warnings negados e download limpo das oito capturas.
O CI J/S também passou antes da separação final.

## Fronteiras de responsabilidade (2026-09-05)

O host Wasmtime, CLI, patches, specs, receitas e montagem MLOW agora pertencem
a `whatsapp-rust/tools`. `unwasm` fornece somente análise/decompilação genérica
por uma dependência Git imutável de `unwasm-core`. Download, autenticação,
locks e restauração são fornecidos pela crate `wa-store`, pinada do whatspec.
`xtask-support` ficou local ao consumidor para descritores e CBOR/zstd.

Todos esses componentes são membros de tooling fora de `default-members`; o
grafo de `whatsapp-rust` e `wacore` não contém Wasmtime, unwasm ou wa-store.
O workflow MLOW executa o oráculo diretamente, sem checkout ou subprocesso de
outro repositório.
