window.BENCHMARK_DATA = {
  "lastUpdate": 1781790730150,
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
      }
    ]
  }
}