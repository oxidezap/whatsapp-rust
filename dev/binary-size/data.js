window.BENCHMARK_DATA = {
  "lastUpdate": 1793363007981,
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
          "id": "f966cd0f80bd48595af483387b14e84399886e60",
          "message": "perf(bench): add connected post-activity retention baseline (#1590)",
          "timestamp": "2026-10-01T11:19:40-03:00",
          "tree_id": "d8220cf56337048c77d1e6b8943a0f757e3e9f30",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f966cd0f80bd48595af483387b14e84399886e60"
        },
        "date": 1790865029811,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11461816,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9257654,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11458286,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2305340,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41209,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1020529,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2150233,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 943469,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29826,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "7b1fb560fd655d196257562490bf9e148d4f7769",
          "message": "feat(api): unify chatstate subscriptions and bound event delivery (#1587)",
          "timestamp": "2026-10-01T11:15:10-03:00",
          "tree_id": "7417d227d5851346f1c5d5506ee8bb7666041f97",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7b1fb560fd655d196257562490bf9e148d4f7769"
        },
        "date": 1790865051197,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11461816,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9257654,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11458286,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2305340,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41209,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1020529,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2150233,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 943469,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29826,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "7daae478e218c5eeb4a804ace13cee840f282b72",
          "message": "feat(transport): retain canonical upgrade for caller-dialed streams (#1591)",
          "timestamp": "2026-10-01T11:47:36-03:00",
          "tree_id": "04460c1b5a023dc0e98014a0bef8ce7243784b46",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7daae478e218c5eeb4a804ace13cee840f282b72"
        },
        "date": 1790866725008,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11502744,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9286262,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11499102,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2305340,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1021954,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159285,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 943615,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29832,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "808aac451d1fcc5d387a29113fc81b086fa72549",
          "message": "refactor(api)!: align group lookup results and hierarchy access (#1583)",
          "timestamp": "2026-10-01T11:47:55-03:00",
          "tree_id": "46f78ce80f8d07af54f7ef92352ca8c44944a148",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/808aac451d1fcc5d387a29113fc81b086fa72549"
        },
        "date": 1790866796398,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11502744,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9286262,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11499102,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2305340,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1021954,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159285,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 943659,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29834,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "3013612f3c71fcae622b26fa0ea84eed9ca03803",
          "message": "feat(store): observe release of crate-owned backend holds (#1593)",
          "timestamp": "2026-10-01T17:07:08-03:00",
          "tree_id": "4975e9c34228dd8a07a389a69ec836fe76bea599",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3013612f3c71fcae622b26fa0ea84eed9ca03803"
        },
        "date": 1790886298669,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11503544,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9286966,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11503262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2305886,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022060,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159254,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 943873,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29855,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "98c18a453e7c044bc3f2d6faa01485eb8241a1c7",
          "message": "feat(events): expose authoritative EventKind enumeration (#1592)",
          "timestamp": "2026-10-01T17:07:56-03:00",
          "tree_id": "61eeac2b1b61f0546ad270eab0abef1cd0daeb5d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/98c18a453e7c044bc3f2d6faa01485eb8241a1c7"
        },
        "date": 1790886355407,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11503544,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9286966,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11503262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2305886,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022060,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159254,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 943873,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29855,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "4cee59ad88624f1d8c8b3162a00d3baa273ef08b",
          "message": "feat(lifecycle): add optional host admission for run-loop dials (#1594)",
          "timestamp": "2026-10-01T17:33:30-03:00",
          "tree_id": "3f43e63b6f92e0402cefdbfbe0cf4a8cee58458d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4cee59ad88624f1d8c8b3162a00d3baa273ef08b"
        },
        "date": 1790887571730,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11506456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9289526,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11503446,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2308099,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022402,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159254,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 944756,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29893,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "0cab808d3aae34f4aaae0918d0b9d542c549f56a",
          "message": "test(store): reject premature release in public consumer (#1595)",
          "timestamp": "2026-10-01T18:19:44-03:00",
          "tree_id": "bc4ea0eeb7e44a585a460a8c71e86796402ee2ba",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0cab808d3aae34f4aaae0918d0b9d542c549f56a"
        },
        "date": 1790890537494,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11506456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9289526,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11503446,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2308099,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022402,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159254,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 944756,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29893,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "2e1b34015382498d37407f578be54bcff8443481",
          "message": "fix(keepalive): track authenticated IQ progress in TCP test (#1598)",
          "timestamp": "2026-10-01T21:01:31-03:00",
          "tree_id": "77407d2e17685456f2577ad6c3d201809fbe6073",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2e1b34015382498d37407f578be54bcff8443481"
        },
        "date": 1790899862129,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11506456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9289526,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11503446,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2308099,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878408,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022402,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159254,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612271,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20233,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 944756,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29893,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "f1e117473076cf978eff31b512cfe43e6440c488",
          "message": "feat(history): surface pairing call-log records as typed events (#1596)",
          "timestamp": "2026-10-01T22:37:14-03:00",
          "tree_id": "7a2d107d55b9881b7bc6fee45eebc29519f823d7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f1e117473076cf978eff31b512cfe43e6440c488"
        },
        "date": 1790906679671,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11510616,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9293366,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11507558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2310027,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 880760,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022764,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159254,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612555,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20237,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 945739,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29922,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "7dca794ba573c5ddce5d7c31ff259ec0387bf003",
          "message": "fix(send): prefer valid trusted-contact tokens (#1597)",
          "timestamp": "2026-10-01T22:37:36-03:00",
          "tree_id": "3eafe75cf85acb095e8e18d2639d9cc09dd0dd3a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7dca794ba573c5ddce5d7c31ff259ec0387bf003"
        },
        "date": 1790906797321,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11510424,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9293238,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11507534,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2309947,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 880760,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1022700,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159254,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612555,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20237,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 945685,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29922,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "107165614+SecondNewtonLaw@users.noreply.github.com",
            "name": "NaN",
            "username": "SecondNewtonLaw"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9e24e4c0cf6682110a3fdf895afef969e280ba2e",
          "message": "feat: explicit PDO retry and music artwork downloads (#1577)\n\nCo-authored-by: João Lucas <jlucaso@hotmail.com>\nCo-authored-by: João Lucas <55464917+jlucaso1@users.noreply.github.com>",
          "timestamp": "2026-10-01T23:04:43-03:00",
          "tree_id": "170de7daa66e64c1e1019c5cc9f1ea0bf672aaf0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9e24e4c0cf6682110a3fdf895afef969e280ba2e"
        },
        "date": 1790907134844,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11546712,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9325750,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11545362,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2342907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881337,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1026232,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2154297,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612737,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20242,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 955744,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30265,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "5229afc13228379998593c54793199b0a74042ff",
          "message": "fix(pdo): preserve unsent automatic reservations on retry (#1600)\n\nCo-authored-by: SecondNewtonLaw <107165614+SecondNewtonLaw@users.noreply.github.com>",
          "timestamp": "2026-10-02T10:01:34-03:00",
          "tree_id": "e296d639329b172d20bd69c2fefcbdfde1dc3496",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5229afc13228379998593c54793199b0a74042ff"
        },
        "date": 1790946850502,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11545304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9324598,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11545366,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2338770,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881335,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024274,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159374,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612737,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20242,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 955935,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30266,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "ca5041ae8db44de0bbb8b9386a708dab1e94e48c",
          "message": "fix(bench): isolate connected-idle initialization from maintenance (#1599)",
          "timestamp": "2026-10-02T10:00:12-03:00",
          "tree_id": "8ceec12c0b45ed4b9470daa6f408bc6c20b267d7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ca5041ae8db44de0bbb8b9386a708dab1e94e48c"
        },
        "date": 1790947014655,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11546712,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9325750,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11545362,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2337830,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881337,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1026232,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2159374,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612737,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20242,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 955744,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30265,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "d9f78b806f1f4ca80c8008caa5846e5d542c2c55",
          "message": "feat: add typed message references and distinct content/operation IDs (#1601)",
          "timestamp": "2026-10-02T13:41:04-03:00",
          "tree_id": "9c8de1a007ca333d0ff9e273b9fd425b1949a08a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d9f78b806f1f4ca80c8008caa5846e5d542c2c55"
        },
        "date": 1790960181585,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551032,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9329910,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11549710,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339049,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024556,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163399,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613754,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 956655,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30281,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "b66241a28c9d9bace433701ca8e31d5c0244c84a",
          "message": "refactor(pictures)!: consolidate lookups on explicit requests (#1611)",
          "timestamp": "2026-10-03T10:38:05-03:00",
          "tree_id": "c3f38fd0ea832792b512e5cc79374f3ae10968de",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b66241a28c9d9bace433701ca8e31d5c0244c84a"
        },
        "date": 1791035511855,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551032,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9329910,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11549710,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339049,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024556,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163399,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613331,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 956600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30278,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "d61021a8b87a36df9a2df404b9aa330da8d3eace",
          "message": "refactor(download)!: remove redundant parameter aliases (#1612)",
          "timestamp": "2026-10-03T10:37:28-03:00",
          "tree_id": "497fbc467a5cc99447433838253630449735f12a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d61021a8b87a36df9a2df404b9aa330da8d3eace"
        },
        "date": 1791035523047,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551032,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9329910,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11549710,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339049,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024556,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163399,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613754,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 956655,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30281,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "ade7f4d736c12401903ec69d1ff7d363180d435a",
          "message": "refactor(voip)!: own the single-reader call event queue (#1609)",
          "timestamp": "2026-10-03T10:39:27-03:00",
          "tree_id": "55c6e6ca4dc168edd1466327026c46910bfbd941",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ade7f4d736c12401903ec69d1ff7d363180d435a"
        },
        "date": 1791035524922,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551032,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9329910,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11549710,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339049,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024556,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163399,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613331,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 956600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30278,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "2996ee924a6bdaca0e71eccf342ca6e32390d01f",
          "message": "refactor(api): encapsulate client implementation state (#1608)",
          "timestamp": "2026-10-03T10:51:55-03:00",
          "tree_id": "294f88082cd54fb17411eac03beec5acfe1d8482",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2996ee924a6bdaca0e71eccf342ca6e32390d01f"
        },
        "date": 1791035959787,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11549848,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9328502,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11549734,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2342587,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024662,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2158306,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613331,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957132,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30303,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "9f5be60f97f95ed218cd825b24d059abe4807fb3",
          "message": "refactor(mex)!: consolidate request construction and execution (#1605)",
          "timestamp": "2026-10-03T10:56:06-03:00",
          "tree_id": "6de5d5ef4f30c42204a081d6cfd63828b01ebe3a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9f5be60f97f95ed218cd825b24d059abe4807fb3"
        },
        "date": 1791036263048,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11549208,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9327862,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11549734,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2336843,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 625285,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024662,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163383,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613331,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 956976,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30303,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "9ceecd6ea1f7469c7c3307ce4cd2afbc6ceaf64c",
          "message": "refactor(sqlite): consolidate construction and database administration (#1603)",
          "timestamp": "2026-10-03T10:57:32-03:00",
          "tree_id": "8ce1999ecf0538d4e87fa1e9408074a873bcf9fa",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9ceecd6ea1f7469c7c3307ce4cd2afbc6ceaf64c"
        },
        "date": 1791036296195,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11547512,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9326518,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11545590,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2341920,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624788,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024263,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2157923,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613331,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 956976,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30303,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "66f5e180b08f9ac2c458d19290d9e666f047834b",
          "message": "refactor(groups): remove redundant hierarchy and call-action aliases (#1604)",
          "timestamp": "2026-10-03T14:40:43-03:00",
          "tree_id": "19548a8e6af671fe82b8cbbf237133525c41883d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/66f5e180b08f9ac2c458d19290d9e666f047834b"
        },
        "date": 1791050177378,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11547512,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9326518,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11545590,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2336843,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624788,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024263,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163000,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613331,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 956939,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30301,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "37943747+caumeira@users.noreply.github.com",
            "name": "Carlos Meira",
            "username": "caumeira"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "446592213314dea72c6f858dc2f7762d22ccc83f",
          "message": "feat(appstate): surface the unarchive-chats setting as a typed event (#1613)",
          "timestamp": "2026-10-03T16:35:13-03:00",
          "tree_id": "03ce5d46895fbeebd50983a2cb096a94fab12a48",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/446592213314dea72c6f858dc2f7762d22ccc83f"
        },
        "date": 1791057076859,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11548216,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9327222,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11545590,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2337515,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881958,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624788,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024263,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163000,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613342,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20274,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957205,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30304,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "87477fcc384e4cb148755ddee102d9e674f7ded8",
          "message": "feat(secrets): name poll/event creations and validate stored reads (#1610)",
          "timestamp": "2026-10-03T16:46:20-03:00",
          "tree_id": "3cbe82351476b05f76edf91b6e353d3099b198f1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/87477fcc384e4cb148755ddee102d9e674f7ded8"
        },
        "date": 1791058107003,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551864,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9330230,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11550290,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2344686,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024347,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2157981,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958014,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30324,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "07521ecd624fe4020e5241e3fe580f54f265f6d9",
          "message": "refactor(lifecycle): consolidate construction and expose logout outcomes (#1606)",
          "timestamp": "2026-10-03T17:07:42-03:00",
          "tree_id": "0646899bac1033c5ac2d2366dd57a9b7b03fb75b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/07521ecd624fe4020e5241e3fe580f54f265f6d9"
        },
        "date": 1791058960877,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9330166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11550266,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339513,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024331,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163058,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957944,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30319,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "8a092eb21fb7acf4ac3be1a5ca028132e47beec3",
          "message": "refactor(events): consolidate subscriptions and explicit unbounded delivery (#1607)",
          "timestamp": "2026-10-03T17:53:06-03:00",
          "tree_id": "0cca9610a368b37a22bb02171ac06fea1a04aa4c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8a092eb21fb7acf4ac3be1a5ca028132e47beec3"
        },
        "date": 1791061715533,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9330166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11550266,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339513,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024331,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163058,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957896,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30318,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "49e300598c68d03695c07b323921b6282f91a04f",
          "message": "test(groups): prove legacy alias rejection through public imports (#1618)",
          "timestamp": "2026-10-03T19:23:22-03:00",
          "tree_id": "4429ed8dcdea74e1226c04398f149af6746187e1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/49e300598c68d03695c07b323921b6282f91a04f"
        },
        "date": 1791067279960,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9330166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11550266,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339513,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024331,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163058,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957896,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30318,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "yousefamar@gmail.com",
            "name": "Yousef Amar",
            "username": "yousefamar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "99a9a1181fefef825daafed68f4eec334eaac75f",
          "message": "feat(voip/mlow): encode inactive frames, so a sender's silence is not flagged as active speech (#1616)",
          "timestamp": "2026-10-03T19:24:43-03:00",
          "tree_id": "c3e4fbc51809c76bf6b940e6519ca00f2be89206",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/99a9a1181fefef825daafed68f4eec334eaac75f"
        },
        "date": 1791067929097,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9330166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11550266,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2344590,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024331,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2157981,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957896,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30318,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "fa99009a5e65a4f367def9087703923e8f1be571",
          "message": "fix(voip): preserve invited group-call join ownership and PID continuity (#1617)\n\nCo-authored-by: Artjosh <51726170+Artjosh@users.noreply.github.com>",
          "timestamp": "2026-10-03T19:26:53-03:00",
          "tree_id": "99f293fdf982067c21d343120f9bc1629cf8563d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fa99009a5e65a4f367def9087703923e8f1be571"
        },
        "date": 1791068017629,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9330166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11550266,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2344590,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024331,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2157981,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957896,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30318,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "yousef@amar.io",
            "name": "Yousef Amar",
            "username": "yousefamar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0aaba75d1959347609d4db86ae47b7e719de5320",
          "message": "docs(voip/mlow): the encoder's docs describe the activity-dependent TOC, not active-only 0x50 (#1620)",
          "timestamp": "2026-10-03T20:45:53-03:00",
          "tree_id": "f3b37218344f88460f712950073f9aa6883c515e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0aaba75d1959347609d4db86ae47b7e719de5320"
        },
        "date": 1791072177336,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9330166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11550266,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339513,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024331,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163058,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957896,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30318,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "e0225b3aaa15e198f512912e3a8e0ecf256b97aa",
          "message": "fix(voip): apply independent relay and committed admission updates (#1621)\n\nCo-authored-by: Artjosh <51726170+Artjosh@users.noreply.github.com>",
          "timestamp": "2026-10-03T21:29:52-03:00",
          "tree_id": "5f57f746112135cf27def4d4c894ae0f54e86bdc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e0225b3aaa15e198f512912e3a8e0ecf256b97aa"
        },
        "date": 1791074745363,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11551768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9330166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11550266,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2339513,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1024331,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2163058,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20273,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 957896,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30318,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "c009f8d9d9d2afed781c5508037f1e2394d5e62d",
          "message": "fix(receipt): send buffered peer receipts individually (#1622)",
          "timestamp": "2026-10-03T22:11:29-03:00",
          "tree_id": "32836cc2d3537c28ae3a264d4e9a1f129c31ad3f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c009f8d9d9d2afed781c5508037f1e2394d5e62d"
        },
        "date": 1791077084034,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566200,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9343990,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355783,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011559,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958281,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30336,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "bc83dd4db3526a9b966cb877650cac12a359d3e0",
          "message": "feat(api)!: add canonical send and edit requests (#1614)",
          "timestamp": "2026-10-03T22:10:44-03:00",
          "tree_id": "4f0e65b39c20361dc832e8fcad7f114a11feb182",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bc83dd4db3526a9b966cb877650cac12a359d3e0"
        },
        "date": 1791077084502,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11565752,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9343542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355387,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011559,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958220,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30337,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "e32ec562a207c95188693ec1a2738ea69c08fa48",
          "message": "perf(receipt): avoid unused buffered peer receipt vectors (#1623)",
          "timestamp": "2026-10-03T23:14:30-03:00",
          "tree_id": "a62d69732f939dce579c53406d088273241e0869",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e32ec562a207c95188693ec1a2738ea69c08fa48"
        },
        "date": 1791080931874,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958458,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30336,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "102ef4084d2a6e8d41a01e01591d1eb0837d370b",
          "message": "fix(tooling): patch Wasmtime 48 security advisories (#1625)",
          "timestamp": "2026-10-03T23:59:14-03:00",
          "tree_id": "341ec77b015ba80045a357f1d78be0414f3460ca",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/102ef4084d2a6e8d41a01e01591d1eb0837d370b"
        },
        "date": 1791085083626,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958458,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30336,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "45d5ce6865681af7a8f7eb6d75cd5556ed135d97",
          "message": "fix(oracle): stop dereferencing absent MLOW centroids (#1633)",
          "timestamp": "2026-10-04T08:52:01-03:00",
          "tree_id": "9069bc6cc5b50c81446aa70908ccc5f3ec8d82ba",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/45d5ce6865681af7a8f7eb6d75cd5556ed135d97"
        },
        "date": 1791115101788,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958458,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30336,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "83977361c0d2839bf52cede58e3c680e87780893",
          "message": "refactor(api): move canonical picture lookup to a neutral facade (#1626)",
          "timestamp": "2026-10-04T08:53:25-03:00",
          "tree_id": "8d089a62e9a035b2ca7cca7191ec2b9b1dc192a9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/83977361c0d2839bf52cede58e3c680e87780893"
        },
        "date": 1791115139145,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958458,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30336,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "b6de7abb6e9e3100c796aa082b08a7bfcfe083a2",
          "message": "refactor(voip): remove duplicate legacy video state events (#1624)",
          "timestamp": "2026-10-04T08:53:00-03:00",
          "tree_id": "37f6d6c72aa5be119d35b3a6aa296e4ffe472673",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b6de7abb6e9e3100c796aa082b08a7bfcfe083a2"
        },
        "date": 1791115142859,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958458,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30336,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "b28164aba35d8904f0d2faa8fa15dcd7dc22f695",
          "message": "feat(groups): evolution-safe group DTO construction and mocks (#1627)",
          "timestamp": "2026-10-04T08:54:12-03:00",
          "tree_id": "58ed65ed8e4f70e578759da6e3647ef77dac3840",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b28164aba35d8904f0d2faa8fa15dcd7dc22f695"
        },
        "date": 1791115395866,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958458,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30336,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "4a88c5fd66a2c9b71cc3f0c5f4241fc2c594fcdd",
          "message": "feat(api)!: make media business and cache DTOs extension-safe (#1628)",
          "timestamp": "2026-10-04T08:54:33-03:00",
          "tree_id": "dec55937e61d24506147ffcb366ae534ea800bce",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4a88c5fd66a2c9b71cc3f0c5f4241fc2c594fcdd"
        },
        "date": 1791115398570,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958570,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30348,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "1fb74734ab8d19345f129555318709b5c75447a9",
          "message": "ci: gate independent API consumers through Rust xtask (#1629)",
          "timestamp": "2026-10-04T08:55:24-03:00",
          "tree_id": "b57b8666530c49b2229eb9b88b0b76f4d9daf020",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1fb74734ab8d19345f129555318709b5c75447a9"
        },
        "date": 1791115537946,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958570,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30348,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "16d9b2f369b57e8a7b336e5463aedb595990e81d",
          "message": "feat(api)!: make typed message actions canonical and extend DTO construction (#1630)",
          "timestamp": "2026-10-04T08:55:51-03:00",
          "tree_id": "f489f48e40fcd3e27f200e7a8fe0b444fec110cc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/16d9b2f369b57e8a7b336e5463aedb595990e81d"
        },
        "date": 1791115601177,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566392,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2361054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2167859,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958570,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30348,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "fb6636ccbe19afa886bf528c4e5b34612ac5a97d",
          "message": "refactor(status): type content and operation IDs and seal send options (#1632)",
          "timestamp": "2026-10-04T08:57:02-03:00",
          "tree_id": "1deda1daf82de8ce61ed196390c3aeaba8b5612a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fb6636ccbe19afa886bf528c4e5b34612ac5a97d"
        },
        "date": 1791115609893,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566392,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 958769,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30352,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "41c4602d3b3a6bd1e6dbe0cb6e7eff0c47b2afaa",
          "message": "fix(api)!: validate community creation and preserve partial failures (#1631)",
          "timestamp": "2026-10-04T10:09:46-03:00",
          "tree_id": "96be18eeb518e5326b03e2ce3b1cd53ffb398552",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/41c4602d3b3a6bd1e6dbe0cb6e7eff0c47b2afaa"
        },
        "date": 1791119810279,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11566392,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9344182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566558,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2355977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 959343,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30363,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "0059ad910b0928f334181177d2cd915b8c5af4d1",
          "message": "fix(message): deduplicate SKDM-bearing PDO retries (#1635)",
          "timestamp": "2026-10-04T12:55:41-03:00",
          "tree_id": "23ba55242c60d34c0d55e5b573fc140713398e18",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0059ad910b0928f334181177d2cd915b8c5af4d1"
        },
        "date": 1791129756268,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11567576,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9345206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566534,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356986,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960200,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30370,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "6f7a62296242f5dab8967431bd1c6528c1339efe",
          "message": "test(message): serialize the offline PDO retry fixture (#1637)",
          "timestamp": "2026-10-04T14:03:37-03:00",
          "tree_id": "539bad0db8c91d2a61c452e16520689974d3cace",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6f7a62296242f5dab8967431bd1c6528c1339efe"
        },
        "date": 1791133798531,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11567576,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9345206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566534,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362063,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2167859,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960200,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30370,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "54ce6cd9007f4361a07dbad50718973bbf56e2e9",
          "message": "docs: remove task histories and shorten contributor guides (#1638)",
          "timestamp": "2026-10-04T14:26:56-03:00",
          "tree_id": "381f156fa6779b7622127bede21ade16f9d46e5e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/54ce6cd9007f4361a07dbad50718973bbf56e2e9"
        },
        "date": 1791135765467,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11567576,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9345206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566534,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356986,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960200,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30370,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "86b1b315af3be1a48e1961c0a72f3e42152f1a33",
          "message": "refactor(polls): prefer typed vote decryption and share secret generation (#1640)",
          "timestamp": "2026-10-04T15:54:39-03:00",
          "tree_id": "a89bc624784663d995009ec24c688cc9000a54c5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/86b1b315af3be1a48e1961c0a72f3e42152f1a33"
        },
        "date": 1791142057293,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11567576,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9345206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566534,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362063,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1829083,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2167859,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960565,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30376,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "9fbf23eaeacceae856ae3d305bcc6b800544cc36",
          "message": "perf(signal): remove transient identity and sender-key allocations (#1641)",
          "timestamp": "2026-10-04T17:35:17-03:00",
          "tree_id": "b974b85e14b03772640168423c723d3def682304",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9fbf23eaeacceae856ae3d305bcc6b800544cc36"
        },
        "date": 1791146727770,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11562936,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9340598,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11562518,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356901,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822889,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011639,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960646,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30378,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "8f711a9bf3e1ff556d2c8374702462a918e7b433",
          "message": "perf(receive): isolate cold PDO recovery state (#1642)",
          "timestamp": "2026-10-04T18:24:55-03:00",
          "tree_id": "3745dc86f0cd3f8f22a9a2bffcf92df7e8557b93",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8f711a9bf3e1ff556d2c8374702462a918e7b433"
        },
        "date": 1791149644686,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11562616,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9340406,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11562398,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356709,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822889,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011632,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30385,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "2024159f3e2a0ebad2854d70ebffc4067586cb8d",
          "message": "perf(receive): release drain encode arena on live transition (#1643)",
          "timestamp": "2026-10-04T19:44:11-03:00",
          "tree_id": "b4e2d6893ef75d3a53ee2d76339fad1d3f4c2f5b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2024159f3e2a0ebad2854d70ebffc4067586cb8d"
        },
        "date": 1791154812466,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11563128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9340854,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11562374,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2357120,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822889,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011632,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613657,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20279,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960723,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30385,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "228997d4e8da79aeea618b65cdeced2d8b2698c5",
          "message": "fix(api): roundtrip receipts and unify presence activity models (#1644)",
          "timestamp": "2026-10-04T20:54:58-03:00",
          "tree_id": "44b7969a364e51b5027d478ac582d303cc25fb88",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/228997d4e8da79aeea618b65cdeced2d8b2698c5"
        },
        "date": 1791159048976,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11563000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9340726,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11562374,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356991,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822889,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011632,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613721,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960351,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30377,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "02456d452b9ee3376b19934f084ccba2e0a03b79",
          "message": "fix(api): preserve message identities and report pending push-name sync (#1645)",
          "timestamp": "2026-10-04T21:02:50-03:00",
          "tree_id": "0d038cd7f867cb6405d91b94c4ff88a8dd3e4470",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/02456d452b9ee3376b19934f084ccba2e0a03b79"
        },
        "date": 1791159355943,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11563000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9340726,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11562374,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356988,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822889,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011632,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613721,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960293,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30375,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "9ef62ef44e9885e7cab116b9e525786abe8b169c",
          "message": "test: isolate retained-byte and allocation-size measurements (#1646)",
          "timestamp": "2026-10-04T21:46:10-03:00",
          "tree_id": "c9e807f129564359a981360473fc36643cf4714a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9ef62ef44e9885e7cab116b9e525786abe8b169c"
        },
        "date": 1791161606080,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11563000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9340726,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11562374,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356988,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822889,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011632,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172936,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613721,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 960293,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30375,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "8cf246a1487a90c61f38c0eb78fb86df7cc24731",
          "message": "fix(lifecycle): own waiters and coalesce correlated media reuploads (#1648)",
          "timestamp": "2026-10-04T23:10:06-03:00",
          "tree_id": "c28e5fd45f6724c16e13bf5441b4aa4e202b8340",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8cf246a1487a90c61f38c0eb78fb86df7cc24731"
        },
        "date": 1791166699149,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11564856,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9342326,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11566470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2358897,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 883290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822889,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624569,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1011291,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2173000,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 613759,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 967543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30685,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 607,
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
          "id": "93e5c7f75cdad98472efd218d9bff8a464cf47a7",
          "message": "fix(media): redact diagnostics and simplify portable download routes (#1647)",
          "timestamp": "2026-10-04T23:11:22-03:00",
          "tree_id": "df0a19126f621eaf3a8f95626d3df8cfc3c37b58",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/93e5c7f75cdad98472efd218d9bff8a464cf47a7"
        },
        "date": 1791166992981,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11521240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9290934,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11521874,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356085,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1820713,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624287,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964208,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2179629,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615170,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20296,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 968449,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30718,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "bd1f3069ac0fccec0060c1b1ee7aa54f4fa217a0",
          "message": "fix(groups): parse member mode values sent as raw bytes (#1651)",
          "timestamp": "2026-10-05T10:41:46-03:00",
          "tree_id": "ef83936a2e7e4fc9a44a378a71079c4f41c49465",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bd1f3069ac0fccec0060c1b1ee7aa54f4fa217a0"
        },
        "date": 1791208382432,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11520664,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9290358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11521874,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356085,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 877848,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1820713,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624287,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964208,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2179629,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615107,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20299,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 968449,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30718,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "b4e349989cc980ec7c9de6f35d95a3f5017b3914",
          "message": "refactor(chat-actions)!: build message ranges from typed references (#1652)",
          "timestamp": "2026-10-05T12:36:26-03:00",
          "tree_id": "2b2ebddcc2c43322dd95a5569d45fcd2da35a1da",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b4e349989cc980ec7c9de6f35d95a3f5017b3914"
        },
        "date": 1791216736781,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11520664,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9290358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11521874,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356085,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 877848,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1820713,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624287,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964208,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2179629,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615107,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20299,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 967740,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30684,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "39d3b776531230abf2f22e2c7b8544df6debc291",
          "message": "fix(media)!: schedule distinct reuploads and preserve phased failures (#1653)",
          "timestamp": "2026-10-05T13:46:48-03:00",
          "tree_id": "ff8ffaa3e8fe922fe0159e5337a42903aa80afff",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/39d3b776531230abf2f22e2c7b8544df6debc291"
        },
        "date": 1791219821977,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11520664,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9290358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11521874,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2356085,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 877848,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1820713,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624287,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964208,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2179629,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615107,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20299,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 968089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30688,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "f1080df166bbc2c88e762eda6eb5bb7f6efc2f05",
          "message": "fix(media): redact final diagnostics and compare IPv6 origins (#1654)",
          "timestamp": "2026-10-05T14:18:45-03:00",
          "tree_id": "4a2a7c36b43714c4ab38f94d0a2d18424ec585b3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f1080df166bbc2c88e762eda6eb5bb7f6efc2f05"
        },
        "date": 1791222663132,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11527736,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9292598,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11522538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362713,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878010,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1820713,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624287,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964597,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2174622,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615837,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20347,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 969084,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30708,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "ee89f5ad9cf0553825f201f0e264ad32c4b73966",
          "message": "fix(media): coalesce forced refreshes and preserve auth on expired downloads (#1659)",
          "timestamp": "2026-10-05T15:47:41-03:00",
          "tree_id": "d7f722832a7194ea0f60dbeb7dc0122d4abadaa9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ee89f5ad9cf0553825f201f0e264ad32c4b73966"
        },
        "date": 1791226871196,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9296758,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526618,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362171,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878010,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1820713,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624287,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964265,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2179699,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615837,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20347,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 969603,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30719,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "75211334+i-am-v-alexander-v@users.noreply.github.com",
            "name": "i-am-v-alexander-v",
            "username": "i-am-v-alexander-v"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e14ddca2a6ba8823e82473a3b6a438cd82010e3b",
          "message": "feat(history): let the durability hook capture history-sync chunks before their receipt (#1657)",
          "timestamp": "2026-10-05T15:48:09-03:00",
          "tree_id": "3720d97c444b7a0d4aa62a76ea17664ffee2ad03",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e14ddca2a6ba8823e82473a3b6a438cd82010e3b"
        },
        "date": 1791226944747,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11533208,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9298038,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11530730,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362971,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878127,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1820713,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624287,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964594,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2179699,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615837,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20347,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 969906,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30720,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "d804acaec1439e54c43c129e2f90d45016506abe",
          "message": "refactor(download)!: validate owned parameters before routing (#1656)",
          "timestamp": "2026-10-05T16:06:28-03:00",
          "tree_id": "e48742219f64617d7781fe298f91ea21bdd484a5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d804acaec1439e54c43c129e2f90d45016506abe"
        },
        "date": 1791227882205,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11533592,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9298422,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11530730,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363095,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878397,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1820713,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624287,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59268,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964594,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2179699,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 969974,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "4772c5dfd374bfad8075efbf4ff4030bb93b50fb",
          "message": "chore(deps): bump the cargo-major group with 2 updates (#1663)\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-05T19:25:47-03:00",
          "tree_id": "8c3285feba4ac3da117621972785c6368719df2b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4772c5dfd374bfad8075efbf4ff4030bb93b50fb"
        },
        "date": 1791240004848,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531096,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297014,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2366841,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962403,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176426,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 969974,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "166c46c1ad1257b1c78b94f3a27cb1f3f24bea1d",
          "message": "chore(deps): bump the cargo group with 3 updates (#1662)\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-05T19:25:32-03:00",
          "tree_id": "a5d44270f203019b2923bdfb74652b84e8c0fd5a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/166c46c1ad1257b1c78b94f3a27cb1f3f24bea1d"
        },
        "date": 1791240016228,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531096,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297014,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2361764,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962403,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 969974,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "7ed5b5d154a2fb6ecb1e0f50fd6a08d60385b48b",
          "message": "test(history): close durability receipt coverage gaps (#1661)",
          "timestamp": "2026-10-05T19:26:03-03:00",
          "tree_id": "271103879d13efac5945d066eda1af8c2df78695",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7ed5b5d154a2fb6ecb1e0f50fd6a08d60385b48b"
        },
        "date": 1791240464104,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531160,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297078,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2361796,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962403,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 969987,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "d515abaf63a3e56af77d4e902f91a929da0e4a81",
          "message": "fix(lifecycle): recheck host admission before queued dials (#1665)",
          "timestamp": "2026-10-06T12:15:02-03:00",
          "tree_id": "d06a06a2f43fb08a227ac61b6cd4df19aeadcdb2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d515abaf63a3e56af77d4e902f91a929da0e4a81"
        },
        "date": 1791300681951,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362092,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970051,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30723,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "05961f4365ae006d1248e2373c476be26f9ed1cf",
          "message": "fix(derive): preserve WireEnum tag visibility and extensibility (#1666)",
          "timestamp": "2026-10-06T14:24:39-03:00",
          "tree_id": "2c533e0a21d7c2bac9a0cbbc472ceba5787e656b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/05961f4365ae006d1248e2373c476be26f9ed1cf"
        },
        "date": 1791309037522,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362092,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970051,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30723,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "dfde9f09adc946cfe2c59a2ef58a4843d562e7f3",
          "message": "refactor(api): keep pairing dispatch and session telemetry internal (#1668)",
          "timestamp": "2026-10-06T14:39:51-03:00",
          "tree_id": "168d9faf5448d514db096c56491c0701d34820c5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dfde9f09adc946cfe2c59a2ef58a4843d562e7f3"
        },
        "date": 1791310435989,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2367173,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176426,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "a7290f6a7d21c9246fd51892d007ef0b5b633a95",
          "message": "fix(chatstate): remove incoming-only stanza's panicking conversion (#1667)",
          "timestamp": "2026-10-06T14:40:13-03:00",
          "tree_id": "4bd0dc560ba81979d5099ffa30abf1c431fad969",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a7290f6a7d21c9246fd51892d007ef0b5b633a95"
        },
        "date": 1791310846066,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362096,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615779,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20317,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "0b8732aedbc8d53e29b3d5a23253a812993492e4",
          "message": "refactor(socket): keep sender queue jobs private (#1670)",
          "timestamp": "2026-10-06T16:36:49-03:00",
          "tree_id": "785094fff5dd11006b57920966fe40f6695ebd24",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0b8732aedbc8d53e29b3d5a23253a812993492e4"
        },
        "date": 1791319799863,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362096,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615779,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20317,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "2e6b944b67f8b1ae430d772e5bbbcf7fd20da700",
          "message": "fix(api): make server and media host classifications extensible (#1669)",
          "timestamp": "2026-10-06T17:39:21-03:00",
          "tree_id": "d1f7dacb8a531d32b4021a7189928993ef2f44f3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2e6b944b67f8b1ae430d772e5bbbcf7fd20da700"
        },
        "date": 1791321659728,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362096,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615779,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20317,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "a18fafe49301bc3693071822fc2a5992e100283a",
          "message": "fix(api): make passkey and SDK metadata evolvable (#1673)",
          "timestamp": "2026-10-06T18:25:04-03:00",
          "tree_id": "05e78e556c34b247ed8bbe4afb280eae125b5806",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a18fafe49301bc3693071822fc2a5992e100283a"
        },
        "date": 1791324717580,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362096,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615779,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20317,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "8125326a7b885a8d0e5a5134e706255017cb0435",
          "message": "fix(api): prepare HTTP hosts and network contracts for evolution (#1674)",
          "timestamp": "2026-10-06T19:38:52-03:00",
          "tree_id": "184918e9a57c79d201ac2a746a84aea7d1533595",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8125326a7b885a8d0e5a5134e706255017cb0435"
        },
        "date": 1791329018009,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362096,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615779,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20317,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "8f76dc4eac433e9898752a6d64a4cdb6a9c2e034",
          "message": "test(storage): qualify v0.7 historical fixtures through upgrade and restart (#1675)",
          "timestamp": "2026-10-06T19:43:42-03:00",
          "tree_id": "78681a5713bff1b49fa75f0394998b50e58210a3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8f76dc4eac433e9898752a6d64a4cdb6a9c2e034"
        },
        "date": 1791329207019,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11531480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9297398,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11526582,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362096,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 879949,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962412,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181503,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 615779,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20317,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "bac94e7c3c83fb7a283f3b551b233c476b283cfa",
          "message": "fix(iq): preserve typed missing response fields (#1677)",
          "timestamp": "2026-10-06T19:46:15-03:00",
          "tree_id": "2e05f48f614d7c684e06729ab6656fcdfdd49dfb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bac94e7c3c83fb7a283f3b551b233c476b283cfa"
        },
        "date": 1791329886732,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11533816,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9298806,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531374,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2361785,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616319,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20350,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 611,
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
          "id": "6c5f8d557ab1329c55d16e6633bf83fa04f98f18",
          "message": "fix(ib): warn once per client for unsupported known bulletins (#1692)",
          "timestamp": "2026-10-06T22:31:48-03:00",
          "tree_id": "425a1c68c974d97e7c859f98bcc48d3f9367619a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6c5f8d557ab1329c55d16e6633bf83fa04f98f18"
        },
        "date": 1791337423702,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11534904,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9299766,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2362748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616319,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20350,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970227,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "885ae76e7f9301903531f9b12f1b9fd9060e4727",
          "message": "ci(release): require compatibility, WASM and packaged consumer qualification (#1671)",
          "timestamp": "2026-10-06T22:31:28-03:00",
          "tree_id": "d1780f370b7c13fa840989132da832236fbdad4e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/885ae76e7f9301903531f9b12f1b9fd9060e4727"
        },
        "date": 1791337462756,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11533816,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9298806,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531374,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2361785,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616319,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20350,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30721,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "dc8a19b327d1886072d4a8f4ba47971bab110ff7",
          "message": "fix(labels): preserve optional action timestamps (#1691)",
          "timestamp": "2026-10-06T22:32:22-03:00",
          "tree_id": "22988219b0615c8967299a4189bc660c5aa0e927",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dc8a19b327d1886072d4a8f4ba47971bab110ff7"
        },
        "date": 1791337728182,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535288,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300150,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363092,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616319,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20350,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970318,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "8ef8d106dc15160cfde6cf278307f164be4425c8",
          "message": "feat(appstate): make the generated action catalog extensible (#1682)",
          "timestamp": "2026-10-06T22:49:44-03:00",
          "tree_id": "350fee8e1f59b48e881a115590e49b42265eb2cb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8ef8d106dc15160cfde6cf278307f164be4425c8"
        },
        "date": 1791347849972,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535288,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300150,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363092,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616319,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20350,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970318,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "5e128dee904f6cae0ff302456c7b4b1f229116e9",
          "message": "fix(newsletter): preserve add-on response parse causes (#1678)",
          "timestamp": "2026-10-07T07:33:57-03:00",
          "tree_id": "d3bd0d84ae5f55527237a5f4919bf246e59f3590",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5e128dee904f6cae0ff302456c7b4b1f229116e9"
        },
        "date": 1791370313963,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535288,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300150,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363092,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616319,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20350,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970318,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "46a110a6aa7f17a8f138bf083eca007482c51a95",
          "message": "refactor(api): internalize PDO dispatch and gate call test helpers (#1680)",
          "timestamp": "2026-10-07T07:34:34-03:00",
          "tree_id": "b13c1fc28e37a2f3bb2372c88448076052cbfa77",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/46a110a6aa7f17a8f138bf083eca007482c51a95"
        },
        "date": 1791370456409,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535288,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300150,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363092,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616068,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970318,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "4d08cf49ef8b786198c19a35b5f10d624bcf166b",
          "message": "test(wasm): consolidate execution in the existing API consumer (#1683)",
          "timestamp": "2026-10-07T07:35:02-03:00",
          "tree_id": "3ac4f90ac9e71664ad3a8f84908587ba9bb6dfa7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4d08cf49ef8b786198c19a35b5f10d624bcf166b"
        },
        "date": 1791370560692,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535288,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300150,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363092,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41282,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616068,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970318,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "0bbe1672fc4bda1354022550b114f4f16378c0b2",
          "message": "fix(appstate): derive runtime collections from the action catalog (#1696)",
          "timestamp": "2026-10-07T07:35:46-03:00",
          "tree_id": "460c9a681131373cdb08a5c2d420349090d1acab",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0bbe1672fc4bda1354022550b114f4f16378c0b2"
        },
        "date": 1791370914009,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535224,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300086,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2368121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176496,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616068,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970283,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "c05cf5b22c9a045e9de1137fe0c1cf83887f096c",
          "message": "ci: defer binary size diagnostics until a failed PR budget (#1697)",
          "timestamp": "2026-10-07T07:36:12-03:00",
          "tree_id": "42007a294a0db6426f25171a1a211f1f7c5df11f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c05cf5b22c9a045e9de1137fe0c1cf83887f096c"
        },
        "date": 1791370931541,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535224,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300086,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363044,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616068,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970283,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "5d107f508b776daf1e63ffb66aeac1b8153a860e",
          "message": "ci(consumers): honor bounded Cargo build parallelism (#1695)",
          "timestamp": "2026-10-07T10:58:16-03:00",
          "tree_id": "8c8b7c512432dc9a6743788753299f3f733b85d7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5d107f508b776daf1e63ffb66aeac1b8153a860e"
        },
        "date": 1791383809218,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535224,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300086,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531342,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363044,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962610,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616068,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970283,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30722,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "ec50ede9c38c52c33b7473e2dcd1ceb31af36450",
          "message": "fix(store)!: require atomic token merges and patch commits (#1681)",
          "timestamp": "2026-10-07T14:10:45-03:00",
          "tree_id": "46afc7cb8bb33f06f94aca1bad63304ce5fc954c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ec50ede9c38c52c33b7473e2dcd1ceb31af36450"
        },
        "date": 1791397123457,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11535704,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300470,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531366,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363479,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962567,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181573,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616328,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 970436,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30727,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "3eba9a638071f46c6428c7b30f55fc45b352fcd0",
          "message": "feat: configure history capture independently of message durability (#1672)",
          "timestamp": "2026-10-07T16:19:35-03:00",
          "tree_id": "d31b1ace9bdab09c9ed39ad8ffd61ba2252ed512",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3eba9a638071f46c6428c7b30f55fc45b352fcd0"
        },
        "date": 1791402165121,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11536184,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300918,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11535550,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363738,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962716,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181589,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616328,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 971067,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30754,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "22e911cacf89a642e513b5117d407ddfa30ec5e9",
          "message": "fix(tracing): classify IQ diagnostics and attribute lifecycle events (#1693)",
          "timestamp": "2026-10-07T19:03:18-03:00",
          "tree_id": "d69bf10b3bc0bc4781381bb05680c00a780b230c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/22e911cacf89a642e513b5117d407ddfa30ec5e9"
        },
        "date": 1791411586347,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11536184,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300918,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11535550,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363738,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881261,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962716,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181589,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616328,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 971067,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30754,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "a257693cf084dc8810e7d687e9a327ed5d7de6fb",
          "message": "refactor(api): gate cache and SFrame internals for benchmarks (#1684)",
          "timestamp": "2026-10-07T19:03:29-03:00",
          "tree_id": "afb6b533cbb9a20b4e2b1583b08e45dca63381d1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a257693cf084dc8810e7d687e9a327ed5d7de6fb"
        },
        "date": 1791411593868,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11536152,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300918,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531430,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363919,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881080,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962716,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181589,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616328,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 971067,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30754,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "edaa64b68d0de24bdb6848b98a824a32e276c55c",
          "message": "refactor(api): keep built-in stanza dispatch internal (#1685)",
          "timestamp": "2026-10-07T19:45:11-03:00",
          "tree_id": "86a1bf8f31c3c4137b50051e3578d076eaa8cb37",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/edaa64b68d0de24bdb6848b98a824a32e276c55c"
        },
        "date": 1791413792598,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11536152,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9300918,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11531430,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2363919,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881080,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962716,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181589,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616328,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 971067,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30754,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "21b7359c22a5f363abd9db97cd615494e05c0b95",
          "message": "fix(receipt): preserve Status actor and self-read direction (#1707)",
          "timestamp": "2026-10-08T17:13:49-03:00",
          "tree_id": "3c59bc704dc55ee93e862afac0cd045cf8d3ab25",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/21b7359c22a5f363abd9db97cd615494e05c0b95"
        },
        "date": 1791491910443,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11539032,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9303670,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11535510,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2366683,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881080,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 962716,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181589,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616328,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 971382,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30760,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "6d22cb3caa154ca73c1223ac66eefe9fc7540bd8",
          "message": "fix(pair): stop pairing timers with their connection (#1705)",
          "timestamp": "2026-10-08T18:16:53-03:00",
          "tree_id": "c22848c23aafa273832812fdedcb21cf3123fa73",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6d22cb3caa154ca73c1223ac66eefe9fc7540bd8"
        },
        "date": 1791495503663,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11548696,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9312822,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11547974,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2374512,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 881080,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 963923,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181589,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616328,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 974350,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30908,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "b32d0ae4d6988bebf85393715117a366c24d470a",
          "message": "fix(message): learn explicit self-sync recipient aliases conservatively (#1706)",
          "timestamp": "2026-10-08T20:10:00-03:00",
          "tree_id": "5bf05828e7792e257ee84520f6a1238ef7ccd7e8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b32d0ae4d6988bebf85393715117a366c24d470a"
        },
        "date": 1791504031150,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11556408,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9320182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11552158,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2384849,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882796,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964303,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176512,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616526,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20356,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 975462,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30935,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "aa0612ee31f9e592d72d44a7e0944b353096a926",
          "message": "feat(labels): observe sublist upserts and removals (#1704)",
          "timestamp": "2026-10-08T20:11:59-03:00",
          "tree_id": "5947b9d4c85c87f070f6eba03a331cbc0dc93669",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/aa0612ee31f9e592d72d44a7e0944b353096a926"
        },
        "date": 1791504410552,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11558520,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9322358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11556238,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2381925,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 882796,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195818,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41277,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822691,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624449,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 964331,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181589,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 616538,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20357,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 976372,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 30939,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "2ebadf581855fe5897b0f6cf9e5ae7fbd95130f4",
          "message": "build(codegen): refresh qualified whatspec pin and retain historical APIs (#1708)",
          "timestamp": "2026-10-09T09:42:32-03:00",
          "tree_id": "33a7b2e0b22fb1abf8055e6648760dbeedf97d82",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2ebadf581855fe5897b0f6cf9e5ae7fbd95130f4"
        },
        "date": 1791550687357,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11509944,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9276214,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11507086,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2394504,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 844054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195719,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41356,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1807187,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624243,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 959917,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181917,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543848,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18238,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 945538,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29823,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "5645364bc32883502da716cb69132fac16cdc0d4",
          "message": "build(iq): generate private builders and success payloads for two pilots (#1709)",
          "timestamp": "2026-10-09T12:20:48-03:00",
          "tree_id": "f2bc23beb841f234c9b543707ad4ed8cd63146d6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5645364bc32883502da716cb69132fac16cdc0d4"
        },
        "date": 1791562061349,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11509944,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9276214,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11507086,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2394504,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 844054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195719,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41356,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1807187,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624243,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 959917,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181917,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544250,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18254,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 945538,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29823,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "835e82250b83df57f4ab23bbeec656c2243b4169",
          "message": "fix(voip)!: preserve missing timestamps on group rekeys (#1711)",
          "timestamp": "2026-10-09T16:39:09-03:00",
          "tree_id": "febdc6b74d37fdf0bf39aed66a6a55f52cd7aa77",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/835e82250b83df57f4ab23bbeec656c2243b4169"
        },
        "date": 1791575453946,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11510200,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9276470,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11507086,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2394714,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 844054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195719,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41356,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1807187,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624243,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 959917,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181917,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544261,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18254,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 945545,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29823,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "94c53e833805c58c9f5966b08d08c990650a651e",
          "message": "bench(waproto): measure oneof encoding and future-field decoding (#1686)",
          "timestamp": "2026-10-09T16:49:02-03:00",
          "tree_id": "a09f9742e60f03503f1f3c12d52221ef91ce0233",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/94c53e833805c58c9f5966b08d08c990650a651e"
        },
        "date": 1791576975489,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11510200,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9276470,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11507086,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2394714,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 844054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195719,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41356,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1807187,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624243,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 959917,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181917,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544261,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18254,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 945545,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29823,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "6b29f52b8f3f0c1b7c7614b99ab3a86cb2a5554d",
          "message": "perf(signal): share immutable signing keys independently (#1694)",
          "timestamp": "2026-10-11T09:59:42-03:00",
          "tree_id": "28f233455e4cb21ef8007cee785a5f5c5eaab4b5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6b29f52b8f3f0c1b7c7614b99ab3a86cb2a5554d"
        },
        "date": 1791724616022,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11510072,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9276342,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11507102,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2394714,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 844054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82681,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41356,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1807187,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 624243,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12926,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 959861,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2181917,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544261,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18254,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 945361,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29816,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "3eeb80806ba456170018b114cd1fda66bc74329d",
          "message": "feat(waproto)!: protect generated API evolution before 1.0 (#1676)",
          "timestamp": "2026-10-11T14:41:06-03:00",
          "tree_id": "0bcc4f7adaac94c6607c714f3174f0147c405bfe",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3eeb80806ba456170018b114cd1fda66bc74329d"
        },
        "date": 1791741530273,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11512856,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9278710,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11511198,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2395161,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 842432,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86248,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195788,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41194,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1807211,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622856,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 959858,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2183364,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 541400,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18254,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935606,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29824,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "0dc38af07632f48f0cf6c66df285f1643fc22b95",
          "message": "ci: pin CodSpeed memory profiling toolchain (#1712)",
          "timestamp": "2026-10-12T15:22:09-03:00",
          "tree_id": "67d225076d8a301015c70360e66736574d571d8a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0dc38af07632f48f0cf6c66df285f1643fc22b95"
        },
        "date": 1791830134056,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11512856,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9278710,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11511198,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2395161,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 842432,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86248,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195788,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41194,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1807211,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622856,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 959858,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2183364,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 541400,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18254,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935606,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29824,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "fb970318c4dd47898a18ad6212ae17a4f530385b",
          "message": "fix: retain inbound plaintext across durability failures and reconnects (#1679)",
          "timestamp": "2026-10-13T19:46:37-03:00",
          "tree_id": "b1b52f519910580732bd183a88d31c8f4c5208ca",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fb970318c4dd47898a18ad6212ae17a4f530385b"
        },
        "date": 1791932501126,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11580792,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9324406,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11577870,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2477485,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82859,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195788,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41385,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582559,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59248,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12929,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 973678,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2173250,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545528,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 982805,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31284,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "1fb3133697be9375b155bf7fa0e9ed37b99fc2d3",
          "message": "fix(identity): learn app-state and business usync LID-PN pairs (#1715)",
          "timestamp": "2026-10-13T19:45:58-03:00",
          "tree_id": "8dd4d822573d20bc272aed53439728f0590d9519",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1fb3133697be9375b155bf7fa0e9ed37b99fc2d3"
        },
        "date": 1791932726414,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11527288,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9291894,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11523906,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2405939,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 844147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86248,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195788,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41194,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1807211,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622856,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59277,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 961316,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2182435,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 542433,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18278,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 940061,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29977,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "d9c8a565392bda889af063211f51b82e10845282",
          "message": "perf(message): remove per-batch session future allocation (#1716)",
          "timestamp": "2026-10-13T22:31:26-03:00",
          "tree_id": "b47d88efeb10770f0bb0afc87601fb6e728277ec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d9c8a565392bda889af063211f51b82e10845282"
        },
        "date": 1791942271025,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11599128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9341302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11598910,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2489670,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 846286,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82337,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195788,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41385,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582559,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59248,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 975993,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172910,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545528,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 988310,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31410,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoferro.contact@gmail.com",
            "name": "João Ferro",
            "username": "joaomdsferro"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c73489cd274dde403343076c1bc643ce640de083",
          "message": "fix(app-state): decode gzip peer recovery without bypassing snapshot MAC (#1698)\n\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-10-13T22:31:03-03:00",
          "tree_id": "48270c4f2652c77e493bb45954f5ef05817f5b16",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c73489cd274dde403343076c1bc643ce640de083"
        },
        "date": 1791942430213,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11598968,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9341430,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11598894,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2490018,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 846286,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82337,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195788,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41385,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582559,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59248,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 975778,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172910,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545528,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 988096,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31387,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoassp@gmail.com",
            "name": "Joao Paulo Assis",
            "username": "assisjp"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3e3abeb2e1e2b4ac720bf0236b7d14d88dea0641",
          "message": "feat(groups): gate notifications on durable capture (#1699)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-13T22:37:02-03:00",
          "tree_id": "d87f7e69231d8b433965200755034ccca6aa41e9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3e3abeb2e1e2b4ac720bf0236b7d14d88dea0641"
        },
        "date": 1791943122129,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11613176,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9353462,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11611630,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2498941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847337,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82337,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195788,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41385,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582559,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59248,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 977657,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2172926,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545528,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 992659,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31565,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 612,
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
          "id": "1f25f7e18faf328ed0dd8f57fc22db9fd44b1964",
          "message": "chore(deps): bump the cargo group with 13 updates (#1717)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-10-14T09:43:04-03:00",
          "tree_id": "696f3c4c8dace3387ad16a58b8664220edb5a235",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1f25f7e18faf328ed0dd8f57fc22db9fd44b1964"
        },
        "date": 1791982775965,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11607832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9348278,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11606722,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2495305,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 845132,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58749,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 977057,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2174627,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 992695,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31565,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "50a723d46fee6216f20d4daba9cd834a39261743",
          "message": "fix(receive): expose pre-decrypt admission refusals in metrics (#1723)",
          "timestamp": "2026-10-14T23:58:53-03:00",
          "tree_id": "ff8ef60c4ce0b64452864d0c93714d10b7e01495",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/50a723d46fee6216f20d4daba9cd834a39261743"
        },
        "date": 1792035202899,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11615000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9354358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11614946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2495839,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 845964,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 979393,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176914,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 992790,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31566,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "20c48f54ba9fbf0c3a1631f434d4d249dcb0edfe",
          "message": "fix(voip): keep a relay on the web-client port when it answers there too (#1721)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-15T09:40:02-03:00",
          "tree_id": "c404598973d026e0eca21b5385e8d90cebd10a91",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/20c48f54ba9fbf0c3a1631f434d4d249dcb0edfe"
        },
        "date": 1792069710475,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11615000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9354358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11614946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2495839,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 845964,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 979393,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176914,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 992790,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31566,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "85a83d0364943346ca940a02bee522aa4d458979",
          "message": "fix(voip): hear a WhatsApp peer's audio in a 1:1 video call (#1720)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-15T11:11:37-03:00",
          "tree_id": "932385a818e1329a154539e6101e7a5babfc284d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/85a83d0364943346ca940a02bee522aa4d458979"
        },
        "date": 1792074674774,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11615000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9354358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11614946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2495839,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 845964,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 979393,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176583,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 992790,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31566,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "825bdb1c1ec40e6484cc478ac6ffec96f39bf77d",
          "message": "docs: define support and operational contracts (#1725)",
          "timestamp": "2026-10-15T12:00:05-03:00",
          "tree_id": "248c55f73c8c762c1f9d1546fd9bd550629c82d7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/825bdb1c1ec40e6484cc478ac6ffec96f39bf77d"
        },
        "date": 1792077674846,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11615000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9354358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11614946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2495839,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 845964,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 979393,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176583,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 992790,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31566,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "5fa1c5a838af93d8aa4951e0403311d4f8fa2f65",
          "message": "fix(storage): require explicit session locking and fail closed in Signal adapters (#1729)",
          "timestamp": "2026-10-15T15:59:59-03:00",
          "tree_id": "bfce1e97074f1e07b4ffd58e7b6aab4cd65b43f8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5fa1c5a838af93d8aa4951e0403311d4f8fa2f65"
        },
        "date": 1792091882062,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11615064,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9354422,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11614978,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2495948,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 845964,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 979393,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176914,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 992745,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31564,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "8591d2d55dc4489ac9b2623386532d4d53a18e9d",
          "message": "feat(media)!: configure byte budgets on existing download methods (#1730)",
          "timestamp": "2026-10-15T16:37:18-03:00",
          "tree_id": "9320c80d9f682868ccc74c8f568258b6e4eec888",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8591d2d55dc4489ac9b2623386532d4d53a18e9d"
        },
        "date": 1792093957617,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11625048,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9362998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11624022,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2501982,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 980015,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176653,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 995772,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31656,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "aa368a8978cba96e2c3ec6527bd6876e726852b8",
          "message": "fix(voip): pace outbound video instead of sending each access unit in one burst (#1726)",
          "timestamp": "2026-10-15T17:01:14-03:00",
          "tree_id": "875aac9962b8ffd95611fe9d289ed443176b7824",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/aa368a8978cba96e2c3ec6527bd6876e726852b8"
        },
        "date": 1792095930221,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11625048,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9362998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11624022,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2501982,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 980015,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176984,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 995772,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31656,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c7c2e0f350358fd5ef36df36f14242544565a060",
          "message": "feat(voip): answer the peer's NACKs for our video in 1:1 calls (#1727)",
          "timestamp": "2026-10-15T17:01:31-03:00",
          "tree_id": "9f23fc7a22b2d3191ecb48c2d6922a8a3626c6eb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c7c2e0f350358fd5ef36df36f14242544565a060"
        },
        "date": 1792095942082,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11625048,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9362998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11624022,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2501982,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 980015,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176984,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 995772,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31656,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "5e72baf000aa4fb65ed89d1c955a54029a7f29a6",
          "message": "fix: preserve live LID-PN mappings during startup warm-up (#1731)",
          "timestamp": "2026-10-15T18:06:05-03:00",
          "tree_id": "5ae6c0ecfc230b73f682f7aeb794689108c34151",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5e72baf000aa4fb65ed89d1c955a54029a7f29a6"
        },
        "date": 1792100492517,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11639128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9376566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11640406,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2514346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981186,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176653,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999681,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31769,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "19271eea6b0fccd7f12694849111b3509b0ee6d4",
          "message": "fix(voip): ask for a keyframe when the peer's video loses a packet (#1728)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-15T18:05:44-03:00",
          "tree_id": "4293f99373f18c04f86a5198ebdb32c3ccc33d79",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/19271eea6b0fccd7f12694849111b3509b0ee6d4"
        },
        "date": 1792100507434,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11625048,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9362998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11624022,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2501982,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 980015,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176984,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 995772,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31656,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "7ccd406065fccd8ab3d7b5920ed8a12d554c2e7e",
          "message": "feat(mex)!: make generated contracts evolvable (#1733)",
          "timestamp": "2026-10-15T19:56:01-03:00",
          "tree_id": "3dcf2d2537e827f44c51d13fade968cc12c2022d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7ccd406065fccd8ab3d7b5920ed8a12d554c2e7e"
        },
        "date": 1792106829396,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11639128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9376566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11640406,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2514346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981186,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176984,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999915,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31779,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoassp@gmail.com",
            "name": "Joao Paulo Assis",
            "username": "assisjp"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1f7c52bfa6ab6cbf946ecb239709b6b990cc4607",
          "message": "feat(voip): expose generation-scoped call phase (#1732)",
          "timestamp": "2026-10-15T19:56:21-03:00",
          "tree_id": "9e2357d12bf14ac38b39d8af37ce03c8bb3b403e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1f7c52bfa6ab6cbf946ecb239709b6b990cc4607"
        },
        "date": 1792106865379,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11639128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9376566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11640406,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2514346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981186,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176653,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999915,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31779,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "98fc24a7edd66ecc04ed716c4e04185f707bee5a",
          "message": "feat(runtime)!: allow local WASM runtimes and cancellation (#1734)",
          "timestamp": "2026-10-15T20:23:22-03:00",
          "tree_id": "6c65e06128e25e0ef40c6f8287bc3106d087bcd3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/98fc24a7edd66ecc04ed716c4e04185f707bee5a"
        },
        "date": 1792107751473,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11639128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9376566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11640406,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2514346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981186,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176653,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999915,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31779,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "0f1bc086cedff57ce5fdb9566fd49454b72e01cc",
          "message": "fix: keep startup LID-PN provenance out of the fixed client footprint (#1735)",
          "timestamp": "2026-10-15T20:23:43-03:00",
          "tree_id": "41906f13c6e295daa0c572605962a9ceed010511",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0f1bc086cedff57ce5fdb9566fd49454b72e01cc"
        },
        "date": 1792109882098,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11638488,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9375798,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11636310,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2513515,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981247,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176653,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999983,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31787,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "e95047875a2f7006d611cbe38615066936cb55ca",
          "message": "fix(storage)!: preserve retry metadata and classify typed errors (#1738)",
          "timestamp": "2026-10-15T23:03:07-03:00",
          "tree_id": "c7af08001a940acc16d328599b36548043089031",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e95047875a2f7006d611cbe38615066936cb55ca"
        },
        "date": 1792117078317,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11655128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9393078,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11656734,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2517441,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 850023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592377,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981754,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2177380,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545624,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999997,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31787,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "8b5ee1850f0ae61887aec0991338a672f70a3a50",
          "message": "test(runtime): execute downstream cancellation lifecycle contracts (#1737)",
          "timestamp": "2026-10-15T23:02:46-03:00",
          "tree_id": "49d643aa9a9a171c4111b75d03eb7f4ebac09f2e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8b5ee1850f0ae61887aec0991338a672f70a3a50"
        },
        "date": 1792117119135,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11638488,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9375798,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11636310,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2513515,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 847605,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 582620,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981247,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2176984,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545543,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999983,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31787,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "633f01480fe3a95510172c6312d01484563dcf3c",
          "message": "fix(types)!: make generated open wire enums evolvable (#1736)",
          "timestamp": "2026-10-15T23:03:24-03:00",
          "tree_id": "e92c15a1348b8a892894004876b050d079103579",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/633f01480fe3a95510172c6312d01484563dcf3c"
        },
        "date": 1792117134792,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11655128,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9393078,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11656734,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2517441,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 850023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592377,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981754,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2177711,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545624,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999997,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31787,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "cfa106e2575ce9177d24edb6d488cce9fd629152",
          "message": "refactor(runtime): remove unused retry sender and share blob lookup (#1739)",
          "timestamp": "2026-10-16T01:19:16-03:00",
          "tree_id": "ebd857aed907a3ba0bebbdf07c3e864bf620ac0b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cfa106e2575ce9177d24edb6d488cce9fd629152"
        },
        "date": 1792125080769,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11655160,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9393142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11656662,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2517456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 850023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592377,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981754,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2177711,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545624,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999892,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31785,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "d38324d18aacc352cdca47cc7f757620ef461137",
          "message": "fix(bench): fence activity cleanup before cache maintenance (#1740)",
          "timestamp": "2026-10-16T01:19:44-03:00",
          "tree_id": "149a2274da7b445cb61361e0c01878e723020432",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d38324d18aacc352cdca47cc7f757620ef461137"
        },
        "date": 1792125082535,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11655160,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9393142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11656662,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2517456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 850023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592377,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981754,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2177380,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545624,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999892,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31785,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "05e4b9d9693dc6d12f9d4d1eb7e5d542db8c59d4",
          "message": "test(bench): cover delivery receipt fence edge cases (#1741)",
          "timestamp": "2026-10-16T10:23:46-03:00",
          "tree_id": "bb1a1f8473fa30395f84f6f17badcfa1d952e716",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/05e4b9d9693dc6d12f9d4d1eb7e5d542db8c59d4"
        },
        "date": 1792158064117,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11655160,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9393142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11656662,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2517456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 850023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1808861,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592377,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981754,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2177711,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545624,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999892,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31785,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "135e1aebf9d62c06da17ae4516a90554a472c60e",
          "message": "chore(deps): bump sqlite-wasm-rs from 0.5.5 to 0.6.1 in the cargo-major group (#1718)\n\nCo-authored-by: João Lucas <jlucaso@hotmail.com>\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-16T12:43:39-03:00",
          "tree_id": "a678979b14f7c967ad06eca4113e4687b0ca7bc8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/135e1aebf9d62c06da17ae4516a90554a472c60e"
        },
        "date": 1792166519312,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11655160,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9393142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11656662,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2517456,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 850023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195781,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809192,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592377,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 981754,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2177380,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545624,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 999892,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 31785,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "f7d9021505dcc64603ae38335a0dec5098848540",
          "message": "fix(app-state): bound blob buffers and async task memory (#1742)",
          "timestamp": "2026-10-17T14:06:45-03:00",
          "tree_id": "2f27fd96a016a228486bb221ea8b0dedade0a20a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f7d9021505dcc64603ae38335a0dec5098848540"
        },
        "date": 1792257805066,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11625304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9357814,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11621650,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2512014,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817815,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82563,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195714,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 983960,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180111,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545624,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18375,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009046,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32304,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "7c4021d4fdf4756b0243cf237977e265fd5075a9",
          "message": "perf(binary): simplify token byte verification (#1743)\n\nCo-authored-by: autofix-ci[bot] <114827586+autofix-ci[bot]@users.noreply.github.com>",
          "timestamp": "2026-10-17T17:40:11-03:00",
          "tree_id": "ef473391c02f57201daee1dc1feadfe1a047ee36",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7c4021d4fdf4756b0243cf237977e265fd5075a9"
        },
        "date": 1792270609982,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11625496,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9357942,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11621634,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2512014,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817815,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195714,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 983960,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180111,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545520,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18372,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1008936,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32300,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "86bdcafdc88831dc0e8dbd5190c7fdbc40887e33",
          "message": "fix(app-state): restore buffered HTTP download fallback (#1744)",
          "timestamp": "2026-10-17T18:18:05-03:00",
          "tree_id": "0bb699dce85bc2f1da99ffe58830747853f36f70",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/86bdcafdc88831dc0e8dbd5190c7fdbc40887e33"
        },
        "date": 1792273411397,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11625112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9357686,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11621646,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2511709,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817826,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195714,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 983975,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180111,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545520,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18372,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1008973,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32298,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "deb93d46cba6361fe140fc764767f87d07280bd9",
          "message": "perf(signal): inline protocol address fragment writes (#1745)",
          "timestamp": "2026-10-17T19:01:57-03:00",
          "tree_id": "2e34112c26579663c74620c37c7cb5219cfc95be",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/deb93d46cba6361fe140fc764767f87d07280bd9"
        },
        "date": 1792275328447,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11628536,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9360630,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11625750,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2512824,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 818887,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 983975,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180111,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545703,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1008973,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32298,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "2eda7fb52abcd3319a88a2aa17f562eb6e26e0fe",
          "message": "perf(mlow): read pitch energy windows without copying (#1746)",
          "timestamp": "2026-10-17T19:02:29-03:00",
          "tree_id": "637536cd23660e9329f2d82718bdea175bc8608b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2eda7fb52abcd3319a88a2aa17f562eb6e26e0fe"
        },
        "date": 1792275340330,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11628536,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9360630,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11625750,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2517901,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 818887,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 983975,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2175034,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545703,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1008973,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32298,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "194477f0723dfdb339b5e36e2e1eebfcb9d2f63d",
          "message": "perf: share participant hash tail across iterator inputs (#1747)",
          "timestamp": "2026-10-17T19:02:49-03:00",
          "tree_id": "4653d082fafecfcc18002c3493a21379e3ab879a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/194477f0723dfdb339b5e36e2e1eebfcb9d2f63d"
        },
        "date": 1792275436659,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11627256,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9359222,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11625750,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2512824,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 983975,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180442,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1008973,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32298,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "b08cd322ba2636a099e9705287cddd228d3d6e13",
          "message": "bench: fence passive startup before connected activity (#1748)",
          "timestamp": "2026-10-17T19:03:12-03:00",
          "tree_id": "e2f2b3214ba7ea92c07aa97e135cd60ef90db505",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b08cd322ba2636a099e9705287cddd228d3d6e13"
        },
        "date": 1792275662971,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11627256,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9359222,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11625750,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2512824,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 983975,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180111,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1008973,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32298,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "17cc8a34da05549a2268844cf88483c02ae8c131",
          "message": "feat(retry): opt-in DecryptFailureAdmission hook for outbound retry receipts (#1749)",
          "timestamp": "2026-10-17T21:22:03-03:00",
          "tree_id": "44629c00e02eb0c1b8b8c40708f5e470aeb96095",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/17cc8a34da05549a2268844cf88483c02ae8c131"
        },
        "date": 1792284019220,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11628952,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9360822,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11625798,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2519309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 984205,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2175034,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009909,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32324,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "35a69a14149fa2cd2a0cc719f875f441d1c6e1dc",
          "message": "refactor: freeze retry policies during client assembly (#1750)",
          "timestamp": "2026-10-18T00:35:29-03:00",
          "tree_id": "5c4ddc14f7686d41728470d340258a1a0ac94286",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/35a69a14149fa2cd2a0cc719f875f441d1c6e1dc"
        },
        "date": 1792295389167,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11629848,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9361782,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11625778,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2515309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 984130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180346,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009372,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32297,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoassp@gmail.com",
            "name": "Joao Paulo Assis",
            "username": "assisjp"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c23965eeda87221241ee6f86d90ba16bcb09d5bd",
          "message": "fix(voip): own cancelled call startup cleanup (#1752)",
          "timestamp": "2026-10-18T22:40:26-03:00",
          "tree_id": "51b2fdd365d21d9a9b59b609f7b943b0926f8db5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c23965eeda87221241ee6f86d90ba16bcb09d5bd"
        },
        "date": 1792374928572,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11629848,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9361782,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11625778,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2515309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 984130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180346,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009372,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32297,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoassp@gmail.com",
            "name": "Joao Paulo Assis",
            "username": "assisjp"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d1d929aeb3ea9a10ee61cb87a3176b72e90f3dd2",
          "message": "fix(voip): authenticate direct call answers (#1753)",
          "timestamp": "2026-10-18T22:40:43-03:00",
          "tree_id": "682d7e08d937d256b887abd499f6bfdee1823878",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d1d929aeb3ea9a10ee61cb87a3176b72e90f3dd2"
        },
        "date": 1792374933794,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11629848,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9361782,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11625778,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2515309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 984130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2180015,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009372,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32297,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "cf79d65bf44c616cef70d6844ffc2212cd8cc3bd",
          "message": "fix(ci): skip credentialed E2E for Dependabot contexts (#1760)",
          "timestamp": "2026-10-19T22:27:14-03:00",
          "tree_id": "7478538ef7d2d55bc60284e0815e87989978b02b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cf79d65bf44c616cef70d6844ffc2212cd8cc3bd"
        },
        "date": 1792460340150,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11647120,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9376822,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11645498,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2515309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 984130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2194897,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009372,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32297,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "736f8a09fac2ca70cb1ed6132010268953eb1b30",
          "message": "chore(deps): bump CodSpeedHQ/action from 5.4.2 to 5.5.0 in the actions group (#1757)\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-19T23:09:29-03:00",
          "tree_id": "4884d9646a76ad6983c6938b9a9df46ed6bfc9b6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/736f8a09fac2ca70cb1ed6132010268953eb1b30"
        },
        "date": 1792464435324,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11647120,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9376822,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11645498,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2515309,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 817482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 196465,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 41346,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 23923,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592388,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 984130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2195228,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543507,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009372,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32297,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "05168a7f47bcaa78b6af8994c8c794d943991573",
          "message": "chore(deps): bump the cargo group with 6 updates (#1758)\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-19T23:10:21-03:00",
          "tree_id": "d2ddacda94ad601bd903282d1417a59c268699dc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/05168a7f47bcaa78b6af8994c8c794d943991573"
        },
        "date": 1792465060580,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11672240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9402166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11670190,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2518082,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826973,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 992479,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193806,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009431,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32299,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "c373266fd2c66ae39752171cff5e0dd798a90205",
          "message": "chore(deps): bump the cargo-major group with 3 updates (#1759)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-20T01:00:05-03:00",
          "tree_id": "53e9fafc08a6ccfaa631064681b78c9139ec8de8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c373266fd2c66ae39752171cff5e0dd798a90205"
        },
        "date": 1792472010780,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11673712,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9403830,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11670194,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2519494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 827154,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 992479,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193866,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009431,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32299,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "dd2a5fc95ff1a84a78789fc7659cb7200a0df875",
          "message": "fix(oracle): isolate signaling scenarios and require complete evidence (#1755)",
          "timestamp": "2026-10-20T09:33:40-03:00",
          "tree_id": "abdfa6ee83264df642f4137b4e91cdd3e7731469",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dd2a5fc95ff1a84a78789fc7659cb7200a0df875"
        },
        "date": 1792500329224,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11673712,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9403830,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11670194,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2519494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 827154,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 992479,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193866,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1009431,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32299,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "1da154a9fd7198418e1ed3bb2e648de780cf8818",
          "message": "fix: bound incoming chat and transport ACK queues (#1756)",
          "timestamp": "2026-10-20T09:34:17-03:00",
          "tree_id": "e6ddbe72fbfa62dd851f382dd545ebcca6dfc543",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1da154a9fd7198418e1ed3bb2e648de780cf8818"
        },
        "date": 1792500543958,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11695248,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9423542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11691110,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2535707,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 827154,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 995915,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2194117,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1016719,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32521,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bf6231581bc442a1bc7cd58f511598d8ed9d60ae",
          "message": "fix(voip): report bytes received and receive rate in the 1:1 report extension (#1761)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-20T11:41:30-03:00",
          "tree_id": "f8d1e35a4ca8afdbfaf25be98209439c06c3076d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bf6231581bc442a1bc7cd58f511598d8ed9d60ae"
        },
        "date": 1792508551759,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11695248,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9423542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11691110,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2540784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 827154,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 995915,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2189040,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1016719,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32521,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "31011319+senamakel@users.noreply.github.com",
            "name": "Steven Enamakel",
            "username": "senamakel"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8e17194669c755b876cf29fdc30a6dfa474b5914",
          "message": "fix(waproto): keep the crate fresh after the first build (#1762)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-20T11:54:38-03:00",
          "tree_id": "03190d5410aa09a46674a6a10ce2a7b5282c6174",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8e17194669c755b876cf29fdc30a6dfa474b5914"
        },
        "date": 1792508973723,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11695248,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9423542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11691110,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2535707,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 827154,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 995915,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2194117,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1016719,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32521,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7f955aeac4b07f20a18c43eefb80f8c382f776dc",
          "message": "feat(voip): count why received video packets never become pictures (#1763)",
          "timestamp": "2026-10-20T14:14:31-03:00",
          "tree_id": "f94811d421bd5d47aa4d5397731bbed1b235079e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7f955aeac4b07f20a18c43eefb80f8c382f776dc"
        },
        "date": 1792517329544,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11695248,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9423542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11691110,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2535707,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 827154,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 995915,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193786,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1016719,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32521,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3fc86ad22b0066b5ec6d8eefd9034401a31f2433",
          "message": "fix(sqlite-storage): put the sqlite-wasm-rs 0.6 edge behind a js feature (#1765)",
          "timestamp": "2026-10-20T15:56:24-03:00",
          "tree_id": "58033db39a6be625176e0505563943d1b1e0dc85",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3fc86ad22b0066b5ec6d8eefd9034401a31f2433"
        },
        "date": 1792523485889,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11695248,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9423542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11691110,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2535707,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 827154,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43856,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806476,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58804,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 995915,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193786,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1016719,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32521,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoassp@gmail.com",
            "name": "Joao Paulo Assis",
            "username": "assisjp"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b771320aa5be6288cb8862750f2f6b6326a7c48a",
          "message": "fix(deps): update TLS verification and trusted roots (#1768)",
          "timestamp": "2026-10-20T19:08:28-03:00",
          "tree_id": "09aa84d2a851c23b82533403832ec7577f9c7d99",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b771320aa5be6288cb8862750f2f6b6326a7c48a"
        },
        "date": 1792534955319,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11691152,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9419510,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11687174,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2534060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826664,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 994937,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193370,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1016719,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32521,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "472cfe272b1625b2acca4f39606c9a5b7e5b1a47",
          "message": "fix(sdk): expose SQLite browser hooks through js opt-in (#1766)",
          "timestamp": "2026-10-20T19:09:20-03:00",
          "tree_id": "0ef7a5cbd6aecbc61852d015ec04147ddd731dcc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/472cfe272b1625b2acca4f39606c9a5b7e5b1a47"
        },
        "date": 1792535190561,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11691152,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9419510,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11687174,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2534060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826664,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 994937,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193370,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543691,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18346,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1016719,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32521,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoassp@gmail.com",
            "name": "Joao Paulo Assis",
            "username": "assisjp"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7c03127280544a12a2456cdb516a1a4311db610d",
          "message": "feat: recover history sync notifications durably (#1751)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-20T20:40:04-03:00",
          "tree_id": "450815b95c5f7f6a7f7f045c9d7e37f113fb88fb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7c03127280544a12a2456cdb516a1a4311db610d"
        },
        "date": 1792540500115,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193258,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544108,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18348,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "eab45737fb79ef583e1ca39ba458d9ec2b900c3b",
          "message": "fix(community): generate Create IQs from whatspec (#1770)",
          "timestamp": "2026-10-21T09:20:23-03:00",
          "tree_id": "29105a9926e62b73c4c64bc54f445ccdb6bd6fa8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/eab45737fb79ef583e1ca39ba458d9ec2b900c3b"
        },
        "date": 1792586091941,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193258,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544439,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "94b142009547dbd9f1de5b96088d6327bcd2d3e1",
          "message": "fix(voip): send each video access unit as one burst, split evenly (#1771)",
          "timestamp": "2026-10-21T14:45:30-03:00",
          "tree_id": "883fc82a962840c6d7ffe8044eb03975bb130139",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/94b142009547dbd9f1de5b96088d6327bcd2d3e1"
        },
        "date": 1792605504343,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193258,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544439,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "43e3e5e726120aa307fdba8a5086412e7460c0c0",
          "message": "fix(voip): allow a group-bound call to a two-person group (#1772)",
          "timestamp": "2026-10-21T14:45:51-03:00",
          "tree_id": "06cecdfbb15fbe8311cc93c22d8c41af9572f004",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/43e3e5e726120aa307fdba8a5086412e7460c0c0"
        },
        "date": 1792605731315,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193258,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "81e89e5d64c85cbbb1906308360c67514672ea04",
          "message": "docs(voip): correct protected video packet budget (#1775)",
          "timestamp": "2026-10-21T15:50:32-03:00",
          "tree_id": "acb58f94cbb18c8730c2374b09e6af010b956204",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/81e89e5d64c85cbbb1906308360c67514672ea04"
        },
        "date": 1792609328857,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193258,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "35ef9b467205912468a4033532c5834b64a84710",
          "message": "fix(voip): keep a picture sent on its predecessor's RTP timestamp (#1774)",
          "timestamp": "2026-10-21T15:10:31-03:00",
          "tree_id": "89fb844f2920e055b3ede29340185810188f5239",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/35ef9b467205912468a4033532c5834b64a84710"
        },
        "date": 1792609350220,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193258,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "69a6f949e9b24e0bba13e7f64014609ffe087cf3",
          "message": "fix(http): bound default connects to allow address fallback (#1776)",
          "timestamp": "2026-10-21T15:51:25-03:00",
          "tree_id": "7a284f7a6191eb99d5567c1337016838c0b1fa96",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/69a6f949e9b24e0bba13e7f64014609ffe087cf3"
        },
        "date": 1792609988081,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193578,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoassp@gmail.com",
            "name": "Joao Paulo Assis",
            "username": "assisjp"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bc74e9e821b4bc62acdcf0988681400e07a2b105",
          "message": "feat(voip): expose accepted direct media readiness (#1754)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-21T21:19:46-03:00",
          "tree_id": "5ba145de363b7499fc26deb90460545572f2ad25",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bc74e9e821b4bc62acdcf0988681400e07a2b105"
        },
        "date": 1792630337519,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193578,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "joaoassp@gmail.com",
            "name": "Joao Paulo Assis",
            "username": "assisjp"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9dce09b5109165eeea2811261db2cd0e64cb421b",
          "message": "feat(examples): demonstrate bounded manual presence maintenance (#1767)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-10-21T21:20:09-03:00",
          "tree_id": "e6e89984daeeb27a26d18c724ad6126a57d5e9a8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9dce09b5109165eeea2811261db2cd0e64cb421b"
        },
        "date": 1792630395064,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193578,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "97ad182a6324e8d6cd2c103386f43c8f8fa56ccd",
          "message": "test(http): skip dual-stack fallback without IPv6 loopback (#1779)",
          "timestamp": "2026-10-21T21:20:24-03:00",
          "tree_id": "530a270986821d66abbe44e445c058aea9478c7a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/97ad182a6324e8d6cd2c103386f43c8f8fa56ccd"
        },
        "date": 1792630567968,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547133,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193578,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "f5f241f2f865b62e5ed5e1acb89bfdeedd369b9e",
          "message": "test(send): control startup publication in DM memo coverage (#1780)",
          "timestamp": "2026-10-21T22:41:56-03:00",
          "tree_id": "a0246f29a10173f219bb8d7abe481a88124dcc8b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f5f241f2f865b62e5ed5e1acb89bfdeedd369b9e"
        },
        "date": 1792634100756,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547141,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193578,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "3870ce5c3d5d856e7cb3cc7f3a0cbbf6fcf24f8e",
          "message": "fix(newsletter): publish text polls without E2E creation (#1778)",
          "timestamp": "2026-10-21T22:42:24-03:00",
          "tree_id": "8c6b077746b8f525097bd2b4246b4cbec2dc01f8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3870ce5c3d5d856e7cb3cc7f3a0cbbf6fcf24f8e"
        },
        "date": 1792634108341,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "de5896b6293c8d2e6bfb177b480d7bc09b652c7b",
          "message": "fix(appstate): reject lock mutations without a locked value (#1781)",
          "timestamp": "2026-10-21T22:41:29-03:00",
          "tree_id": "2201a87dd4fb26179becc4461f7b7ae545bc5e9c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/de5896b6293c8d2e6bfb177b480d7bc09b652c7b"
        },
        "date": 1792634137108,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547141,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82677,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193578,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1022539,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32738,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3d3c6270cd7f35883e1e12280ae3d1055e076007",
          "message": "fix(voip): drop video queued before the relay allocates (#1782)",
          "timestamp": "2026-10-22T10:27:41-03:00",
          "tree_id": "48f086ce5c7a903bcca568798e8015f01fa63d44",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3d3c6270cd7f35883e1e12280ae3d1055e076007"
        },
        "date": 1792676345627,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1bfa8d6a66871d0abaf1c5efc77cf4f1694da6f3",
          "message": "feat(voip): send video FEC in 1:1 calls (#1783)",
          "timestamp": "2026-10-22T10:28:04-03:00",
          "tree_id": "ba5c13343dcf407d472b788015cae3a9ed6930cb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1bfa8d6a66871d0abaf1c5efc77cf4f1694da6f3"
        },
        "date": 1792676570853,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "4beb4ff242d65c28afc5b611bef2cd0f7c238174",
          "message": "fix(voip): retire discarded FEC and count successful writes (#1785)",
          "timestamp": "2026-10-22T13:08:55-03:00",
          "tree_id": "fafe055f6d00a6420b4c3caa488f42dba7901ede",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4beb4ff242d65c28afc5b611bef2cd0f7c238174"
        },
        "date": 1792686149506,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "19c01e237bb8c9bf33d987dca5034184f498e75b",
          "message": "test(voip): allocate the relay before the FEC engine test sends video (#1786)",
          "timestamp": "2026-10-22T13:08:00-03:00",
          "tree_id": "71c7e5350e17adaf5c6aadda97e5d69a3cdc4c31",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/19c01e237bb8c9bf33d987dca5034184f498e75b"
        },
        "date": 1792686157790,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "d03f6a8f60d0741e43e25e6c347e7872e9b5180a",
          "message": "fix(voip): retire video input backlog at relay allocation (#1784)",
          "timestamp": "2026-10-22T13:08:37-03:00",
          "tree_id": "78bf1a643cf1055630b4ab98e1f3d1170c1301dc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d03f6a8f60d0741e43e25e6c347e7872e9b5180a"
        },
        "date": 1792686160652,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "ac9d0df3d6269c6b5b60881a7154414d7c348991",
          "message": "fix(voip): remove batches emptied by FEC retirement (#1787)",
          "timestamp": "2026-10-22T15:33:32-03:00",
          "tree_id": "9a75afa99c66951227c9f5013e684647bd4eae26",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ac9d0df3d6269c6b5b60881a7154414d7c348991"
        },
        "date": 1792694767839,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9a9677b6217db5f3570f807604294e5301f1b010",
          "message": "feat(voip): interleave video FEC groups against burst loss (#1789)",
          "timestamp": "2026-10-23T15:09:16-03:00",
          "tree_id": "1b4f94ba36810b709ccd5bf5d97b5dfdfdfd6e5b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9a9677b6217db5f3570f807604294e5301f1b010"
        },
        "date": 1792779807562,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "carmine@paolino.me",
            "name": "Carmine Paolino",
            "username": "crmne"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "547e6b96570423dc689e3688bd77aeabb51e8108",
          "message": "feat(voip): time outgoing video from capture (#1790)",
          "timestamp": "2026-10-23T15:09:30-03:00",
          "tree_id": "801f16741235969111975ffc4b43815a944654e4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/547e6b96570423dc689e3688bd77aeabb51e8108"
        },
        "date": 1792780012247,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11712304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9438646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11712262,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2547632,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 997318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193574,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1023484,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32752,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "44215764+chumarrento@users.noreply.github.com",
            "name": "Lucas",
            "username": "chumarrento"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bdd973b7f46e336976ecdfe2651c12a804a0f63b",
          "message": "fix(app-state): apply a late or unsolicited snapshot recovery from the primary (#1788)",
          "timestamp": "2026-10-23T15:09:53-03:00",
          "tree_id": "f1f62e7deb15f10fa86900889d6cf07f519b2965",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bdd973b7f46e336976ecdfe2651c12a804a0f63b"
        },
        "date": 1792780051771,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11721872,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9446966,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720746,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2555560,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829387,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998016,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193648,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1026684,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32866,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "d73c3cc1478bd790dda1be272d890302007007db",
          "message": "fix(voip): preserve FEC burst protection and paced retransmission bytes (#1793)",
          "timestamp": "2026-10-23T16:40:22-03:00",
          "tree_id": "5e76437f5bd9e21233fee4bf3c9cf25961546671",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d73c3cc1478bd790dda1be272d890302007007db"
        },
        "date": 1792785735512,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11720432,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9445366,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720714,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2554148,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829206,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592527,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998016,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193588,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544510,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18352,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1026684,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32866,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "9cae64c05636316d2d62040fc768596707e3a51b",
          "message": "fix(ci): qualify desktop web profiles on current main (#1792) (#1795)\n\nCo-authored-by: eternalwaitt <229533760+eternalwaitt@users.noreply.github.com>",
          "timestamp": "2026-10-23T18:39:39-03:00",
          "tree_id": "9e21c8135f53a809d15ed1163de30c67f4593076",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9cae64c05636316d2d62040fc768596707e3a51b"
        },
        "date": 1792792314616,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11720560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9445494,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720714,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2554148,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829237,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998031,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193588,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1026688,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32866,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "tradecopierpay@gmail.com",
            "name": "thecreepervibe",
            "username": "thecreepervibe"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9b9fdb7e5c4211cdc7c837abb6a9ae9c597e35c5",
          "message": "fix(voip): send the caller's relaylatency and transport, with a call id (#1098) (#1794)",
          "timestamp": "2026-10-23T18:40:42-03:00",
          "tree_id": "5c745beb9a6367ea40a672df65355de3c7d56f77",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9b9fdb7e5c4211cdc7c837abb6a9ae9c597e35c5"
        },
        "date": 1792792328401,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11720560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9445494,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720714,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2554148,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829237,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998031,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193588,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1026688,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32866,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "cp@imtaqin.id",
            "name": "taqin",
            "username": "fdciabdul"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d136bbc5f5209d543e3b8927d9232ad9b3dfdb9d",
          "message": "Keep the biz node inside message wrappers, and set the About through the text status mutation (#1798)",
          "timestamp": "2026-10-25T15:55:09-03:00",
          "tree_id": "37861aa805d2baf683d31b7a08d6a705935abbf6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d136bbc5f5209d543e3b8927d9232ad9b3dfdb9d"
        },
        "date": 1792955245242,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11720880,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9445814,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720714,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2559578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829237,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998031,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2188511,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1027480,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32872,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "victor.penter@gmail.com",
            "name": "eternalwaitt",
            "username": "eternalwaitt"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fed0cf32488321e34fd880c21b147cce0875480b",
          "message": "Expose history request IDs before dispatch (#1796)",
          "timestamp": "2026-10-26T07:36:30-03:00",
          "tree_id": "067cce599e784f23942d5a4a23246d43b5876a44",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fed0cf32488321e34fd880c21b147cce0875480b"
        },
        "date": 1793011583129,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11720880,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9445814,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720714,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2554501,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829237,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998031,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193588,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1028008,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32874,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "9e56eb2b52b5466bcaa00a729de3a5426479c0a9",
          "message": "fix(plugins): inherit the established workspace license (#1801)",
          "timestamp": "2026-10-26T11:27:27-03:00",
          "tree_id": "1ef1ea9b39f14656e392ef57c50d4ca14fb309c6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9e56eb2b52b5466bcaa00a729de3a5426479c0a9"
        },
        "date": 1793025887346,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11720880,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9445814,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720714,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2554501,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829237,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998031,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193588,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1028008,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32874,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "6c0fb0b379f57859b081d4d657a9f2edc5d617d8",
          "message": "fix(wam): refresh buffer identity from the client snapshot (#1802)",
          "timestamp": "2026-10-26T11:27:50-03:00",
          "tree_id": "4be21fc380fa75d9be150045dab297eed5daad59",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6c0fb0b379f57859b081d4d657a9f2edc5d617d8"
        },
        "date": 1793026642227,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11720880,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9445814,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720714,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2554501,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829237,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998031,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193588,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1028008,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32874,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 615,
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
          "id": "bd910d1516b6fcf2f1bd21cb016d2094b15a0f6e",
          "message": "Fix the host tooling audit with an injected capture HTTP client (#1803)",
          "timestamp": "2026-10-26T14:14:59-03:00",
          "tree_id": "99654c1b9bd1e47018e46063c78b75adf6c727eb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bd910d1516b6fcf2f1bd21cb016d2094b15a0f6e"
        },
        "date": 1793035647443,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11722608,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9447606,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11720798,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2556288,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829059,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 25931,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998031,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2193756,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1028008,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32874,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 572,
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
          "id": "51673ef9cea292c22b658ae53b0f344fd4bcf58a",
          "message": "fix(client): log the full source chain when connecting fails (#1804)",
          "timestamp": "2026-10-26T16:43:01-03:00",
          "tree_id": "0d73703a7d65ab36e936318dc0d1ba4e2b92c99e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/51673ef9cea292c22b658ae53b0f344fd4bcf58a"
        },
        "date": 1793045255075,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11732752,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9456566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11733866,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2560326,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829934,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998176,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2196162,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1028295,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32883,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 572,
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
          "id": "6d427b7ec3af6b5035073e55293b9dd3ff6a45a7",
          "message": "chore(deps): bump CodSpeedHQ/action from 5.5.0 to 5.6.0 in the actions group (#1806)\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-26T21:03:50-03:00",
          "tree_id": "40a687bed2f42efe2ba9fe021f397da236ef0b65",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6d427b7ec3af6b5035073e55293b9dd3ff6a45a7"
        },
        "date": 1793060165140,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11732752,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9456566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11733866,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2560326,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 829934,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82388,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43787,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 592588,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 58775,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12854,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 998176,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2196162,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1028295,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32883,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 572,
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
          "id": "eb1295439f0d0eda37a96307b091c4f9016a30dd",
          "message": "Close eight pre-1.0 lifecycle, extension, and compatibility audit items (#1805)",
          "timestamp": "2026-10-26T21:05:28-03:00",
          "tree_id": "343c82135df68724175c2b40690e69eef6b1b7b7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/eb1295439f0d0eda37a96307b091c4f9016a30dd"
        },
        "date": 1793060676706,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11829872,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9545142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11827997,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2566662,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 851313,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86436,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197519,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 589830,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043352,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2209594,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1025006,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32704,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "ad8500b165460cb0f3f80c18ca5132acb23bd83f",
          "message": "chore(deps): bump the cargo group with 10 updates (#1807)\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-26T21:05:12-03:00",
          "tree_id": "2f90b3e647f41ad11bbb606e941343d503924fcb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ad8500b165460cb0f3f80c18ca5132acb23bd83f"
        },
        "date": 1793060722162,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11829872,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9545142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11827997,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2566545,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 851313,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86436,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197519,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 589830,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043444,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2209594,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1024877,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32700,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "950bbe091ed3c05b9176ca932951f9cabfdf449a",
          "message": "perf(sqlite): cache hot writes and remove redundant token limits (#1808)",
          "timestamp": "2026-10-27T11:52:06-03:00",
          "tree_id": "f393804509cf8cc0209e287c7d18c5ff989890ea",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/950bbe091ed3c05b9176ca932951f9cabfdf449a"
        },
        "date": 1793113299462,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11835920,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9549750,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11836637,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2566662,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 851313,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86436,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197519,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043352,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2209594,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1025006,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32704,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "0ec2942c1db85af97364743746f9b88c72d299b9",
          "message": "Stabilize concurrent prefetch peak measurements at the stream boundary (#1809)",
          "timestamp": "2026-10-27T13:52:23-03:00",
          "tree_id": "f96a039417a9cba511b110afa0126a624bdd2b55",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0ec2942c1db85af97364743746f9b88c72d299b9"
        },
        "date": 1793121423545,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11835920,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9549750,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11836637,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2566662,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 851313,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86436,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197519,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806145,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043352,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2209594,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544600,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1025006,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32704,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "9649af098260cf53b6b1d7c2434b1a21cd6378b3",
          "message": "fix(signal): import bounded skipped-key backlogs with durable retention (#1810)",
          "timestamp": "2026-10-27T17:30:09-03:00",
          "tree_id": "a0dd3b484578cc8a35f9518ecef91a7a31b57bd9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9649af098260cf53b6b1d7c2434b1a21cd6378b3"
        },
        "date": 1793133751517,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11836656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9550390,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11836733,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2572249,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 851428,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86436,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043304,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2204240,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544608,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1025014,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32704,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "cc0d22f7e96232b6a9ab5a745338d32cb2d31b78",
          "message": "Harden shutdown reports, chat clearing, and snapshot publication (#1812)",
          "timestamp": "2026-10-27T19:40:00-03:00",
          "tree_id": "67c403cbe1b2b79837b99424459f365b1261ddd6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cc0d22f7e96232b6a9ab5a745338d32cb2d31b78"
        },
        "date": 1793141544500,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11849456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9561590,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11849221,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2571853,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 851428,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86436,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1048908,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210090,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544608,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1025235,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32720,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "64707077+alitura1@users.noreply.github.com",
            "name": "Ali Tura Çetin",
            "username": "alitura1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1fcb58a272334604261eb23f5e2a3f1b54059a33",
          "message": "fix(voip): parse a partial group-offer ack and keep its refusal (#1811)",
          "timestamp": "2026-10-29T08:52:00-03:00",
          "tree_id": "1a0edc5954a36c8d17b9378a046990ad78422d81",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1fcb58a272334604261eb23f5e2a3f1b54059a33"
        },
        "date": 1793275411948,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11849904,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9562102,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11849233,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2571754,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 852061,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86436,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1048908,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210090,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 545057,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18358,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1025235,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32720,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "2168bec9d0071c0292883461f98d48b3bf991820",
          "message": "Pass actual sent IQ context to generated response contracts (#1818)",
          "timestamp": "2026-10-29T21:34:47-03:00",
          "tree_id": "9d529127c2c362af5f2f113c083e1d6c7592ff82",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2168bec9d0071c0292883461f98d48b3bf991820"
        },
        "date": 1793327083262,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11868080,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9579062,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11869721,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2561396,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049614,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210090,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 586466,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18782,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1029957,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32847,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "8eae18e444e713b228e5666c6b505fa8524ecaba",
          "message": "test(retry): check manual repair against pinned Web session oracle (#1816)",
          "timestamp": "2026-10-29T21:35:31-03:00",
          "tree_id": "3188d5817ea67ecca9a1796fc6d877428ff03fe9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8eae18e444e713b228e5666c6b505fa8524ecaba"
        },
        "date": 1793327112768,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11868080,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9579062,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11869721,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2561396,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049614,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210090,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 586466,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18782,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1029957,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32847,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "3075d3f8e1ee32a0e8bebb2e87a715bae2f09303",
          "message": "Consume contextual group notification contracts and owned normalization (#1817)",
          "timestamp": "2026-10-29T21:35:57-03:00",
          "tree_id": "3301ba15f0093b8d4a2c89a0b728dc58c18e1202",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3075d3f8e1ee32a0e8bebb2e87a715bae2f09303"
        },
        "date": 1793328039079,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11868240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9579126,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11869729,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2561663,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049421,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210090,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 626167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19358,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1030066,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32844,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "300d7d386df8254db5e8fc6d4037609a9aee009d",
          "message": "perf(cache): avoid listener allocations on uncontended insert (#1820)",
          "timestamp": "2026-10-29T21:37:07-03:00",
          "tree_id": "eaf5640874ef19ba3e86ecc5f428764114c1adbc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/300d7d386df8254db5e8fc6d4037609a9aee009d"
        },
        "date": 1793328995580,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11871568,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9582454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11869737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2570316,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049433,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2204736,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 626167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19358,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1030627,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32854,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "892ec77de707cf3274986c59c850459a79a5cdf4",
          "message": "perf(history): avoid temporary strings for shared chat keys (#1821)",
          "timestamp": "2026-10-29T21:37:27-03:00",
          "tree_id": "d960638f761e10288fa4ef582f310c4be671458f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/892ec77de707cf3274986c59c850459a79a5cdf4"
        },
        "date": 1793332446575,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11871248,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9582134,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11869737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2564629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049433,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210090,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 626167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19358,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1030619,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32854,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "58654029+zahidcoder@users.noreply.github.com",
            "name": "Zahid Hussain",
            "username": "zahidcoder"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e0d8f51a0944c0cd6e9117ddc37b0a0decbb5ef1",
          "message": "fix(voip): widen inbound playout ceiling so relay jitter does not trim speech (#1815)",
          "timestamp": "2026-10-29T22:42:06-03:00",
          "tree_id": "60eec9554fb765fea6524698ae3c903e359f3f58",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e0d8f51a0944c0cd6e9117ddc37b0a0decbb5ef1"
        },
        "date": 1793333944218,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11871248,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9582134,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11869737,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2564629,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049433,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210090,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 626167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19358,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1030619,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32854,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "94148929+Yoryoboy@users.noreply.github.com",
            "name": "Jorge Diaz",
            "username": "Yoryoboy"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "af2e018c270f03c8733d303e8c56093f8e720952",
          "message": "fix(receive): fit two pulled offline batches in one chat lane (#1814)",
          "timestamp": "2026-10-29T22:42:36-03:00",
          "tree_id": "e48da45e75179741f0b901a26498732f3e81cb4f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/af2e018c270f03c8733d303e8c56093f8e720952"
        },
        "date": 1793334472087,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11879280,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9589366,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11878189,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2571276,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049916,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210090,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 626167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19358,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1032667,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32929,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "31626c4f197d04232735e2e773caf48f85e81f14",
          "message": "Fix remaining offline resume and Opus review regressions (#1823)",
          "timestamp": "2026-10-30T07:46:52-03:00",
          "tree_id": "22ad9b9c8eb7eab16cc379e5bec67a852fa255b4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/31626c4f197d04232735e2e773caf48f85e81f14"
        },
        "date": 1793358548339,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11881424,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9591414,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11882213,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2578623,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 878546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 43794,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1806852,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1049991,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2204736,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 626167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19358,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1032696,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 32930,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
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
          "id": "0cf4dcf6f4f6886649acc15d7f1a36bc23ae3021",
          "message": "Validate app-state wire protobufs with pinned generated native programs (#1819)",
          "timestamp": "2026-10-30T08:42:51-03:00",
          "tree_id": "67eb6e433b7e2ca688e1843eb116567bbda58bd8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0cf4dcf6f4f6886649acc15d7f1a36bc23ae3021"
        },
        "date": 1793363005863,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11927216,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9623862,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11925201,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2578886,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 884202,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 197147,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 58193,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 27338,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1809438,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 594419,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 59810,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12927,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053628,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2210347,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 627754,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19424,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 1036558,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 33114,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 573,
            "unit": "crates"
          }
        ]
      }
    ]
  }
}