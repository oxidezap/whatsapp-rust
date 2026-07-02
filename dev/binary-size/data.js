window.BENCHMARK_DATA = {
  "lastUpdate": 1783019690568,
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
      }
    ]
  }
}