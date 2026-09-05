# MLOW wasm oracle migration

Branch: `mlow-wasm-vectors`. The request of 2026-09-04 authorized completing
all remaining derivation, packaging, CI and retirement work.

## Implemented and verified locally

- [x] Correct stale documentation, DTX stream history and VAD counts.
- [x] J/S decode for 60 and 120 ms, with identical packet and PCM hashes.
- [x] Explicit SET/GET DTX-off proof over the full 110-packet corpus.
- [x] VAD routing (110 packets) and probabilities (330 frames), bit-exact.
- [x] Live wasm front-end, pitch, LSF quant, VUV, wire parameters/range coder,
      excitation/gennoise, HP and harmonic postfilters, with Rust consumers.
- [x] Correct the tuning/precision differences exposed by those tests.
- [x] Replace large JSON/RAW fixtures with CBOR/zstd and RAW/zstd; retain
      independent C auditors with an immutable, executable archive recipe.
- [x] Reproduce all 11 derivations twice and check every artifact against locks.
- [x] Add tool capture-matrix CI and consumer regeneration/test CI.
- [x] Validate 2261 wacore tests (131 MLOW), workspace clippy, voip-mlow clippy,
      and 17 oracle library tests plus tool workspace clippy.
- [x] Review and commit the tool in two parts; publish `feat/mlow-derived-oracles`.

## Final integration completed

- [x] Complete the consumer commits and publish the CI branch.
- [x] Confirm remote tool/consumer CI executions and record their results.

The detailed investigation is in `agent_docs/mlow_derivation.md`.
Canonical corpus contracts, counts and commands are in
`wacore/src/voip/mlow/testdata/PROVENANCE.md`. The derivation lock is local at
`tools/oracle-core/specs/mlow.lock.json`; Git dependencies are pinned by
`Cargo.lock`.

The tool review also closed two migration hazards: it now checks the old
capture's pin and removes unresolved selectors, which derive rejects before
instantiation. No capture depends on an ephemeral `/tmp` artifact.

TOC retains its small auditor/writer tests by the recorded design decision;
spact is no longer parked. There are no remaining DSP derivation or S-decode
items hidden behind the earlier parked labels.

First remote runs found workflow integration issues, not codec drift:
tool J/S tests and derivations passed, but artifact upload excluded the
hidden evidence directory; the consumer's nested tool checkout inherited
nightly-only rustflags. Both configurations were corrected. A fresh nested local checkout now builds
with stable and re-derives/verifies every artifact successfully. Remote reruns passed, including evidence upload.

## Completion evidence

- Tool CI: https://github.com/oxidezap/unwasm/actions/runs/33938645892
- Consumer CI: https://github.com/oxidezap/whatsapp-rust/actions/runs/33938854330
- All 131 MLOW tests passed together after compacting the C auditors.
- The final PCM length assertion requires 960 samples for all 110 packets;
  its targeted test passed. No false DTX-short exception remains.
- Final testdata: approximately 9.1 MB vs 15.8 MB originally.
- Tool and consumer work are committed and published in their task branches.

No implementation, derivation, packaging or first-CI item from the requested
scope remains open. The linked runs validate the implementation; subsequent
documentation receipts and the stronger PCM length assertion are kept in the
same branch and checked by its workflow.

## Migração cargo xt — 2026-09-05

Os 13 scripts Python/Bash foram substituídos por tarefas Rust, com os
consumidores/workflows atualizados e utilitários compartilhados com unwasm.
A equivalência dos hashes e das regras de CI foi verificada sem alterar os
valores esperados dos fixtures.

### Progresso da migração Rust

- Os 13 arquivos Python/Bash próprios foram substituídos por cargo xt.
- Workflows chamam tarefas Rust; decisões de features, gates, release,
  relatórios, espera do mock e instalação de símbolos também estão em Rust.
- Metadados de features, os três descritores/sidecars e os relatórios de
  tamanho foram comparados byte a byte com os scripts antigos e coincidiram.
- Auditores C e todos os artefatos wasm passaram nas verificações nativas.
- A guarda de biblioteca C desatualizada foi preservada; a referência foi
  reconstruída para validar também o caminho de geração.
- A primeira versão usou o pin `6043ff49a2e37667a9a7d65ff2bcc5ea9d140c00`;
  a separação de responsabilidades abaixo substitui esse arranjo.
- 131/131 testes MLOW passaram; cargo-deny e actionlint passaram.
- Os cinco testes das tarefas e clippy do workspace inteiro com warnings
  negados passaram. A verificação dos fixtures passou novamente com o pin final.
