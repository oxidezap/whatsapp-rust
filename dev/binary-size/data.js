window.BENCHMARK_DATA = {
  "lastUpdate": 1784158589439,
  "repoUrl": "https://github.com/oxidezap/whatsapp-rust",
  "entries": {
    "whatsapp-rust binary size": [
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "908d6cffaa900f9ac48f0ed0efc8c8b6bda32c09",
          "message": "ci: track binary size with a per-PR budget gate and historical series (#859)\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-06-12T15:31:46-03:00",
          "tree_id": "d2c8cf7f0becc6054cdc14027c397cf01b703279",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/908d6cffaa900f9ac48f0ed0efc8c8b6bda32c09"
        },
        "date": 1781289590356,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 14000152,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 11856758,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 14010312,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1900856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 563215,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 105910,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170833,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37618,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 994719,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1559573,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 6158547,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 634926,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 16990,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1275789,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 37090,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 357,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e076ae7ee7b73479d5179574d465c9892f967ed0",
          "message": "perf(iq): de-monomorphize send_and_wait_iq via boxed send future (#862)",
          "timestamp": "2026-06-13T20:04:04-03:00",
          "tree_id": "17421f19076618898f286043186bd5e41a1535ec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e076ae7ee7b73479d5179574d465c9892f967ed0"
        },
        "date": 1781392129245,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 13889080,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 11751222,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 13893888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1798368,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 563550,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106101,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170833,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37618,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 994719,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1556136,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 6158431,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 634926,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 16990,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1259423,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 36874,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 357,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "556fa2d6933bbfe2b20701de6bfb22088056888e",
          "message": "perf: drop moka, use PortableCache as the sole in-process cache backend (#860)",
          "timestamp": "2026-06-13T21:09:13-03:00",
          "tree_id": "d2b12cfc734391b99b492f1edd0270613dfa489d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/556fa2d6933bbfe2b20701de6bfb22088056888e"
        },
        "date": 1781396084081,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11215448,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9213622,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11216480,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1578149,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 564473,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170833,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37618,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 994719,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1176890,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4228349,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 654146,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17617,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 645497,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 19642,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4765ebbeb8ce8e5613fb727886a5ff75eedf9d2c",
          "message": "perf(send): de-monomorphize Signal encrypt fan-out to dyn dispatch (#861)",
          "timestamp": "2026-06-13T21:08:47-03:00",
          "tree_id": "a9e9eb32f3a383aa2c9932ad4da5e049779f337f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4765ebbeb8ce8e5613fb727886a5ff75eedf9d2c"
        },
        "date": 1781396139982,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 13871096,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 11734774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 13878224,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1796159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 565215,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106101,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170833,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37618,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 994719,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1548914,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 6149978,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 654146,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17617,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1257030,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 36807,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 357,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "18cee58bae04b49ab244e764633e821a72dbee8b",
          "message": "perf(signal): pre-key the zero-salt HKDF extract for message-key derivation (#863)",
          "timestamp": "2026-06-14T10:09:47-03:00",
          "tree_id": "45aaecd3e26c1f47f0484452736ed2f86695380c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/18cee58bae04b49ab244e764633e821a72dbee8b"
        },
        "date": 1781442920588,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11217240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9215158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11216680,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1578149,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 564473,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37618,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 994719,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1176890,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4228349,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 654146,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17617,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 645497,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 19642,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "826882b68d56c8bd0879164f6a619a7a6dc0c885",
          "message": "perf(reporting-token): pre-key the zero-salt HKDF extract for token-key derivation (#864)",
          "timestamp": "2026-06-14T11:58:08-03:00",
          "tree_id": "7b6b59b3a7bb717c4fa79ace26f8cc0a6c38e7a4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/826882b68d56c8bd0879164f6a619a7a6dc0c885"
        },
        "date": 1781449433799,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11219096,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9216822,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11221008,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1578149,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 566526,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37237,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 994719,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1176890,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4228349,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 654355,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17627,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 645497,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 19642,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d8de9f4564537a77009b1f3d2b19a2702d94a6e3",
          "message": "refactor(appstate): scan instead of HashSet for index-mac dedup in the patch path (#865)",
          "timestamp": "2026-06-14T12:42:10-03:00",
          "tree_id": "9ce4b146a64176a231e10c1f60b74268ffa36f97",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d8de9f4564537a77009b1f3d2b19a2702d94a6e3"
        },
        "date": 1781452056121,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11216536,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9214326,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11216944,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1578154,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 565190,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 994719,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1176890,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4228349,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 654355,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17627,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 645001,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 19623,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "23130ca26155b36df60e7f632479a3b1d55533e0",
          "message": "perf(proto)!: shrink wa::Message ~75% by boxing inline content variants (#866)",
          "timestamp": "2026-06-14T18:43:43-03:00",
          "tree_id": "5eb171519714c059b4c41850517d2709405278fd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/23130ca26155b36df60e7f632479a3b1d55533e0"
        },
        "date": 1781473810635,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11176152,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9173238,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11175952,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1542806,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 565865,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 983774,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1178511,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4231037,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 661539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17918,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 652742,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 19857,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9cfa4dba318ea31afc7f8c87b26f79d9a24d2fd1",
          "message": "perf(appstate): index-sort dedup for large patches, O(n²) scan stays for small (#868)",
          "timestamp": "2026-06-14T20:09:52-03:00",
          "tree_id": "c0d09df31ff44c2c0b98875985fcea739c90ee2b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9cfa4dba318ea31afc7f8c87b26f79d9a24d2fd1"
        },
        "date": 1781478942748,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11189688,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9185910,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11188248,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1542806,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 569390,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 983774,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1187602,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4231037,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 666701,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18047,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 652742,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 19857,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a725074cb41d30b0c2e27c2c6a6e59111f9949b0",
          "message": "perf: drop a ~67 KiB duplicate prost decode tree + hoist a per-message traversal (#869)",
          "timestamp": "2026-06-14T21:45:23-03:00",
          "tree_id": "ce4202fd13f807f3e3203119fca94c9beab32e2c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a725074cb41d30b0c2e27c2c6a6e59111f9949b0"
        },
        "date": 1781484696937,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11089496,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9087670,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11089944,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1542046,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 556816,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1187674,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213263,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644384,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 652742,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 19857,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b23a9f7f31e504f193a696336825b033415a68bb",
          "message": "fix(retry): bound outbound resend rate per group to prevent AccountLocked (#871)",
          "timestamp": "2026-06-15T10:02:56-03:00",
          "tree_id": "9670ebe686ff2ff54e30e914e05e6837850fd91c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b23a9f7f31e504f193a696336825b033415a68bb"
        },
        "date": 1781528773309,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101048,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9098742,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11102352,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1550764,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 556816,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1190070,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644384,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 657128,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20013,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "09e3d97e1d699e5e47d487823f9c2118c6251384",
          "message": "fix(send): gate SKDM redistribution on the primary device (WA Web parity) (#872)",
          "timestamp": "2026-06-15T11:15:59-03:00",
          "tree_id": "54c588b56eac3e2a14c3932acc7c6297d0c978ef",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/09e3d97e1d699e5e47d487823f9c2118c6251384"
        },
        "date": 1781533124552,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11099704,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9097398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11098256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1549694,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 556816,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 106131,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189775,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644384,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656564,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 19997,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "faff0ba6687cb1bd86b7ce2d0dac8795197726e1",
          "message": "perf(binary): length-bucketed token lookup (tiny_map) on the encode hot path (#873)",
          "timestamp": "2026-06-15T12:48:02-03:00",
          "tree_id": "5d20d2999d703f0b99e4b7eb5987058722bd3ee9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/faff0ba6687cb1bd86b7ce2d0dac8795197726e1"
        },
        "date": 1781538799560,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11115352,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9155446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11113928,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1550208,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557801,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162554,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189933,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645561,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17680,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656893,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20004,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3a84631927d2b5ce4c43a5714060b5b1b1423058",
          "message": "perf(sqlite): skip the per-checkout SELECT 1 liveness probe (#874)",
          "timestamp": "2026-06-15T14:17:33-03:00",
          "tree_id": "2c39c103ecbea9e35eb3e461f58fe4b94e811320",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3a84631927d2b5ce4c43a5714060b5b1b1423058"
        },
        "date": 1781544122813,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11115352,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9155446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11113928,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1550208,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557801,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162554,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189933,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645561,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17680,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656893,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20004,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7f9e64433944099e5547d146bfd98aeb0c9f00c9",
          "message": "feat(example): add 🦀send <jid> <text> chat command (#875)",
          "timestamp": "2026-06-15T15:38:23-03:00",
          "tree_id": "d0e5915915dc4d627616127125dfb3d9bbabc910",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7f9e64433944099e5547d146bfd98aeb0c9f00c9"
        },
        "date": 1781548866127,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11121176,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9160950,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11122200,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1555409,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557801,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1190094,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645561,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17680,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656893,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20004,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "26aec2d46626ec3e2c3a3a914f054478c42c9ce0",
          "message": "perf(conn): reuse the shutdown listener across the read loop (#876)",
          "timestamp": "2026-06-15T17:00:06-03:00",
          "tree_id": "c657a6b229ca7f1a822206918122b4c989a88a0d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/26aec2d46626ec3e2c3a3a914f054478c42c9ce0"
        },
        "date": 1781553934599,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11120984,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9160758,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11122200,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1555250,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557801,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1190073,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645561,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17680,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656926,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20012,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0141b6b10352be2dbc51c874739fbc0b291f35c6",
          "message": "test(bench): add an in-order DM decrypt benchmark (#878)",
          "timestamp": "2026-06-15T18:13:04-03:00",
          "tree_id": "0b551bb6c8df98f21b05e67a1dd95725288fe166",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0141b6b10352be2dbc51c874739fbc0b291f35c6"
        },
        "date": 1781558146869,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11120984,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9160758,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11122200,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1555250,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557801,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 172357,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1190073,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645561,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17680,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656926,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20012,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e854e6312e9c0cbc58edc1c1018b4ecb9fb92b50",
          "message": "perf(signal): skip the rollback clone for in-order decrypts (#877)",
          "timestamp": "2026-06-15T19:05:06-03:00",
          "tree_id": "44e68dc3872a6f6f1126829c0b0ab95d0aa6746b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e854e6312e9c0cbc58edc1c1018b4ecb9fb92b50"
        },
        "date": 1781561424334,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11121880,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9161590,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11122224,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1555250,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557801,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 173174,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1190073,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645561,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17680,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 657010,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20013,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a03672ee5af62568e1960fa90e8f0ae0ecf15050",
          "message": "perf(signal): reuse the encrypt buffer instead of take + realloc (#879)",
          "timestamp": "2026-06-15T20:22:55-03:00",
          "tree_id": "11cde62fed9cf158e842dbe4f37859c28689c9e1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a03672ee5af62568e1960fa90e8f0ae0ecf15050"
        },
        "date": 1781566088721,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11122744,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9162486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11122200,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1554901,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 174558,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1190043,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645616,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17669,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 657082,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20006,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d441e5fa2d09b5aaaef966d00fa7d18e7df04a30",
          "message": "perf(signal): share the sender-key message backlog behind an Arc (#881)",
          "timestamp": "2026-06-15T21:53:32-03:00",
          "tree_id": "144f3aa97c67857ac7fd861ba3d3d415a80702c5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d441e5fa2d09b5aaaef966d00fa7d18e7df04a30"
        },
        "date": 1781571538321,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11124024,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9163510,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11122192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1554894,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 174948,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1190661,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645636,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17671,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 657120,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20008,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3e704d22e96005cbb4465e6cccfefdb6a5a7d0b9",
          "message": "feat(retry): WA Web log-level parity and retry-flow observability counters (#887)",
          "timestamp": "2026-06-17T14:57:39-03:00",
          "tree_id": "c2c1fbeaaf1dee977f8584bb38fcf1ce906059e6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3e704d22e96005cbb4465e6cccfefdb6a5a7d0b9"
        },
        "date": 1781719442498,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11124088,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9163574,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11122192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1554958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 174948,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1190661,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645636,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17671,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 657189,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20013,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a8efc6ce6238251bc29540b3c404eff440f100b1",
          "message": "perf(signal): flush the signal cache without holding the device read-lock (#888)",
          "timestamp": "2026-06-17T23:09:43-03:00",
          "tree_id": "cff9b8e83bd92670989f771ba3e2185b216dcfc9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a8efc6ce6238251bc29540b3c404eff440f100b1"
        },
        "date": 1781748824731,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11118648,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9158198,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11118096,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1552314,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 557697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 174948,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31416,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1187935,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4213206,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645636,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17671,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 657088,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20013,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 354,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d9753ed077a8f1e83441c16c67431afe3bec5792",
          "message": "chore(deps): update Cargo.lock to latest compatible versions (#890)",
          "timestamp": "2026-06-17T23:11:27-03:00",
          "tree_id": "98cd34ff5eef5cfac0d76de991acd403f6e1a2ec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d9753ed077a8f1e83441c16c67431afe3bec5792"
        },
        "date": 1781749083754,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11100216,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9139766,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11101520,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1548140,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 550531,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 162643,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 171436,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 36185,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 31043,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 916825,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 211154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33888,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6338,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1187644,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4210154,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645499,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17667,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656954,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20009,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 341,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c00e9440f112fa7d33397e8c5e76a3750ba4b301",
          "message": "chore: bump nightly toolchain to nightly-2026-06-16 (#891)",
          "timestamp": "2026-06-17T23:17:42-03:00",
          "tree_id": "f420c32ebca32672623e7337aafea9fd52f86dde",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c00e9440f112fa7d33397e8c5e76a3750ba4b301"
        },
        "date": 1781749469741,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11053656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9090422,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11052608,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1554469,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530350,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161481,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170673,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 30477,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33627,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1184604,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4198244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639462,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17664,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 651023,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20025,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 341,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "32a9cbb3fa7e87b34f2bee802765d8d7a26fafc4",
          "message": "fix(retry): dedup registration-id parsing and reject oversized payloads (#889)",
          "timestamp": "2026-06-17T23:31:12-03:00",
          "tree_id": "8e179282d1b9d6c40935bad1522d5ca31e73ed4c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/32a9cbb3fa7e87b34f2bee802765d8d7a26fafc4"
        },
        "date": 1781750179410,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11053400,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9090166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11052600,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1553974,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530619,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161481,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170673,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 30477,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33627,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1184604,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4198244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 650732,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20022,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 341,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2d8fa7f57d30f00946e711f0836fcef7c591fecb",
          "message": "refactor: move the demo binary to examples/ and make env_logger a dev-dependency (#892)",
          "timestamp": "2026-06-17T23:41:44-03:00",
          "tree_id": "25214db1b21fc1653afcba153d546769bbe981e6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2d8fa7f57d30f00946e711f0836fcef7c591fecb"
        },
        "date": 1781750951445,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11077432,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9101046,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11074153,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1499291,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530497,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161481,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170673,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 29760,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1183256,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4266139,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 650732,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20022,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 341,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "486c06de654fd8241ed5971e02e1318debe99960",
          "message": "refactor(error)!: replace anyhow in public APIs with per-domain typed errors (#893)",
          "timestamp": "2026-06-18T08:59:05-03:00",
          "tree_id": "573dc9b84a2c451d79d880be376b6167bcad9ea1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/486c06de654fd8241ed5971e02e1318debe99960"
        },
        "date": 1781784209579,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101688,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099489,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511917,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530453,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189199,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654867,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20155,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 341,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "19ef46ba00b0f5cf17ce0044259f7022add09e70",
          "message": "ci: add an all-features build job and fix the --nocapture flag (#894)",
          "timestamp": "2026-06-18T09:07:45-03:00",
          "tree_id": "37101031a2bf06ab65be8515563fa195c0d9b2f6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/19ef46ba00b0f5cf17ce0044259f7022add09e70"
        },
        "date": 1781784695832,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101688,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099489,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511917,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530453,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189199,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654867,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20155,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 341,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3414bfbd297752f268a2be8ac5f8d0ac604269fa",
          "message": "test(binary): add decoder roundtrip property tests and unmarshal fuzz target (#895)",
          "timestamp": "2026-06-18T10:26:29-03:00",
          "tree_id": "2cbf5ec48b36f552ad727cc3b8ad7cb2d868c7ab",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3414bfbd297752f268a2be8ac5f8d0ac604269fa"
        },
        "date": 1781789452281,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101688,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099489,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511917,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530453,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189199,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654867,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20155,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 349,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b53ae507ec7ce01086169db6a730da56ac34e3fd",
          "message": "refactor(send): extract tc-token lifecycle and pin/revoke actions into submodules (#896)",
          "timestamp": "2026-06-18T10:28:57-03:00",
          "tree_id": "49c56095c2b8c03de18c0a83357a8796ed3979ec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b53ae507ec7ce01086169db6a730da56ac34e3fd"
        },
        "date": 1781789580465,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101688,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099489,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511917,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530453,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189199,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654867,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20155,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 349,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9c8646979d1662617d92130c1b8aa184e3eb0691",
          "message": "perf(send): resolve group SKDM warm gate with one inner-map lookup per device (#897)",
          "timestamp": "2026-06-18T10:47:56-03:00",
          "tree_id": "21b64b1ea19428cf3f9e4c6ac03528fa2dee8886",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9c8646979d1662617d92130c1b8aa184e3eb0691"
        },
        "date": 1781790728649,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101752,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122614,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099473,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511433,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530453,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189803,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654850,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20155,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 349,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b88629b438d118db6afc4668bf8a88a4a234b76f",
          "message": "chore(deps): prune the aes-gcm dev-dep and tidy dependency declarations (#899)",
          "timestamp": "2026-06-18T12:06:14-03:00",
          "tree_id": "bb10eda4aed212df3d8f1ac5dd5ce84ae5459018",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b88629b438d118db6afc4668bf8a88a4a234b76f"
        },
        "date": 1781795535478,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101688,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122550,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099473,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511433,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530072,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189803,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654850,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20155,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "931a5d69c479ba2b08ae2106046fea6d2bc0810d",
          "message": "bench(integration): port integration benchmarks to CodSpeed (simulation + memory) (#898)",
          "timestamp": "2026-06-18T13:28:07-03:00",
          "tree_id": "b729e60e31241cd15225247258223abed2e4f6f7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/931a5d69c479ba2b08ae2106046fea6d2bc0810d"
        },
        "date": 1781800423024,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101688,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122550,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099473,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511433,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530072,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189803,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639574,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654850,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20155,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9d7e6de88205b57cc3518a38cb60ed9eeb4c7eb3",
          "message": "perf(prekeys): avoid full record decode on the pre-key upload path (#900)",
          "timestamp": "2026-06-18T15:33:52-03:00",
          "tree_id": "510c1c7bc74420fc0fc32e8e61c9e5d953933bb5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9d7e6de88205b57cc3518a38cb60ed9eeb4c7eb3"
        },
        "date": 1781807995913,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11102328,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9123254,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099497,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511585,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 530524,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189862,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639594,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 655265,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20171,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d252c7caf9cfdc4ea1380a0de719dd13ab404fd0",
          "message": "perf(prekeys): stream prekey generation to cut the connect-time peak (#901)",
          "timestamp": "2026-06-18T16:36:44-03:00",
          "tree_id": "9d88be97d877bfb3bb67c380591de275537f1a65",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d252c7caf9cfdc4ea1380a0de719dd13ab404fd0"
        },
        "date": 1781811674951,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101368,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099505,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 529703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189862,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639594,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654931,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20159,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "21aa1e207d5088378aa1217bc85d14d1cab80de3",
          "message": "bench(integration): cut CodSpeed variance with a fixed 2-worker runtime + deterministic allocator (#902)",
          "timestamp": "2026-06-18T18:31:47-03:00",
          "tree_id": "eb2556d1f96bbea227781d2ca17fd1eaaa89a373",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/21aa1e207d5088378aa1217bc85d14d1cab80de3"
        },
        "date": 1781818520689,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101368,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099505,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 529703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189862,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639594,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654931,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20159,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d9b571ef8ec47bfa40096a2a32234f16a9f5230a",
          "message": "perf(send): skip the unused DeviceSentMessage plaintext on companion-less DMs (#903)",
          "timestamp": "2026-06-18T20:22:10-03:00",
          "tree_id": "2917444cb083ce01a346c7d2282c48e14922e527",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d9b571ef8ec47bfa40096a2a32234f16a9f5230a"
        },
        "date": 1781825251314,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11101304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9122294,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11099505,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511433,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 529703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189866,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 639594,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17666,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 654965,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20160,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "afc89d84cdff61a344e858387cb6f71c33b1510a",
          "message": "perf(send): share one message encode between the reporting token and DM plaintext (#904)",
          "timestamp": "2026-06-18T21:25:52-03:00",
          "tree_id": "574be7c6ad8b9d45d33be62c15b3edb68eeb23cf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/afc89d84cdff61a344e858387cb6f71c33b1510a"
        },
        "date": 1781829077045,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11105336,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9126006,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11103617,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1512409,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 532380,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189912,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 640205,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 655030,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20162,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d9297eedde91e3f690ae0e328a68addfb83e6281",
          "message": "perf(send): share one message encode between the group reporting token and skmsg plaintext (#905)",
          "timestamp": "2026-06-18T21:48:00-03:00",
          "tree_id": "a32906c0bf38daf30454daea05baa9210169c5c4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d9297eedde91e3f690ae0e328a68addfb83e6281"
        },
        "date": 1781830495280,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11106680,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9127286,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11103601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1512779,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 533250,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189970,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 640205,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 655094,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20164,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8d4d584099d9bcb3e0dd1f6b88092c2e7897595d",
          "message": "ci(codspeed): drop the memory instrument from the integration benches (#906)",
          "timestamp": "2026-06-18T22:41:49-03:00",
          "tree_id": "2933525540bcc90d7015189e70f3e1b58987bb56",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8d4d584099d9bcb3e0dd1f6b88092c2e7897595d"
        },
        "date": 1781833521830,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11106680,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9127286,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11103601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1512779,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 533250,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212464,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 33271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6076,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189970,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4271244,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 640205,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 655094,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20164,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c3e44df2bc504fbe6df1b288aeb385b1cbf743fd",
          "message": "chore(deps): cargo update (#910)",
          "timestamp": "2026-06-19T19:51:57-03:00",
          "tree_id": "0fdb77754650999e301d39b718b80b2816c5f414",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c3e44df2bc504fbe6df1b288aeb385b1cbf743fd"
        },
        "date": 1781909961023,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11099032,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9119286,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11095353,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1511233,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 533438,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170100,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212360,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 32757,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6156,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1189552,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4266102,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 640206,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 655089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20166,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "107017350+Salientekill@users.noreply.github.com",
            "name": "Salientekill",
            "username": "Salientekill"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d8e5f73c7d1c1817d41c1e2f0df986fa91cd9d08",
          "message": "feat(groups): backfill participant phone_number from LID-PN mapping (#909)\n\nCo-authored-by: Salientekill <Salientekill@users.noreply.github.com>",
          "timestamp": "2026-06-19T20:27:50-03:00",
          "tree_id": "3bfdf29dddbeb841f0b0b6c99a0438515a72fc41",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d8e5f73c7d1c1817d41c1e2f0df986fa91cd9d08"
        },
        "date": 1781912000585,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11108792,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9128182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11107961,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1543458,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 533438,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 160405,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170100,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 37871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 897244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 212360,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 32757,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 6156,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1191551,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 4240841,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 640206,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 655089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20166,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0b4558ebb267fb18d6f39bda68b9aaf6cdb31cb7",
          "message": "perf(size): size-optimize off-hot-path crates via per-package opt-level (#912)",
          "timestamp": "2026-06-21T19:29:26-03:00",
          "tree_id": "9d6a4d924a598c811a864e53d5452b5efabd1f2d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0b4558ebb267fb18d6f39bda68b9aaf6cdb31cb7"
        },
        "date": 1782081330582,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10561336,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8533110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10559965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1543787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 536309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159587,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169999,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147222,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 895617,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 473146,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44433,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1023470,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3447898,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 638569,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17724,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 652564,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20327,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "302d4787b4b31dd5d1159422741358d4f6f90510",
          "message": "fix(atomics): use portable_atomic for 64-bit atomics + lint against std (#913)",
          "timestamp": "2026-06-22T18:42:27-03:00",
          "tree_id": "2506505bb4a0d96a54aaf5135460060fc18d93af",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/302d4787b4b31dd5d1159422741358d4f6f90510"
        },
        "date": 1782164963592,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10561336,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8533110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10559965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1543787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 536309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159587,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169999,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147222,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 895617,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 473146,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44433,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1023470,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3447898,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 638569,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17724,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 652550,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20326,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6a274da669bb249440c7dce2435be47b31a6ffca",
          "message": "feat: opt-in inbound durability hook (at-least-once delivery) (#920)",
          "timestamp": "2026-06-26T23:25:15-03:00",
          "tree_id": "b608b491a2438024730b967df11c627afb84f9d2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6a274da669bb249440c7dce2435be47b31a6ffca"
        },
        "date": 1782527437954,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10607768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8564790,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10603469,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1553407,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 536309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159587,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169999,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147222,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 895617,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 492548,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44433,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1025697,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3448259,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 640244,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17781,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 655809,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20386,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 347,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c67511b279123616e5260c8f3f5f258e979e164d",
          "message": "Merge pull request #918 from oxidezap/feat/voip\n\nVoIP 1:1 calling",
          "timestamp": "2026-06-27T21:23:13-03:00",
          "tree_id": "fe7b2b73ecd75330ad70d40a40a588cfa1559945",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c67511b279123616e5260c8f3f5f258e979e164d"
        },
        "date": 1782606532754,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10607928,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8564662,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10603692,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1556312,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 538910,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159587,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147222,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 492548,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024554,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3446169,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656506,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20409,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 476,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c57cef791b3ccbce6c23185a008305acce47b86c",
          "message": "chore(deps): update workspace dependencies to latest (#921)",
          "timestamp": "2026-06-27T22:32:07-03:00",
          "tree_id": "80a4700284e7ea59dc006a079fb099397621ccb2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c57cef791b3ccbce6c23185a008305acce47b86c"
        },
        "date": 1782610724749,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10608120,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8564854,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10603620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1556312,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 538910,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159587,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147222,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 492548,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44716,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024931,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3446076,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 656508,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20409,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 469,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "301f2e030eac2061e2794e5ac790a5153ced2d5d",
          "message": "refactor(sqlite-storage)!: replace bincode with prost for persisted blobs (#911)",
          "timestamp": "2026-06-28T01:06:49-03:00",
          "tree_id": "c1cb39e1903bb1524e934653e6d7cf05b26d4122",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/301f2e030eac2061e2794e5ac790a5153ced2d5d"
        },
        "date": 1782619938932,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10594168,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8557238,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10594108,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540311,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159579,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 486469,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44716,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019490,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3444771,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658431,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 465,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9c6ee8a6770abfc09fe186fea893aa75c734c6e1",
          "message": "fix(build): isolate voip example so the demo build drops cpal/alsa-sys (#922)",
          "timestamp": "2026-06-28T01:59:07-03:00",
          "tree_id": "23837285c3d9d4915033e30498da61a2732320c0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9c6ee8a6770abfc09fe186fea893aa75c734c6e1"
        },
        "date": 1782622983702,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10594168,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8557238,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10594108,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540311,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159579,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 486469,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44716,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019490,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3444771,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658431,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9e8c70e2fcadc8f52246c8336243504fea89edf6",
          "message": "fix(status): omit addressing_mode on status@broadcast send (#925)",
          "timestamp": "2026-06-29T03:18:39-03:00",
          "tree_id": "1f1ce7ee5d3f963ebbd4ff91248a7512ed8e7a4b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9e8c70e2fcadc8f52246c8336243504fea89edf6"
        },
        "date": 1782714323637,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10594296,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8557366,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10594108,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540480,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159579,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 486469,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44716,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019490,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3444771,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658435,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "44aafc5a38a73d21b1b6573190c2eba4f9809e31",
          "message": "perf(sqlite-storage): cut per-session memory & threads, with configurable tuning (#926)",
          "timestamp": "2026-06-29T23:20:48-03:00",
          "tree_id": "ad3c0c014070c8226296c6c31b70d67170151dce",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/44aafc5a38a73d21b1b6573190c2eba4f9809e31"
        },
        "date": 1782786302945,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10596792,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8559158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10598324,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540480,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159579,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487137,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44716,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022827,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442543,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658435,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "645e2ba5b3fc470632da1f727b93a13e9de3b828",
          "message": "ci(docker): portable multi-arch image, unprivileged runtime, GHCR publish (#927)",
          "timestamp": "2026-06-30T00:52:47-03:00",
          "tree_id": "a6c49624b3088107e3e5caaedc7aad49502d7c95",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/645e2ba5b3fc470632da1f727b93a13e9de3b828"
        },
        "date": 1782791846672,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10596792,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8559158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10598324,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540480,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159579,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487137,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44716,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022827,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442543,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658435,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4ac1c81d21cf7353eca94af65349ad656a1a2800",
          "message": "chore(deps): bump aes-gcm from 0.11.0-rc.4 to 0.11.0 (#929)",
          "timestamp": "2026-06-30T21:23:16-03:00",
          "tree_id": "f678cb86495b01f8f7f980c3ecb02070f406b5d8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4ac1c81d21cf7353eca94af65349ad656a1a2800"
        },
        "date": 1782865574654,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10596792,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8559158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10598324,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540480,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159579,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487137,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44716,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022827,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442543,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658435,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "idunvoneinzbern@gmail.com",
            "name": "arsa0x",
            "username": "arsa0x"
          },
          "committer": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "570e82b451326b92776c06b399153ab48c82dd8a",
          "message": "feat(media): support ContextInfo in media options",
          "timestamp": "2026-07-01T09:14:05-03:00",
          "tree_id": "a703037c4c3f5d4d30e6f288dcb027eae470dd6e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/570e82b451326b92776c06b399153ab48c82dd8a"
        },
        "date": 1782908307808,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10596792,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8559158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10598324,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540480,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 159579,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487039,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44814,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022827,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442543,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17879,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658443,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "95bec41ab0f9d67d09c67506121e83c1cd8f4093",
          "message": "perf(binary): validate wire strings with smoothutf8 (#932)",
          "timestamp": "2026-07-01T12:13:48-03:00",
          "tree_id": "52e9df3bf0461bc768f199bfeca4272a1d5aaa05",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/95bec41ab0f9d67d09c67506121e83c1cd8f4093"
        },
        "date": 1782919214290,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10601784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8564214,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10602460,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543415,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161317,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487137,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022817,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442996,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644769,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17882,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658443,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "228d5908c7663f29e963d3a7651d88560ea3d8e7",
          "message": "perf(binary): inflate into uninitialized buffer, drop the zero-init memset (#933)",
          "timestamp": "2026-07-01T12:46:29-03:00",
          "tree_id": "45a661d181b125e471b9c4f68bf13676004a039e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/228d5908c7663f29e963d3a7651d88560ea3d8e7"
        },
        "date": 1782921114278,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10600312,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8563382,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10598148,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1559991,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543351,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161628,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487002,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9157,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022125,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442450,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644764,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17880,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658443,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20447,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33990152+blaueeiner@users.noreply.github.com",
            "name": "Maximilian Winter",
            "username": "blaueeiner"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f2b68348e01c01e00b2f792cf9494e28dd2c7b48",
          "message": "fix(receive): skip PDO placeholder-resend for view-once, ack instead (#934)",
          "timestamp": "2026-07-01T13:36:26-03:00",
          "tree_id": "409a7ca5bfd2184e6ecdaf62f8ec92f06d3d5384",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f2b68348e01c01e00b2f792cf9494e28dd2c7b48"
        },
        "date": 1782924018898,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10600568,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8563638,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10602244,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1560232,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543351,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161628,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487137,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022125,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442450,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644764,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17880,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658525,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20448,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "421c66a3b81067f3762b6dc952c08022622dfb0f",
          "message": "fix(receive): skip PDO placeholder-resend for bot and hosted unavailable too (#935)",
          "timestamp": "2026-07-01T15:05:18-03:00",
          "tree_id": "68e073d19e180d996909526cd7267e04ac069bd9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/421c66a3b81067f3762b6dc952c08022622dfb0f"
        },
        "date": 1782929422060,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10605496,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8564406,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10606300,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1561000,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543351,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487137,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022125,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442450,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 644791,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17880,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 658630,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20452,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "20c77187299c13d3603b3756dc00081dc8972201",
          "message": "perf(send): trim group-send warm-path CPU and cold fan-out allocations (#936)",
          "timestamp": "2026-07-01T19:00:57-03:00",
          "tree_id": "34aa66ea68f2339499cacfc9a898ec598524080d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/20c77187299c13d3603b3756dc00081dc8972201"
        },
        "date": 1782943561566,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10623000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8581494,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10622740,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1573742,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147697,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487137,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1023381,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3442450,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 645750,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17926,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 662695,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20615,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3fb4729e1ee14b376c0ce42ef5046b0b286afcc7",
          "message": "feat(passkey): SHORTCAKE_PASSKEY companion linking (#928)",
          "timestamp": "2026-07-01T19:18:40-03:00",
          "tree_id": "9ee4e6afeee87901534458e2155fb8d73976b8e8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3fb4729e1ee14b376c0ce42ef5046b0b286afcc7"
        },
        "date": 1782944643423,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10674968,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8625014,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10672900,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1599467,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 556983,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487039,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44710,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1026643,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3445967,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 649184,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17998,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 673826,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 20879,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9593ce2d84476af975238bade61e406827bb1210",
          "message": "perf(send): single-flight cold group sender-key distribution (#937)",
          "timestamp": "2026-07-01T20:51:46-03:00",
          "tree_id": "481df4016af94e43a2839deeec4735b6941b09f8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9593ce2d84476af975238bade61e406827bb1210"
        },
        "date": 1782950202678,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10696312,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8645110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10693404,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1621178,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 556624,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487039,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44710,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1025278,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3445967,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 649184,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17998,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 678181,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 21000,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f548d1fcfbf7f16010d936f7f1bff69fc7bd4335",
          "message": "perf(send): keep send_message's future pointer-sized (#938)",
          "timestamp": "2026-07-01T20:52:00-03:00",
          "tree_id": "d7615359ad45cf238e1ed6187c034abcc8d92448",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f548d1fcfbf7f16010d936f7f1bff69fc7bd4335"
        },
        "date": 1782950218992,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10696376,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8645174,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10693404,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1621232,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 556624,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 487137,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1025278,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3445967,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 649184,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17998,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 678181,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 21000,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "96686eac65460b3bc8e10e3c37f27c0a602c3f54",
          "message": "fix(send): gate DM LID wire addressing on the account's 1:1 migration state (#943)\n\nCo-authored-by: juanlotito <91030149+juanlotito@users.noreply.github.com>",
          "timestamp": "2026-07-02T13:38:15-03:00",
          "tree_id": "547a799b512aea138bf76c2e0b219694f9fe0a1a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/96686eac65460b3bc8e10e3c37f27c0a602c3f54"
        },
        "date": 1783010643974,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10718680,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8665334,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10714396,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1633609,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 558550,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 488335,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1029812,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3446011,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 649591,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18006,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 681373,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 21088,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e6168637fb0c3b5355f7dd4fcc9224b8c187bda2",
          "message": "perf(history-sync): size the secret-record accumulator by sampled density (#945)",
          "timestamp": "2026-07-02T16:01:00-03:00",
          "tree_id": "fa6dc26d37e4618201ab9fdebfa8cfd287b524f2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e6168637fb0c3b5355f7dd4fcc9224b8c187bda2"
        },
        "date": 1783019176929,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10719128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8665782,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10714396,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1633609,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 558964,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 169841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 488200,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9157,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1029812,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3446011,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 649751,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18010,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 681373,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 21088,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dcd1d97b6387e80a2b761feab12245cd30ea7ebd",
          "message": "perf(libsignal): evict skipped message keys without shifting the buffer (#946)",
          "timestamp": "2026-07-02T16:08:21-03:00",
          "tree_id": "410980f38fca553167f03b88c30937bd6e6e6efa",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dcd1d97b6387e80a2b761feab12245cd30ea7ebd"
        },
        "date": 1783019688669,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10722968,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8669110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10718948,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1633609,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 558964,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 488237,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44710,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1032224,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3446011,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 649751,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18010,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 681373,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 21088,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "84be3946dc070552d64a0674740470b607a04d8b",
          "message": "perf(history-sync): keep heap-error types off the scanner happy path (#947)",
          "timestamp": "2026-07-02T16:18:13-03:00",
          "tree_id": "fa73e1e2605d808c574cdf40f004ebf8542c6f55",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/84be3946dc070552d64a0674740470b607a04d8b"
        },
        "date": 1783020233852,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10715384,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8661430,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10714700,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1633609,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 551002,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 488335,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1032490,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3446011,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 649507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18010,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 681373,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 21088,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b5daf757fdb08c7cad068cdd04ea0461cc2f5f13",
          "message": "perf(send): keep public send futures pointer-scale (#948)",
          "timestamp": "2026-07-02T16:36:12-03:00",
          "tree_id": "a06d8c0c2faf6cca3a1270d854510dbd88f40c4f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b5daf757fdb08c7cad068cdd04ea0461cc2f5f13"
        },
        "date": 1783021275449,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10716728,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8662582,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10714732,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1622742,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 551002,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161576,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 170759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 147754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 28375,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 892915,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 488335,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44612,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9022,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1044475,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3446011,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 649507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18010,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 681373,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 21088,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a6fb1174855adce72d581f8102e713696989e11a",
          "message": "feat!: migrate from prost to buffa for protobuf codegen (#557)",
          "timestamp": "2026-07-02T17:20:07-03:00",
          "tree_id": "878b25100d6cc36f83995ebf833d88e1d4dc97d4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a6fb1174855adce72d581f8102e713696989e11a"
        },
        "date": 1783024418386,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11135224,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9072502,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131908,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1610847,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 545621,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161622,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 180692,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1671103,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 491245,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44606,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9168,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031869,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084476,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498015,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17045,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714243,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23185,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0ce4907b230d01a80f832c2f6585aa1684ad6e90",
          "message": "perf(binary): keep BinaryError construction off the decoder happy path (#949)",
          "timestamp": "2026-07-02T18:44:58-03:00",
          "tree_id": "ffbbbb0ae8bb5665c8a1eacb052e07aeeb52a126",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0ce4907b230d01a80f832c2f6585aa1684ad6e90"
        },
        "date": 1783029231279,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11134648,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9072054,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131932,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1610847,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 545621,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161361,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 180692,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1671103,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 491245,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031661,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084476,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498015,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17045,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714243,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23185,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 467,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d77226780f83d3ba90992a6dde969df08b7f5742",
          "message": "bench: deterministic rng and hashers; pin and shard the CodSpeed CI (#950)",
          "timestamp": "2026-07-02T19:37:23-03:00",
          "tree_id": "afbf2ec6424348537772ac2debece5ac5efd76a2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d77226780f83d3ba90992a6dde969df08b7f5742"
        },
        "date": 1783032366512,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11134648,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9072054,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131932,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1610847,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 545621,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161361,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 180692,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1671103,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 491245,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031661,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084476,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498015,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17045,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714243,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23185,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d4e3d55be8d1f8491fafa287d851f326ebd7770e",
          "message": "perf: keep droppy error/default construction off per-message happy paths (#952)",
          "timestamp": "2026-07-02T19:42:57-03:00",
          "tree_id": "3db826b62088930ae368ad2f3976fa9c1be8bfa4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d4e3d55be8d1f8491fafa287d851f326ebd7770e"
        },
        "date": 1783032708474,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11133944,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9071350,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131892,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1610532,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 545321,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 180553,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1671103,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 491245,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44606,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9168,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031627,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084476,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498667,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17076,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714243,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23185,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "09cf50a0dc5cc6930af18653e089d008fdb6f11e",
          "message": "tracing: tag wa.iq / wa.send.message / wa.conn.run spans with account identity (#951)",
          "timestamp": "2026-07-02T19:46:28-03:00",
          "tree_id": "8b64d36b54079fc6330b309d0d90ce5545d23ecd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/09cf50a0dc5cc6930af18653e089d008fdb6f11e"
        },
        "date": 1783032907573,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11133944,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9071350,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131892,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1610532,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 545321,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 180553,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1671103,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 491245,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44606,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9168,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031627,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084476,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498667,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17076,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714243,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23185,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4bd09f03fc90a8d0294ac3c63cc3570988883577",
          "message": "perf(libsignal): store the sender-key backlog as Copy (iteration, seed) pairs (#953)",
          "timestamp": "2026-07-02T20:25:02-03:00",
          "tree_id": "2abefe83ca3af292050ad43b0daf1b21da570b04",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4bd09f03fc90a8d0294ac3c63cc3570988883577"
        },
        "date": 1783035348626,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11134136,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9071542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131916,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1610532,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 545321,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 491245,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44606,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9168,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031379,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084476,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498667,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17076,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714243,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23185,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "84459faa0d91da7c2938efa42c28f939c9e738ef",
          "message": "api: re-export the crates whose types appear in the public API (#954)",
          "timestamp": "2026-07-02T20:55:10-03:00",
          "tree_id": "c11caba8f32aca22b4fa6c924165746c85964e92",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/84459faa0d91da7c2938efa42c28f939c9e738ef"
        },
        "date": 1783037278685,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11134136,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9071542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131916,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1610532,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 545321,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 491245,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031379,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084476,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498667,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17076,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714243,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23185,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "053b3abfdf782774dc7954a27227e862da764ddb",
          "message": "perf: inline index-MAC keys, reusable conversation decode, single-encode send (#955)",
          "timestamp": "2026-07-02T22:07:07-03:00",
          "tree_id": "3c1294cd81f18bb103b6aa808a98054478b53490",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/053b3abfdf782774dc7954a27227e862da764ddb"
        },
        "date": 1783041627020,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11136696,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9073846,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131900,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1611296,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546405,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 493288,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44606,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9168,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031466,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3082856,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498531,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17059,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714603,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23204,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "915ed8561a5d2a12036ddc9c099a34a558c2c56a",
          "message": "perf(voip): zerocopy the RTP/STUN fixed-layout parse; borrow STUN attr values (#957)",
          "timestamp": "2026-07-02T22:35:14-03:00",
          "tree_id": "1a6b1fd952bdbf99e0eb9d8a92933511f8852bda",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/915ed8561a5d2a12036ddc9c099a34a558c2c56a"
        },
        "date": 1783042961690,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11136696,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9073846,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11131900,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1611296,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546405,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 493288,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44606,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9168,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031466,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3082856,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498531,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17059,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714603,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23204,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3c97916c5c6bfc0b03d95242941b91f481223cb7",
          "message": "fix!: typed read-loop exit — routine server recycles are not errors; Disconnected carries the reason (#956)",
          "timestamp": "2026-07-02T22:37:59-03:00",
          "tree_id": "9aeb3b79c89944c9f078239e38b5b8e22f0c40d0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3c97916c5c6bfc0b03d95242941b91f481223cb7"
        },
        "date": 1783043266879,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11138136,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9075254,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11135964,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1612501,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546405,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 493288,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031638,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3082856,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498549,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17061,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 714987,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23210,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6b6016e75e375f5a48cbf5a9f147cf250545d217",
          "message": "perf(connect): move own-device sync off the pre-active path; drop keepalive loop span (#958)",
          "timestamp": "2026-07-02T23:04:43-03:00",
          "tree_id": "550530ff846f318b90a070ffe5828ee6bdb83c49",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6b6016e75e375f5a48cbf5a9f147cf250545d217"
        },
        "date": 1783044879695,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11137784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9074998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11135964,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1612262,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546405,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 493288,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031623,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3082856,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498549,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17061,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 715114,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23215,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "94e20d6ec92a467a609d6aaa910f385f5007a9b1",
          "message": "perf(tracing): drop client-lifetime wa.conn.run span (#959)",
          "timestamp": "2026-07-02T23:09:37-03:00",
          "tree_id": "336cd7f80f64950c9dfe0c9689b73792f2c28c63",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/94e20d6ec92a467a609d6aaa910f385f5007a9b1"
        },
        "date": 1783045102363,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11137784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9074998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11135964,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1612262,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546405,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 493288,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031623,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3082856,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498549,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17061,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 715114,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23215,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ec3a12ad65c5181221dfa368ee64c914d6f27e53",
          "message": "fix: shut down on SIGTERM, not just SIGINT (docker stop timeout) (#960)",
          "timestamp": "2026-07-03T00:25:29-03:00",
          "tree_id": "68d01bb90851c8566821c2523d17a054e25400f8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ec3a12ad65c5181221dfa368ee64c914d6f27e53"
        },
        "date": 1783049605126,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11138776,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9076022,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11136012,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1612929,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546405,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 493288,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031560,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3083274,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 498549,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17061,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 715724,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23248,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "52eb7b34bbca628a81c07203d969a27848ce557d",
          "message": "feat: wire I/O counters, memory report in bytes, runtime-agnostic task instrumentation (#962)",
          "timestamp": "2026-07-03T04:02:17-03:00",
          "tree_id": "c091ef2ef6d11a6399dc795631d738075695db56",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/52eb7b34bbca628a81c07203d969a27848ce557d"
        },
        "date": 1783062665754,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11140824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9077366,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11136252,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1613230,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 547002,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 493288,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031930,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3083290,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 500712,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17154,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 719804,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23295,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c57b87bbbf7015c470d95ae0023e75dcc8711fdd",
          "message": "Meter Bot::run's main future through the task instrument (#963)",
          "timestamp": "2026-07-03T04:45:12-03:00",
          "tree_id": "f8f4114f7cf14b4f9eaea7c940c5970025433c4b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c57b87bbbf7015c470d95ae0023e75dcc8711fdd"
        },
        "date": 1783065177851,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11140952,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9077494,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11140348,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1613285,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 547002,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673045,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 493288,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031962,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3083373,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 500712,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17154,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 720149,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23315,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "eb91ac1996d37f58acc57481281697587069b403",
          "message": "feat(recv)!: batch the inbound commit pipeline during the offline drain (#961)",
          "timestamp": "2026-07-03T13:42:59-03:00",
          "tree_id": "e18e003c09b33c30a0674131877d68dc5a15b1d1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/eb91ac1996d37f58acc57481281697587069b403"
        },
        "date": 1783097584530,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11196504,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9123638,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11194948,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1643833,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 548754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673224,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 500609,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038103,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3083403,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 500835,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17162,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 728604,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23639,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2192fd40e879640dd79d63134252e436819a5291",
          "message": "fix(stats): make Client::memory_report() Send (#964)",
          "timestamp": "2026-07-03T14:12:15-03:00",
          "tree_id": "efd978866120cfa9345db2a1f66f54375f48361d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2192fd40e879640dd79d63134252e436819a5291"
        },
        "date": 1783099198332,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11196504,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9123638,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11194948,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1643833,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 548754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179004,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673224,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 500609,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038103,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3083403,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 500835,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17162,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 728604,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23639,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d0c670bd9e094cec72df1e3537417abd4a28a36b",
          "message": "feat: WA Web parity fixes (offline drain, crypto caps, reconnect, groups) (#965)",
          "timestamp": "2026-07-03T16:41:36-03:00",
          "tree_id": "522b5860bc92e9fa5d62ab8faadf591c95e6ec78",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d0c670bd9e094cec72df1e3537417abd4a28a36b"
        },
        "date": 1783108398597,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11200280,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9127030,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11199180,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1648111,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546848,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179191,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673224,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 500609,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44508,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9266,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038912,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3083403,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 500835,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17162,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 729492,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23657,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "db3566f959dd59a8c7461d397ce067dc67281ae4",
          "message": "feat(stats): attribute per-session resources beyond the Client (#967)",
          "timestamp": "2026-07-03T17:41:13-03:00",
          "tree_id": "38e856b90520e206c19c2cd6da485118fd2c671c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/db3566f959dd59a8c7461d397ce067dc67281ae4"
        },
        "date": 1783111816298,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11208952,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9133302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11208084,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1648430,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 546848,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179191,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673224,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 504940,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44549,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9295,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1039346,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084502,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 501469,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17199,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 731444,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23694,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fde503fe77d69e6219e2fb6aba1790ccdbb7d2f5",
          "message": "fix(tctoken): persist issuance timestamp on IQ success and gate cstoken independently (#966)",
          "timestamp": "2026-07-03T17:53:39-03:00",
          "tree_id": "40b629baff49941c180400d46f71f0013d681dd3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fde503fe77d69e6219e2fb6aba1790ccdbb7d2f5"
        },
        "date": 1783112693557,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11222520,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9141814,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11221380,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1645983,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544646,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179191,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673224,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 518715,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44549,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9295,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038758,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084480,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 501968,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17211,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 730809,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23703,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0b1d349c4b2e1554d149ab965591400084df04c5",
          "message": "feat(tctoken): attach tctoken in usync status/about and spam-report IQs (#969)",
          "timestamp": "2026-07-03T18:58:59-03:00",
          "tree_id": "22715a3eb30a41929cc616701d1feaaef907b119",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0b1d349c4b2e1554d149ab965591400084df04c5"
        },
        "date": 1783116528215,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11222520,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9141814,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11221380,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1645983,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544646,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 179191,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159844,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1673224,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 518715,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44549,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9295,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038758,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084480,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 502227,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17218,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 730809,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23703,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d1fe9fe93c129199b4524375e05a623655e120db",
          "message": "feat: rotate signed pre-key on a cadence (WA Web RotateKeyJob) (#968)",
          "timestamp": "2026-07-03T19:24:47-03:00",
          "tree_id": "d66c628ab79a576237523003af93796bc8958392",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d1fe9fe93c129199b4524375e05a623655e120db"
        },
        "date": 1783118143858,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9169334,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11254468,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1659291,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543633,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1676522,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 519941,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44549,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9295,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1045410,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084702,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 503148,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17243,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 737618,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23868,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dbdd2403896f67a53920115f4091bba845a3ac01",
          "message": "feat(tctoken): attach and issue tctoken on outgoing VoIP call offers (#970)",
          "timestamp": "2026-07-03T19:55:53-03:00",
          "tree_id": "e95f4605b3bbbfbcf84ee0153574f03031f4ac6a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dbdd2403896f67a53920115f4091bba845a3ac01"
        },
        "date": 1783119877662,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9169206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11254436,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1659172,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543633,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1676522,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 519941,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44549,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9295,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1045416,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084702,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 503148,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17243,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 737616,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23865,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "16298aaf36a8cf6714647f3b179daac894989900",
          "message": "feat: gate DM read/played receipts on readreceipts privacy (#971)",
          "timestamp": "2026-07-03T20:29:01-03:00",
          "tree_id": "ab0d437501e2cde028224a95809a1ca58b909894",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/16298aaf36a8cf6714647f3b179daac894989900"
        },
        "date": 1783121896020,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11259864,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9171510,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11258628,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1659453,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543673,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 159907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1676522,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 521340,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44549,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9295,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1045984,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084707,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 503173,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17243,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 737937,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23865,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "51030e623d1a1da4f3ff2c726eed9e5accf63922",
          "message": "fix(waproto): stop the build script recompiling on every run (#973)",
          "timestamp": "2026-07-03T21:43:25-03:00",
          "tree_id": "6224948d9d5798f08d2a757bd71a7f8f3540335d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/51030e623d1a1da4f3ff2c726eed9e5accf63922"
        },
        "date": 1783126149666,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11259864,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9171510,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11258628,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1659453,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543673,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160161,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1676522,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 521475,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 9160,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1045984,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3084349,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 503173,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17243,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 737937,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23865,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4def65976a8d7ab7f429d6e3300201026f795b15",
          "message": "perf: parallelize remaining serial/blocking hot paths (startup, media, send/recv) (#975)",
          "timestamp": "2026-07-04T12:04:43-03:00",
          "tree_id": "2333c3ffa26b8b1cccbee9e2b945b168af8f22fc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4def65976a8d7ab7f429d6e3300201026f795b15"
        },
        "date": 1783178000049,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11289816,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9198966,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11288116,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1682430,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 541324,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 521475,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1048623,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3085900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 503173,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17243,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 745608,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24265,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d394551ea38403f1b1c388988e979f765017b76e",
          "message": "fix(pair-code): correct link_code_pairing_nonce byte and close WA Web stage-2 gaps (#976)",
          "timestamp": "2026-07-04T13:34:13-03:00",
          "tree_id": "7a5dc31e9f5e65f33d7c5bfe7c49f9bc1605b12e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d394551ea38403f1b1c388988e979f765017b76e"
        },
        "date": 1783183369412,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11294104,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9203126,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11292388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1686091,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 541324,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 521377,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44751,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1048658,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3085900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 503158,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17243,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 746771,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24288,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "61df3803c8062e72b9aa687433cae4855a7d5bfb",
          "message": "fix(iq): cancellation-safe IQ response waiters (unblocks try_join!) + named fan-out consts (#978)",
          "timestamp": "2026-07-04T13:36:33-03:00",
          "tree_id": "e102e3c26a6f9a47094e5462fec3e34de7fea8bf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/61df3803c8062e72b9aa687433cae4855a7d5bfb"
        },
        "date": 1783183426113,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11283064,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9191926,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11280100,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1677326,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 541521,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 521475,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1044995,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3086900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 503158,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17243,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 745692,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24271,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a3c81c3c170fac27d76ae574a12bd647dbb0a2c0",
          "message": "fix(tc-token): atomic newer-wins store; drop tc_token_lock and close the cross-source race (#980)",
          "timestamp": "2026-07-04T14:42:52-03:00",
          "tree_id": "785a3683448e11195a4b081c5cf7e4525fb37fcd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a3c81c3c170fac27d76ae574a12bd647dbb0a2c0"
        },
        "date": 1783187428548,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11285816,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9194294,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11284428,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1676373,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 541521,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1044507,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3086900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 503177,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17244,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 745528,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24271,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f4f88e107671f4919e7091b940d0c125a8abe1bf",
          "message": "fix(pair-code): canonicalize companion_platform_display OS to a server-safe set (#979)",
          "timestamp": "2026-07-04T15:20:34-03:00",
          "tree_id": "bab83d60aff4d531011a391b7275a6d017f36049",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f4f88e107671f4919e7091b940d0c125a8abe1bf"
        },
        "date": 1783189673707,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11289976,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9197942,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11288588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1675205,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 542502,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1045905,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3089237,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504285,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17275,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 745748,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24280,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "882d1f8730ad727903b87289469a75e45bce45a6",
          "message": "feat(events): opt-in ordered + bounded inbound event delivery (#981)",
          "timestamp": "2026-07-04T17:30:38-03:00",
          "tree_id": "bd510711a2820cf9e7e4d706f6ec670f82ea7211",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/882d1f8730ad727903b87289469a75e45bce45a6"
        },
        "date": 1783197459606,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11296984,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9204470,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11296860,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1679215,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1046290,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090738,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504289,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17275,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 749290,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24387,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bda9bb8723642ae09fb09dad71a6471ad78c3e89",
          "message": "perf(recv): hold the per-sender session lock only around Signal decrypt (#983)",
          "timestamp": "2026-07-05T01:17:16-03:00",
          "tree_id": "ac9b557fc9c54099186dc7dcc4b09d3b121d0bec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bda9bb8723642ae09fb09dad71a6471ad78c3e89"
        },
        "date": 1783225578151,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11295224,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9202870,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11292836,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1677742,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1046148,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090738,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504289,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17275,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 748702,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24405,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2228c2cf3198e720486f6b46d8ee270c9c6ea719",
          "message": "feat(retry): opt-in RetryAdmission hook (compliant supersede of #982) (#985)",
          "timestamp": "2026-07-05T17:57:43-03:00",
          "tree_id": "205b558b42deb4475611847755f1bd3e8ae0f6ba",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2228c2cf3198e720486f6b46d8ee270c9c6ea719"
        },
        "date": 1783285466589,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11303128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9210294,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11301036,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1683484,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1046650,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091935,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504289,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17275,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 750419,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24480,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d4955558409287d66e40260b7805c6c4649a721e",
          "message": "fix(appstate): bound pairing key-share wait by the 180s critical deadline (#974)",
          "timestamp": "2026-07-05T17:58:18-03:00",
          "tree_id": "48a088ed1003532e58f064fcafa1f4137fa09ff7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d4955558409287d66e40260b7805c6c4649a721e"
        },
        "date": 1783285609761,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11304376,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9211574,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11301068,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1684816,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1046788,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091726,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504289,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17275,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 750735,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24484,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8414b430481d1a80ed0641040e08827375a4156d",
          "message": "fix(recv): retry (not NACK) recoverable group skmsg decrypt failures (#986)",
          "timestamp": "2026-07-06T18:37:19-03:00",
          "tree_id": "0a64d83cb15299f3cce69c5e1c3c53315ce470f6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8414b430481d1a80ed0641040e08827375a4156d"
        },
        "date": 1783374511566,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11304824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9212022,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11305164,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1685280,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1046762,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091726,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504289,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17275,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 750924,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24486,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5cf0e77701fdb545a591e1ac99cc0dde0d32f8a8",
          "message": "fix(recv): retry (not NACK) InvalidSignedPreKeyId on the 1:1 decrypt path (#987)",
          "timestamp": "2026-07-06T18:55:28-03:00",
          "tree_id": "e55d3ecbd9a2d37864c44105361f180f4849b508",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5cf0e77701fdb545a591e1ac99cc0dde0d32f8a8"
        },
        "date": 1783375390571,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11304472,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9211638,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11301068,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1684884,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1046778,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091726,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504289,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17275,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 751079,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24487,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8382d3d5114f8a8979fa356cd419e6d182fee9d9",
          "message": "fix(appstate): validate aggregate snapshot/patch MACs for genesis patches (#988)",
          "timestamp": "2026-07-06T19:20:45-03:00",
          "tree_id": "dcca05e00e2c70357566af8d9525209d08ef5160",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8382d3d5114f8a8979fa356cd419e6d182fee9d9"
        },
        "date": 1783376995489,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11304536,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9211702,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11301068,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1684884,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543189,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1046778,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091726,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504289,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17275,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 751079,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24487,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "52fd3630f5794c9c2cad2a4c4fc7d79f3beb338b",
          "message": "fix(send): lock per-device sessions across the group SKDM fan-out (#990)",
          "timestamp": "2026-07-06T20:59:39-03:00",
          "tree_id": "85596afcf84ca0815513a62c9fc986488f4f7f5c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/52fd3630f5794c9c2cad2a4c4fc7d79f3beb338b"
        },
        "date": 1783382810494,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11308408,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9214966,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11305300,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1686548,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543966,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1047511,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091726,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504304,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 751733,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24518,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9774a29e72025c46d186c885f7d9ae9d2fcb96fb",
          "message": "fix(recv): serialize the group inbound sender-key chain with a lock (#992)",
          "timestamp": "2026-07-06T21:00:17-03:00",
          "tree_id": "bb9f248cbeeeaca69e43d1ed2d36fb3a78afcb07",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9774a29e72025c46d186c885f7d9ae9d2fcb96fb"
        },
        "date": 1783382860339,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11309368,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9215926,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11309420,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1687092,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543966,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1047921,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091726,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504304,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 751836,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24518,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4eecf0ebca0c3d5fe4a651bd279a01b64c370126",
          "message": "fix(cache): don't capacity-evict a session lock a task still holds (#991)",
          "timestamp": "2026-07-06T21:23:29-03:00",
          "tree_id": "a3e4fc444ffb6ae2775197a9371b73fb8ff7d509",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4eecf0ebca0c3d5fe4a651bd279a01b64c370126"
        },
        "date": 1783384264434,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11328312,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9232886,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11325756,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1703047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543966,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161265,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049327,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504304,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757262,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24574,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "27417af3ae105e44671f489f47d040e9f3643500",
          "message": "fix(binary): bound node-decode recursion depth to reject hostile frames (#994)",
          "timestamp": "2026-07-06T23:07:04-03:00",
          "tree_id": "c4961de0bdc26f488f73a619a1a1709de2a485ce",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/27417af3ae105e44671f489f47d040e9f3643500"
        },
        "date": 1783390573957,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11328632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11325764,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1703047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543966,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049344,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504308,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757266,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24574,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "32c47d9f795a2a47a3814b91890600eb777acbd1",
          "message": "fix(passkey): don't skip the verification-code UX on a fresh link (#997)",
          "timestamp": "2026-07-06T23:14:01-03:00",
          "tree_id": "8bea6835374789ee3396f79033162e2e6528f6df",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/32c47d9f795a2a47a3814b91890600eb777acbd1"
        },
        "date": 1783390990705,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11328632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11325764,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1703043,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543966,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049354,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504308,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757302,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24575,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2f001b5a3d6374cc5cf7177792c2a81f87a54080",
          "message": "fix(voip): authenticate WARP MI tag before folding recv ROC state (#998)",
          "timestamp": "2026-07-06T23:18:28-03:00",
          "tree_id": "2e9140302a3d865ac539ac7a9a86c37025595937",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2f001b5a3d6374cc5cf7177792c2a81f87a54080"
        },
        "date": 1783391216725,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11328632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11325764,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1703043,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543966,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049354,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504308,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757302,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24575,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5f07cb9aacd3b99da3cfb08eaefb7744bc98c04b",
          "message": "fix(keepalive): anchor dead-socket watchdog to first send, not last (#995)",
          "timestamp": "2026-07-06T23:23:34-03:00",
          "tree_id": "08227ca1ac35736de60992e7bee00ffb39d4c6b2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5f07cb9aacd3b99da3cfb08eaefb7744bc98c04b"
        },
        "date": 1783391502088,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11328760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233334,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11325764,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1703115,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543966,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049354,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504310,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757319,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24575,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5ba420102cc342b358b6de7bb9fba3b71957b187",
          "message": "fix(send): isolate a group device's session-setup failure from the cohort (#996)",
          "timestamp": "2026-07-06T23:37:38-03:00",
          "tree_id": "7cb7e0c52064b140f1118b7b36db269bfb1a53d5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5ba420102cc342b358b6de7bb9fba3b71957b187"
        },
        "date": 1783392362361,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11333112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233590,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11329860,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1703115,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544210,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049354,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504310,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757359,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24575,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "251c23855ed53be88497e5667510b370164261a2",
          "message": "fix(send): never memoize own devices in the sender-key map (WA Web parity) (#999)",
          "timestamp": "2026-07-07T00:01:33-03:00",
          "tree_id": "d7b535df9a2d0f349d3c61f68b76357b5412307d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/251c23855ed53be88497e5667510b370164261a2"
        },
        "date": 1783393869337,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11333176,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233654,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11329860,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1703227,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544210,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049354,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504310,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17277,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757359,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24575,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "110734564+JeanCapixaba@users.noreply.github.com",
            "name": "Jeanderson Bianchi Vieira",
            "username": "JeanCapixaba"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "376ac1b319732e5bda3590ccf524e3dbc1d7c266",
          "message": "feat(events): observe-only ServerAck event for server <ack> stanzas (#989)",
          "timestamp": "2026-07-07T11:25:59-03:00",
          "tree_id": "a592cd8231cbd3d7e3030183df86c8140a7cea46",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/376ac1b319732e5bda3590ccf524e3dbc1d7c266"
        },
        "date": 1783434824147,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11336696,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9237110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11333964,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1706560,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544226,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049448,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504323,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757829,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24586,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "37effa5d2fe35394c0099bcc619a621dcda9f57d",
          "message": "refactor(events): model ServerAck.class as Option, document payload stability (#1000)",
          "timestamp": "2026-07-07T12:43:07-03:00",
          "tree_id": "e58e4357811c53b852aee329aec8324e2d2204b0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/37effa5d2fe35394c0099bcc619a621dcda9f57d"
        },
        "date": 1783439443019,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11336696,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9237110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11333964,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1706545,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544226,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049448,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504323,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 757825,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24586,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 466,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "45c5b642ce3ca64edd0ed8d2e9a5deccfa3bf304",
          "message": "refactor(events): seal ServerAck with non_exhaustive + bon builder (#1002)",
          "timestamp": "2026-07-07T15:07:36-03:00",
          "tree_id": "9e86cc8bd25943c4c0bbc97697c186885c81ab15",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/45c5b642ce3ca64edd0ed8d2e9a5deccfa3bf304"
        },
        "date": 1783448196162,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11336824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9237238,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11333964,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1706670,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544226,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049448,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504323,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 758006,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24596,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 468,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "cd317bc2711f041b74a02a368dc0bff4c664cc16",
          "message": "refactor(events): seal notification, sync event payloads (non_exhaustive + bon builder) (#1003)",
          "timestamp": "2026-07-07T15:42:41-03:00",
          "tree_id": "8f81931d219ddcd369049a9655cc1f0c19c3d2aa",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cd317bc2711f041b74a02a368dc0bff4c664cc16"
        },
        "date": 1783450304243,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11338424,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9238774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11338036,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1708179,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544226,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183024,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 160202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1677256,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049457,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504323,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 761162,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24680,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 468,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ba1ac3bf0fdf09ecc92531a5431cef77a1302ca2",
          "message": "chore(proto): bump WhatsApp protocol surface to 2.3000.1042742319 (#1001)",
          "timestamp": "2026-07-08T04:58:00-03:00",
          "tree_id": "9a308f095faf64d37d97906ef5c7422169633e11",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ba1ac3bf0fdf09ecc92531a5431cef77a1302ca2"
        },
        "date": 1783498199256,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11343736,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9244022,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11342148,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1708858,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 544243,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1050215,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091246,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504429,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17330,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 761299,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24695,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 468,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b863f9dfba32163fc95eb98693843c6656300dd8",
          "message": "refactor(events): complete the event-payload API freeze (#1004)",
          "timestamp": "2026-07-08T05:20:39-03:00",
          "tree_id": "82eeda14620ffdbea3c092a71055f5c883583a12",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b863f9dfba32163fc95eb98693843c6656300dd8"
        },
        "date": 1783499450434,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11345368,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9246134,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11341956,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1719986,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 535545,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525291,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44653,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1050005,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091148,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504433,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17332,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 764868,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24862,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 468,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6e0f241dc0265add92e1abff0203ec115b8fa4a7",
          "message": "chore(deps): update dependencies to latest versions (#1009)",
          "timestamp": "2026-07-08T05:40:34-03:00",
          "tree_id": "c81b3c6e31b9b2d092fa0ffc0c61aec48ee295c0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6e0f241dc0265add92e1abff0203ec115b8fa4a7"
        },
        "date": 1783500760025,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11342584,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9243894,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11341948,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1719967,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 535672,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161486,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1050015,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3088868,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 504433,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17332,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 764868,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24862,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 471,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33990152+blaueeiner@users.noreply.github.com",
            "name": "Maximilian Winter",
            "username": "blaueeiner"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "18c78f46c5ec8ce42ffdff7c107f50cdc1bef958",
          "message": "feat(history-sync): learn PN-LID mappings from phoneNumberToLidMappings (#1010)",
          "timestamp": "2026-07-08T17:55:49-03:00",
          "tree_id": "14539701c05aeb842e9ecfee2a66685d6ce16883",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/18c78f46c5ec8ce42ffdff7c107f50cdc1bef958"
        },
        "date": 1783544643795,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11347896,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9249014,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11346084,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1720435,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 539621,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161671,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1050500,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3088868,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505440,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17345,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 765731,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24902,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 471,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f95eb2de21661fd8e602025db6ff8fc03b62f7c1",
          "message": "feat(lid-pn): source-aware write policy matching WA Web createLidPnMappings (#1011)",
          "timestamp": "2026-07-08T20:02:25-03:00",
          "tree_id": "f9724aa8e421e142760b299fa5f2ac8488d7cdfd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f95eb2de21661fd8e602025db6ff8fc03b62f7c1"
        },
        "date": 1783552176022,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11355384,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9255926,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11354348,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1727226,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 538721,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161671,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1051492,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3088868,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505440,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17345,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 767242,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24947,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 471,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33990152+blaueeiner@users.noreply.github.com",
            "name": "Maximilian Winter",
            "username": "blaueeiner"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d8fd43f831718d2d89f598587bdf5fde3804c8d0",
          "message": "feat(lid-pn): make add_lid_pn_mapping pub for embedder-learned sources (#1013)",
          "timestamp": "2026-07-09T13:26:48-03:00",
          "tree_id": "a72ea59efad3127da0f4ee27da941a03ebba0715",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d8fd43f831718d2d89f598587bdf5fde3804c8d0"
        },
        "date": 1783614865288,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11355384,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9255926,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11354348,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1727226,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 538721,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161671,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1051492,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3088868,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505440,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17345,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 767242,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24947,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 471,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3a6ef95eefbb1fbdf1258cecdd97864f6555f60e",
          "message": "feat(chat-store): SQLite-backed chat/message history store (#1014)",
          "timestamp": "2026-07-10T12:12:45-03:00",
          "tree_id": "35b1e21a09584acd97d06f75de060f7fd6a40bbc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3a6ef95eefbb1fbdf1258cecdd97864f6555f60e"
        },
        "date": 1783696882671,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11355384,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9255926,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11354348,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1727226,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 538721,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161671,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1051492,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3088868,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505440,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17345,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 767242,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24947,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "07b271ad91e1496c729d7f09de2e9488c2577a99",
          "message": "perf(message): reduce hot-path allocations (#1015)",
          "timestamp": "2026-07-10T18:36:32-03:00",
          "tree_id": "011b15643d85d81eb377a50f855809a9ba006175",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/07b271ad91e1496c729d7f09de2e9488c2577a99"
        },
        "date": 1783719915393,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11357400,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9257590,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11354452,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1729594,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 539069,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161671,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049987,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3089296,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505783,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17358,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 767999,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24916,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d9141ac0ca1fc4614d11d058a654019d9d9a1dc3",
          "message": "perf(receipt): persistent worker for live delivery receipts (#1016)",
          "timestamp": "2026-07-10T19:38:31-03:00",
          "tree_id": "afb0f2b97dc97ba781b5415657c58a132d0496fc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d9141ac0ca1fc4614d11d058a654019d9d9a1dc3"
        },
        "date": 1783723674306,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11365304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9264758,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11362908,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1733973,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 539617,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161671,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1051405,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090076,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 506082,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17369,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 770790,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25007,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f9a08738dc50d095a9a6608b543e47bdce0b9334",
          "message": "perf(signal): sync fast paths for hot store adapter methods (#1017)",
          "timestamp": "2026-07-10T19:38:12-03:00",
          "tree_id": "4c5649166db88e5270624d4af5a79123c635b683",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f9a08738dc50d095a9a6608b543e47bdce0b9334"
        },
        "date": 1783723675633,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11360472,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9260342,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11358788,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1731740,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 539617,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 161671,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1050070,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3089296,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 506082,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17369,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 768518,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24936,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d2d604664818362713c5ca83a8f8cbd7992e56c2",
          "message": "perf(node): cut ack re-encode, exact stanza sizing, warm session pre-filter (#1018)",
          "timestamp": "2026-07-10T20:29:33-03:00",
          "tree_id": "1e98a87628dbf855936e727fd2de2421c3afa8cc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d2d604664818362713c5ca83a8f8cbd7992e56c2"
        },
        "date": 1783726648940,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11360024,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9259062,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11358524,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1735106,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540165,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 153003,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1052667,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090076,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 506082,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17369,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 770998,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25020,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "61c7c39edd7ad61cf42ef7905d2dc98ad8073621",
          "message": "perf(send): dispatch send_message_impl to per-branch boxed futures (#1019)",
          "timestamp": "2026-07-10T20:58:18-03:00",
          "tree_id": "41f7b6e7d080395b0095c069de00cbd5a235b42a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/61c7c39edd7ad61cf42ef7905d2dc98ad8073621"
        },
        "date": 1783728333249,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11374488,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9275254,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11374940,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1749599,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 539773,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152823,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1055107,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090094,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 506082,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17369,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 771512,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25060,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2591b246c589dd67ebfabd16ea8f2c3e38bc486d",
          "message": "perf(binary): inline the exact-marshal string hint cache (#1020)",
          "timestamp": "2026-07-10T21:19:30-03:00",
          "tree_id": "f1dc8f60c95a94e2678dc4c5ac82cfbcc55efde6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2591b246c589dd67ebfabd16ea8f2c3e38bc486d"
        },
        "date": 1783729721156,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11374872,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9275574,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11374972,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1749378,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540461,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1055107,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090729,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505737,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17371,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 771437,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25062,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d9e693f5446c9e0747b6b8766dd543beb8191e2b",
          "message": "perf(group): keep warm sends warm under the own-device SKDM steady state (#1021)",
          "timestamp": "2026-07-10T22:33:05-03:00",
          "tree_id": "c7c156bf8e56d47bb8ea4f4f4d643d4324fe8b59",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d9e693f5446c9e0747b6b8766dd543beb8191e2b"
        },
        "date": 1783733971125,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11380824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9281014,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11379020,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1754135,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540461,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1055692,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090763,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505737,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17371,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 771719,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25065,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9476f375236f099b60c7425ba51607854fa1c428",
          "message": "perf(signal): coalesce receive flushes and persist outbound state pre-wire (#1022)",
          "timestamp": "2026-07-14T08:53:09-03:00",
          "tree_id": "6832f8f097771ef787d0d0069aa569438de413a1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9476f375236f099b60c7425ba51607854fa1c428"
        },
        "date": 1784030345081,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11385464,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9285558,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11383164,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1757085,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540461,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1057317,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090763,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505737,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17371,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 772606,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25079,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b351bbf6e283eb948e0228f92f8f738b8d2be26f",
          "message": "perf(client): trim per-message allocations on the send/receive hot path (#1025)",
          "timestamp": "2026-07-14T17:14:58-03:00",
          "tree_id": "2fb423f0bb946506950a1be58f8455527d57e2ab",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b351bbf6e283eb948e0228f92f8f738b8d2be26f"
        },
        "date": 1784060574206,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11382072,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9282230,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11379108,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1753711,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 540456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183725,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678295,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056929,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091159,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505737,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17371,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 772648,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25101,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a7dc852ddfbfae00e36e40acb596afa50649402f",
          "message": "perf(signal): lease outbound counters in batches instead of flushing every send (#1026)",
          "timestamp": "2026-07-14T17:28:12-03:00",
          "tree_id": "d656dcf26f984d094295db789e7501e3b1c75449",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a7dc852ddfbfae00e36e40acb596afa50649402f"
        },
        "date": 1784061343202,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11391320,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9290998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11391428,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1757424,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 541290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 188086,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26676,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678093,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525308,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44845,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056954,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3091159,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 505894,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 773089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25116,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "861cfee18d935fafbe71bd6b350dbc4781dffa72",
          "message": "chore(deps): update workspace dependencies to latest compatible versions (#1034)",
          "timestamp": "2026-07-15T12:58:39-03:00",
          "tree_id": "573d8fe562da3381e216c80e49c1d4fb56fcd287",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/861cfee18d935fafbe71bd6b350dbc4781dffa72"
        },
        "date": 1784131854863,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11403192,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9298486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11403604,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1757424,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 549627,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 188086,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26656,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678093,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525393,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1057022,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090279,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 507674,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17378,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 773089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25116,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a5029cec0025797d230e6fd5dbfca4d8a58b0fa5",
          "message": "ci: cache target dir and drop incremental to stop full recompiles (#1033)",
          "timestamp": "2026-07-15T13:11:56-03:00",
          "tree_id": "533cc154d73400b7b18da149cbe3dd87f531ffeb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a5029cec0025797d230e6fd5dbfca4d8a58b0fa5"
        },
        "date": 1784132329430,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11403192,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9298486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11403604,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1757424,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 549627,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 188086,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26656,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678093,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525393,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1057022,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3090279,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 507674,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17378,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 773089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25116,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0e1db5762aff1988f74f95a6dd614bcba0c5d644",
          "message": "fix(signal): gate the group sender-key advance before the wire (#1027)",
          "timestamp": "2026-07-15T14:50:34-03:00",
          "tree_id": "2e82bffd35a2bb59236d1c2600de263d33dfe607",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0e1db5762aff1988f74f95a6dd614bcba0c5d644"
        },
        "date": 1784138256667,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11416792,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9309942,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11417412,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1754745,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543187,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 199933,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26656,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678093,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525393,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1058561,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3097255,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 508537,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17438,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774302,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25188,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a6e701e31aaf63bbf348f9b0e367fd45c12a089f",
          "message": "fix(signal): persist retry advances before wire (#1041)",
          "timestamp": "2026-07-15T16:02:13-03:00",
          "tree_id": "c8165a86cceca4de92aea6a91948aa47c75c8ea7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a6e701e31aaf63bbf348f9b0e367fd45c12a089f"
        },
        "date": 1784142547230,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11416920,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9309942,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11417396,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1754980,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 543187,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 199933,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26656,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678093,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525393,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1058323,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3097255,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 508537,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17438,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774379,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25193,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "042367ed65483a82af9bf21f3250a0450a0e21ca",
          "message": "fix(signal): retain durability gates through deletes (#1042)",
          "timestamp": "2026-07-15T16:38:03-03:00",
          "tree_id": "66c82d8ca0832699a13414dd664323f8f7c5a868",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/042367ed65483a82af9bf21f3250a0450a0e21ca"
        },
        "date": 1784144744487,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11415256,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9308278,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11413300,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1754980,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 541536,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 199933,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26656,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678093,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525393,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1058323,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3097255,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 508533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17438,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774394,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25193,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2851ca3384e14253c69d71ccee54e9c234d4f020",
          "message": "fix(signal): serialize sender-key mutations (#1043)",
          "timestamp": "2026-07-15T20:27:40-03:00",
          "tree_id": "d5432a7beb01b9d3e1b5693d8959e5b205b487b5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2851ca3384e14253c69d71ccee54e9c234d4f020"
        },
        "date": 1784158588246,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11424952,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9317174,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11425604,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1761742,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 542137,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 152011,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 199933,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 162047,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 26656,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1678093,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 525393,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 44737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 10726,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1059764,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 3097255,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 508533,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17438,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 776627,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25235,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 472,
            "unit": "crates"
          }
        ]
      }
    ]
  }
}