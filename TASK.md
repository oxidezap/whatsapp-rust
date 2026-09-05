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

The detailed investigation is in the sibling `unwasm/MLOW_DERIVE.md`.
Canonical corpus contracts, counts and commands are in
`wacore/src/voip/mlow/testdata/PROVENANCE.md`. Tool revision and derivation
lock are pinned by `scripts/mlow-vectors/oracle.lock.json`.

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
- `xtask-support` e o oráculo usam o commit Git
  `6043ff49a2e37667a9a7d65ff2bcc5ea9d140c00`; nenhuma dependência local resta.
- 131/131 testes MLOW passaram; cargo-deny e actionlint passaram.
- Os cinco testes das tarefas e clippy do workspace inteiro com warnings
  negados passaram. A verificação dos fixtures passou novamente com o pin final.
- A confirmação remota será registrada após a publicação desta migração.