- CI remoto concluído: [rederivação J/S, utilitários Rust, comparação de todos
  os fixtures e 131 testes MLOW](https://github.com/oxidezap/whatsapp-rust/actions/runs/33945391416).
- O CI do produtor também passou nas duas capturas:
  [unwasm J/S](https://github.com/oxidezap/unwasm/actions/runs/33945252397).

Migração encerrada nos commits `249c0250472e4431a203b9a8ad76a3331d8d56bf`
(consumidor) e `6043ff49a2e37667a9a7d65ff2bcc5ea9d140c00`
(pin compartilhado/produtor). Nenhum trabalho desta migração permanece aberto.

## Desacoplamento dos oráculos — 2026-09-05

Responsabilidades finais:

- `unwasm`: decompilador, análise e runtime gerado independentes de WhatsApp.
- `whatspec`: descoberta, transporte, locks e restauração verificável de JS/WASM.
- `whatsapp-rust`: host Wasmtime do WhatsApp, diagnósticos, specs, receitas,
  montagem MLOW e fixtures.

O oráculo completo, seus testes/exemplos, as specs e os documentos MLOW foram
movidos para `tools/` e `agent_docs/` deste repositório. O executor usa
`unwasm-core` (`8f798470e9f8c22a9ef40780eee302894fe7bdce`) como dependência
Git somente no tooling; ele permanece fora de `default-members`. O `whatspec`
ganhou a crate `wa-store`, aprovada com o workspace, para expor
locks/restauração sem puxar os extratores.

O audit ampliado ao tooling encontrou quatro advisories no Wasmtime 41 que o
gate anterior, limitado ao pacote runtime, não enxergava. `wa-store` também foi
repinado em `10a66fdaab616c9ef6b3f5e6b197bdddfabc142a`, com ureq 3.4,
rustls 0.23.43 e a provider OxiTLS/RustCrypto 0.3.0. O oráculo passa a
usar Wasmtime 48.0.1, a versão estável atual, e o workflow audita explicitamente
o grafo de `tools/xtask` além do grafo de produção.

### Verificação final do desacoplamento

- `cargo tree` confirma que os grafos normais de `whatsapp-rust` e `wacore`
  não contêm Wasmtime, oracle, unwasm, wa-store ou wa-fetch.
- Oito capturas foram restauradas por `wa-store` com nome, tamanho, magic e
  SHA-256 verificados; J/S foram rederivadas do zero nas 11 execuções.
- A lock antiga e a movida são estruturalmente idênticas, exceto pelos hashes
  das specs cujos caminhos mudaram. Todos os oito CBOR descompactados e todos
  os streams permaneceram byte-idênticos; só o envelope zstd foi reemitido.
- Wasmtime 48 passou 117 testes do oráculo; 27 casos lentos continuam
  explicitamente ignorados. As tarefas passaram 6 testes, os utilitários 3 e
  o codec passou 131/131 testes MLOW.
- Clippy do workspace com warnings negados, actionlint e os gates cargo-deny
  separados para runtime/tooling passaram.
- CIs remotos: [unwasm desacoplado](https://github.com/oxidezap/unwasm/actions/runs/33982898581)
  e [whatspec/wa-store + TLS atualizado](https://github.com/oxidezap/whatspec/actions/runs/33983145320).

CI remoto do consumidor concluído: [oráculo Wasmtime 48, tarefas, rederivação
J/S, comparação dos fixtures e 131 testes MLOW](https://github.com/oxidezap/whatsapp-rust/actions/runs/33983916097).
[Supply-chain](https://github.com/oxidezap/whatsapp-rust/actions/runs/33984858936)
também concluiu verde com o novo audit de tooling. O job semver informativo
continuou reportando as sete quebras já presentes na API protobuf gerada; ele
não envolve os membros de tooling e permanece deliberadamente não bloqueante.
Nenhum trabalho desta separação permanece aberto.

## Revisão das PRs — 2026-09-05

A rodada de revisão reforçou as fronteiras que tornam a derivação reproduzível:

- `whatspec` pagina todos os assets de release, valida URLs com parser, limita
  tamanho de cada wasm e memória de dicionário XZ e preserva erros de escrita.
- `unwasm` mantém o decompilador utilizável offline; o xtask de captura fica em
  workspace separado e aponta para `wa-store` em
  `10a66fdaab616c9ef6b3f5e6b197bdddfabc142a`.
- `whatsapp-rust` verifica tamanho e SHA das capturas conhecidas antes da
  instanciação, serializa o teste VoIP, exige a inicialização dos workers e
  libera temporários embind também quando a codificação falha.
- O host passou a rejeitar strings C inválidas e descritores já fechados; as
  leituras amostradas da memória compartilhada usam a mesma carga atômica das
  demais leituras do host.
- O job MSRV testa as crates publicadas diretamente no Rust 1.94, e o job MLOW
  baixa as capturas antes de executar também os testes de integração do oráculo.
- Falhas ao carregar uma captura deixam de virar skips; workers recusam limites
  de stack incompletos, patches sobrepostos são erros e todos os sinks de marker
  registram seus hits. O check dos auditores C também compara o manifesto de
  proveniência inteiro com os arquivos empacotados.
- O host recusa `EM_ASM` desconhecido, descreve corretamente o preopen WASI e
  preserva largura/sinal dos inteiros registrados pelo embind. A inspeção
  estática também recusa exports com índices fora do espaço de funções.

Os pins finais usados pelo tooling são `unwasm`
`8f798470e9f8c22a9ef40780eee302894fe7bdce` e `whatspec`
`10a66fdaab616c9ef6b3f5e6b197bdddfabc142a`.

## Fundação do oráculo E2E de mídia — 2026-09-05

- [x] Captura declarativa e limitada de callbacks de áudio/vídeo, incluindo
  payload, ordem, sequência e timestamp.
- [x] Persistência verificável em manifesto + payloads binários, com tamanho e
  SHA-256 conferidos na leitura e limpeza de records obsoletos.
- [x] Comparador exato disponível em `cargo xt oracle compare-media`.
- [x] Testes de captura real pelo host wasm, limites, tamper, comparação e
  persistência.
- [x] Documento com as etapas para codec, RTP, SRTP/WARP, H.264/FU-A e receive.

A infraestrutura está pronta para receber specs derivadas dos callbacks dos
módulos pinados. Os fixtures E2E de chamadas completas serão adicionados quando
os ABIs de callback de áudio e vídeo forem identificados por seletor e prova de
execução; nenhum índice ou layout foi presumido nesta etapa.
