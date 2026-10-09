window.BENCHMARK_DATA = {
  "lastUpdate": 1791504033333,
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
          "id": "697fc0d8f517ec7d1442f3718355faa0d0b28ebe",
          "message": "feat(handshake)!: remove danger-skip-cert-chain-verify cargo feature (#1444)",
          "timestamp": "2026-09-05T14:53:23-03:00",
          "tree_id": "50d3068fc8a2994be24266184076d173dae1f417",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/697fc0d8f517ec7d1442f3718355faa0d0b28ebe"
        },
        "date": 1788631502791,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10967416,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8824182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10968418,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2073404,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 774571,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600139,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41495,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1071896,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049688,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574581,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18836,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 828297,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26579,
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
          "id": "51c2bb0a29a0c0fab79152e269cba05a5ef07d14",
          "message": "perf(store): reduce contention and harden persistence (#1445)",
          "timestamp": "2026-09-05T17:53:54-03:00",
          "tree_id": "ceaf6e3d72a49ddcaf101bab0d2eac4d935f76ed",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/51c2bb0a29a0c0fab79152e269cba05a5ef07d14"
        },
        "date": 1788642728115,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10998648,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8853366,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10997378,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2069855,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 806353,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600139,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41495,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1072865,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049651,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574609,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18840,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834619,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26742,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 470,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2dfdb1d44fe8d56007e3e805c217999d6c1c1321",
          "message": "fix(benchmark): scope Noise bypass to explicit mock mode (#1446)",
          "timestamp": "2026-09-05T18:58:59-03:00",
          "tree_id": "159fc4cc76ac1639ae41ba9f6aa733749aba3f2c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2dfdb1d44fe8d56007e3e805c217999d6c1c1321"
        },
        "date": 1788646334622,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10998648,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8853366,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10997378,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2069855,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 806129,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600139,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41495,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1072865,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049875,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574609,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18840,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834619,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26742,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 470,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3cf0d648997c8d5a4df96dc50d3c429aad32160f",
          "message": "Add post-commit durability barriers (#1448)",
          "timestamp": "2026-09-06T03:02:24-03:00",
          "tree_id": "82c201eba5b5773d89fae35a8df3ba06a799d590",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3cf0d648997c8d5a4df96dc50d3c429aad32160f"
        },
        "date": 1788675396653,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11021112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8879030,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11021098,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2069855,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 806734,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 618855,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41495,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1078517,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050528,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574609,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18840,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834616,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26741,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 470,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f1b04f79f9c38470842e606e8630e6b6ca8f8021",
          "message": "bench: de-noise flaky rows (phash escape, fixed-key hashers, n=1 guidance) (#1452)",
          "timestamp": "2026-09-06T12:09:29-03:00",
          "tree_id": "543fdd5f7b01de85e22fadff72a808a4933504b1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f1b04f79f9c38470842e606e8630e6b6ca8f8021"
        },
        "date": 1788708334125,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11021112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8879030,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11021098,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2074932,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 807125,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 618855,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41495,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1078517,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2045060,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574609,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18840,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834616,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26741,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 470,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "hello@nizarfadlan.dev",
            "name": "Nizar Izzuddin Yatim Fadlan",
            "username": "nizarfadlan"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "31fa9adeb01f118fe2700f9654bb9c463bc47c47",
          "message": "feat(voip): expose received video generation (#1451)\n\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-09-06T12:43:02-03:00",
          "tree_id": "eedebb544900c29afa64c57b773b97d2132929ae",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/31fa9adeb01f118fe2700f9654bb9c463bc47c47"
        },
        "date": 1788710122889,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11021112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8879030,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11021098,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2074932,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 807125,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 618855,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41495,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1078517,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2045060,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574609,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18840,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834616,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26741,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 470,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dc6c9a340ca177c2f131be5c781028dc5d4fbffc",
          "message": "fix(groups): accept bare join success in AcceptGroupInviteV4Iq (#1453)",
          "timestamp": "2026-09-06T14:35:39-03:00",
          "tree_id": "b1eb8547bc7a47ae7d2942effdb03d854b7daf7e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dc6c9a340ca177c2f131be5c781028dc5d4fbffc"
        },
        "date": 1788716852044,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11021112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8879030,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11021098,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2069855,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 807125,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 618855,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41495,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1078517,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050137,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574629,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18840,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834616,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26741,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 470,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "72d34b7cf1fbb50830bae2a217cef9eb8ec79e01",
          "message": "feat(groups): derive join and passive shapes from the IR (#1454)",
          "timestamp": "2026-09-06T15:09:26-03:00",
          "tree_id": "20afb761e751db818308a760f5153f165064ad24",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/72d34b7cf1fbb50830bae2a217cef9eb8ec79e01"
        },
        "date": 1788719034190,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11021560,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8879414,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11021074,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2069775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 807559,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195775,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 618855,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41495,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1078517,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050137,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574611,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18842,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834733,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26740,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 470,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "965f020d52e649e26e9e72ebb15c8f5778f214b3",
          "message": "feat(mlow): complete codec evidence and own oracle tooling (#1447)",
          "timestamp": "2026-09-06T15:26:39-03:00",
          "tree_id": "79d14baaa317ada9f93f074ab32844044af25f68",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/965f020d52e649e26e9e72ebb15c8f5778f214b3"
        },
        "date": 1788720048333,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11014712,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8871414,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11013306,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2071100,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 807616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86158,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41865,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068724,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049850,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574611,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18842,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834733,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26740,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 604,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a6c45ac35b9b4bfa66e09e7dba139d196641cc85",
          "message": "audit(newsletter): lock receive-path leniency against presence gates (#1455)",
          "timestamp": "2026-09-06T16:30:08-03:00",
          "tree_id": "8918d1ddd0e003fb96f3ead19381f5153ba093c0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a6c45ac35b9b4bfa66e09e7dba139d196641cc85"
        },
        "date": 1788723941412,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11014712,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8871414,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11013306,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2066023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 807616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86158,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41865,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068724,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2054927,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574872,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18844,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834733,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26740,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 604,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9478f313a283c8c0c9fbe0281f4f1eb28802312b",
          "message": "fix(groups): tolerate linked-group metadata in join response (#1456)",
          "timestamp": "2026-09-06T16:29:56-03:00",
          "tree_id": "caceb03a6f9b40c404137700f71a520f41fc8414",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9478f313a283c8c0c9fbe0281f4f1eb28802312b"
        },
        "date": 1788723941531,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11014712,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8871414,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11013306,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2071100,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 807225,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86158,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41865,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068724,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050241,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574872,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18844,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834733,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26740,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 604,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "db9c8a065cb4dd03c37417a411bbd46b521b39d0",
          "message": "fix(bench): drive overlapped burst and read jointly (#1459)",
          "timestamp": "2026-09-06T19:49:05-03:00",
          "tree_id": "ab1e15834e9db11ad8e08267a45e856566c07b74",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/db9c8a065cb4dd03c37417a411bbd46b521b39d0"
        },
        "date": 1788735904327,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11014712,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8871414,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11013306,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2071100,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 807225,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86158,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41865,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068724,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050241,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574872,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18844,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834733,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26740,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 604,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7d37caba25468d8d5e3084968488e55360d3ff78",
          "message": "feat(transport): race dual chat dials with WA Web semantics (#1457)",
          "timestamp": "2026-09-06T19:49:38-03:00",
          "tree_id": "7a61be9daf7b383ca44d1a379f5b923e88236352",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7d37caba25468d8d5e3084968488e55360d3ff78"
        },
        "date": 1788736736168,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11015736,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8872438,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11013306,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2066391,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 808640,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86158,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 22742,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068730,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2055094,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 576949,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18921,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 834432,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26726,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 604,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c8eacdccc1bed1323a6115aa5bcbca0df4f8f1bd",
          "message": "perf(core): extract NoiseSocket and standalone handshake runner to wacore, eliminate md5 crate (#1458)",
          "timestamp": "2026-09-06T21:04:15-03:00",
          "tree_id": "67dcd24fbc748a22f2eda871b3b296a8fb1cf27d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c8eacdccc1bed1323a6115aa5bcbca0df4f8f1bd"
        },
        "date": 1788740709735,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11008024,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8864054,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11009662,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2044821,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826013,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068409,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050063,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 585653,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19320,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 825642,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26407,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 603,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "39d6d372fe66e4d9164e2425e65e644c8454da68",
          "message": "fix(bench): pin cache_insert_at_capacity to a fixed-key hash seed (#1460)",
          "timestamp": "2026-09-06T21:25:34-03:00",
          "tree_id": "7db15e87880bb84c32a945afcf4e388ffb370e9d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/39d6d372fe66e4d9164e2425e65e644c8454da68"
        },
        "date": 1788741098600,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11007960,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8863990,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11009662,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2049852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826013,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068409,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044986,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 585653,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19320,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 825354,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26425,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 603,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "162315bd1c50abcfe5407d38256f486003d56e04",
          "message": "fix(client): avoid process IDs in browser durability probes (#1462)",
          "timestamp": "2026-09-07T00:15:23-03:00",
          "tree_id": "9e67a0696f5e07001f2dbca7e5146ca29c51c95f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/162315bd1c50abcfe5407d38256f486003d56e04"
        },
        "date": 1788751762532,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11008408,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8864438,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11009638,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2045214,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826013,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068409,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050063,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 585653,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19320,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 825414,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26426,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 603,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "077cc3fccaba3e9752252dc998937b63ec63bbf5",
          "message": "fix(voip): preserve upright video orientation in RTP metadata (#1463)",
          "timestamp": "2026-09-07T03:49:59-03:00",
          "tree_id": "d959e5c27649bbdc22d3601ad1e68e7ffb03dfa1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/077cc3fccaba3e9752252dc998937b63ec63bbf5"
        },
        "date": 1788764799380,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11008408,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8864438,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11009638,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2050291,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826013,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068409,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044986,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 585653,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19320,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 825414,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26426,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 603,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "63a3d05ba724e9fee1f592713d1d272457edbcdd",
          "message": "fix(ci): synchronize SQLite burst test and guard empty evidence uploads (#1464)",
          "timestamp": "2026-09-07T04:26:02-03:00",
          "tree_id": "a081daf4ea3a02eae9d8399766d498589ae647c9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/63a3d05ba724e9fee1f592713d1d272457edbcdd"
        },
        "date": 1788766666257,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11008408,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8864438,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11009638,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2045214,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826013,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068409,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050063,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 585653,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19320,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 825414,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26426,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 603,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "28657021fa005447a267334851f9863175631a01",
          "message": "fix(oracle): model VoIP timestamp calibration callbacks (#1465)",
          "timestamp": "2026-09-07T12:58:13-03:00",
          "tree_id": "05a52d267d6c43e9a6cd203d8e064cb570f53351",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/28657021fa005447a267334851f9863175631a01"
        },
        "date": 1788797407591,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11008408,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8864438,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11009638,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2045214,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826013,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068409,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050063,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 585653,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19320,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 825414,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26426,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 603,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2b9a8d799ec2da008cc47fbd1c61d8995d652238",
          "message": "fix(mlow): move per-frame success logs to trace (#1466)",
          "timestamp": "2026-09-07T15:10:19-03:00",
          "tree_id": "371785c6de71f7969541db1533e33efdf4859e9a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2b9a8d799ec2da008cc47fbd1c61d8995d652238"
        },
        "date": 1788805548852,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11008408,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8864438,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11009638,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2045214,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 826013,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619299,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068409,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050063,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 585653,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19320,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 825414,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26426,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 603,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2681a687c35f8fc290c1ff62bb30a857a12df9e0",
          "message": "fix(voip): parse frame info independently of optional RTP extensions (#1470)",
          "timestamp": "2026-09-08T02:44:09-03:00",
          "tree_id": "541f2d6558c6374ae6ab727abcb0fe4fe51d7408",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2681a687c35f8fc290c1ff62bb30a857a12df9e0"
        },
        "date": 1788847141563,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11129432,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8968502,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11125786,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2113995,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085501,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051079,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 852168,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27296,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 603,
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
          "id": "0e8f4e260659d111e2d8d1d6b40584af3fe654f2",
          "message": "chore(deps): bump the cargo-major group with 5 updates (#1468)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-09-08T12:36:47-03:00",
          "tree_id": "1641b2dcbf5431b5d6fd4b03b3ff6e7fc78f9007",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0e8f4e260659d111e2d8d1d6b40584af3fe654f2"
        },
        "date": 1788882112526,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11129432,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8968502,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11125786,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2113995,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41315,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085501,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051079,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 852168,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27296,
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
          "id": "58792d2dccce1907d6ee3eabd9f526723747ed1c",
          "message": "fix(transport): retain TLS sessions and document reuse contracts (#1471)",
          "timestamp": "2026-09-08T13:21:27-03:00",
          "tree_id": "f15ca28c8ea7649069df7ca9fc6eb0efa53e792a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/58792d2dccce1907d6ee3eabd9f526723747ed1c"
        },
        "date": 1788885365834,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11129112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8968246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11125762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2113995,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12829,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051320,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 852168,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27296,
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
          "id": "823158d7b6a7519c991b17505ead4cddf902af09",
          "message": "feat(voip): expose ordered peer video events and a real call fixture (#1472)",
          "timestamp": "2026-09-08T17:22:22-03:00",
          "tree_id": "c4125b897a79b00762e80984e478191437f79166",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/823158d7b6a7519c991b17505ead4cddf902af09"
        },
        "date": 1788899926822,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11129112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8968246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11125762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2113995,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12829,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051320,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 852168,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27296,
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
          "id": "b71c813dbb888aae58ac41994afcd39352148498",
          "message": "feat(voip): expose the immutable initial call peer (#1473)",
          "timestamp": "2026-09-08T18:27:11-03:00",
          "tree_id": "f9c071b871a40112c3e039e0c03dd98f9d0f5422",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b71c813dbb888aae58ac41994afcd39352148498"
        },
        "date": 1788903798535,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11129112,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8968246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11125762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2113995,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12829,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051320,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 852168,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27296,
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
          "id": "b976391099709db4e098f70f6223a00c8ee88a32",
          "message": "perf(cache): flatten slot entry padding (#1480)",
          "timestamp": "2026-09-08T23:29:26-03:00",
          "tree_id": "2aa83b0184f80ca76f65fbf50a05748bfbe66662",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b976391099709db4e098f70f6223a00c8ee88a32"
        },
        "date": 1788922122934,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11132952,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8971830,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11129874,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2117494,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12829,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085318,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051320,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 851694,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27290,
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
          "id": "e3bd71243094ca4dd6a392de157a767511812581",
          "message": "perf(http): reconstruct constant pool report from provenance (#1478)",
          "timestamp": "2026-09-08T23:30:06-03:00",
          "tree_id": "e5fa8836ede23b05bfa742733c193117785a6b09",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e3bd71243094ca4dd6a392de157a767511812581"
        },
        "date": 1788922337498,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11132888,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8971766,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11129874,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2117443,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41271,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085316,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051320,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 851693,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27290,
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
          "id": "57a48c24ca1d0c102e67f477a9c3c067224d9b5f",
          "message": "perf(client): inline unshared Arc wrappers (#1477)",
          "timestamp": "2026-09-08T23:30:29-03:00",
          "tree_id": "2974360cab77587f0023c4e62bbc8f783df36c0a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/57a48c24ca1d0c102e67f477a9c3c067224d9b5f"
        },
        "date": 1788922338549,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11131736,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8970742,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11129842,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2116721,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085184,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051315,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 851077,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27228,
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
          "id": "d44a4a619ba2c2aace7ce6a4f46dd699497ada30",
          "message": "perf(client): retain runtime-only cache config (#1482)",
          "timestamp": "2026-09-08T23:31:39-03:00",
          "tree_id": "3ffde2dcb3968a424db499a4d90870f701431885",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d44a4a619ba2c2aace7ce6a4f46dd699497ada30"
        },
        "date": 1788922528979,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11131864,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8970870,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11129842,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2116917,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839777,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195630,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085120,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051315,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 851165,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27230,
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
          "id": "96cf9b599dbccd6be48fba626b7242cf31991c15",
          "message": "perf(signal): drop empty sender protobuf fields from memory (#1479)",
          "timestamp": "2026-09-08T23:32:41-03:00",
          "tree_id": "746e5d1bc59a9e1443de00c565b2cb6c44512be8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/96cf9b599dbccd6be48fba626b7242cf31991c15"
        },
        "date": 1788922965300,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11130456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8969462,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11125762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2122234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839813,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193889,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085192,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2046238,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 851038,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27224,
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
          "id": "0f505a659b99c47d1674e45924ddac3cdb549aaa",
          "message": "fix(voip): send zero screens in 1:1 video offers (#1476)",
          "timestamp": "2026-09-09T00:05:48-03:00",
          "tree_id": "3f13ac40143e14caf40b8ceeac048aeeead74401",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0f505a659b99c47d1674e45924ddac3cdb549aaa"
        },
        "date": 1788924764609,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11130456,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8969462,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11125762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2117157,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839813,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193889,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085192,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051315,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 851038,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27224,
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
          "id": "d0a6ae910e2366fe69e4799fa63cd4d3793c82aa",
          "message": "fix(message): deduplicate PDO recovery and encrypted retries (#1474)",
          "timestamp": "2026-09-09T00:36:26-03:00",
          "tree_id": "57260992b06152953b152d69b1eac8b6d2b02eba",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d0a6ae910e2366fe69e4799fa63cd4d3793c82aa"
        },
        "date": 1788925782327,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11165720,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9003446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11162618,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2150905,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839829,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193889,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085450,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051144,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 861968,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 27511,
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
          "id": "1528bdc970046a7d8309aa4c050d63e01eb3415e",
          "message": "perf(cache): add metadata-free table for unbounded local maps (#1481)",
          "timestamp": "2026-09-09T00:53:45-03:00",
          "tree_id": "85c8e1637ec41f480672476fc6abbec0319a8701",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1528bdc970046a7d8309aa4c050d63e01eb3415e"
        },
        "date": 1788927057887,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11213656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9045110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11211786,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2191124,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193889,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051144,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 882713,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28158,
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
          "id": "1405eb33bd1c7c801f50f2b87d6fa2b1b397a9c1",
          "message": "fix(voip): send the engine video capability in 1:1 video offers (#1483)",
          "timestamp": "2026-09-09T01:46:27-03:00",
          "tree_id": "beef6255de6e8cb813a984e20f71e815b00dae2a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1405eb33bd1c7c801f50f2b87d6fa2b1b397a9c1"
        },
        "date": 1788930228976,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11213656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9045110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11211786,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2191124,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193889,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051144,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 882713,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28158,
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
          "id": "7aefe3ae4c4567dd8cc70b3bb7b23a64ff69b7af",
          "message": "test(client): pin non-group cache store lifetime (#1488)",
          "timestamp": "2026-09-09T03:04:30-03:00",
          "tree_id": "1f159bdfc199b18957e1f3bd7721b9823acdab28",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7aefe3ae4c4567dd8cc70b3bb7b23a64ff69b7af"
        },
        "date": 1788935894828,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11213656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9045110,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11211786,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2191124,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193889,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051144,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 882713,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28158,
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
          "id": "3b3ae7465d26e8d957df38e5ae60b78049331228",
          "message": "perf(signal): size skipped-key buffers exactly on cold load (#1485)",
          "timestamp": "2026-09-09T03:05:01-03:00",
          "tree_id": "130b3150b2b52efb18f75aff85ce9c4117964c4b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3b3ae7465d26e8d957df38e5ae60b78049331228"
        },
        "date": 1788935990499,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11213720,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9045174,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11211786,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2196201,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086130,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2046067,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 882713,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28158,
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
          "id": "30718d0029a65b491b2a9b1450fef5a34535678c",
          "message": "perf(message): key the dispatch gate by a hashed identity (#1493)",
          "timestamp": "2026-09-09T09:26:36-03:00",
          "tree_id": "acc5a57830819b0c3dd5117704c569e257d83400",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/30718d0029a65b491b2a9b1450fef5a34535678c"
        },
        "date": 1788957645391,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11197240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9028854,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11195474,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2180110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085840,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2046149,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881675,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28160,
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
          "id": "45c221a44b09d96673a8aace7026ee3532e8ab0c",
          "message": "perf(cache): share table growth across cache instantiations (#1491)",
          "timestamp": "2026-09-09T09:27:18-03:00",
          "tree_id": "eed884c7eedbd718e4e3b625a79fc4b3770aba8b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/45c221a44b09d96673a8aace7026ee3532e8ab0c"
        },
        "date": 1788957650816,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11197624,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9028086,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11195490,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2174090,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085840,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2051226,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881823,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28234,
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
          "id": "27f5cd8d068303a24a0d3dfc2b38951c870f07ab",
          "message": "feat(demo): log memory_report() on a periodic timer (#1490)",
          "timestamp": "2026-09-09T09:28:10-03:00",
          "tree_id": "d6250020c30c31e2d78297dc12fe9688b15b2948",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/27f5cd8d068303a24a0d3dfc2b38951c870f07ab"
        },
        "date": 1788957657586,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11256760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9084470,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11253586,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2216732,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086892,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063647,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881823,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28234,
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
          "id": "4940becd58499baeb8f9e3e1a2ef9db3e1d5b610",
          "message": "test(client): pin per-client size saving from runtime cache config (#1487)",
          "timestamp": "2026-09-09T09:29:08-03:00",
          "tree_id": "6c66537bc3ff55ca48ef477ba5816d87af52785a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4940becd58499baeb8f9e3e1a2ef9db3e1d5b610"
        },
        "date": 1788957660115,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11256760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9084470,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11253586,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2216732,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086892,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063647,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881823,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28234,
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
          "id": "0489126621acab555f57fe572e6e48c5a6f629d3",
          "message": "perf(cache): release dedup table buckets when it empties (#1484)",
          "timestamp": "2026-09-09T09:29:58-03:00",
          "tree_id": "1fed8be9ab343c95d08ddc505c7ffdcd396c73ab",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0489126621acab555f57fe572e6e48c5a6f629d3"
        },
        "date": 1788957808026,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9085302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2217784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086652,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063647,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881996,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28240,
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
          "id": "e14300d18da6b0446e77b4777027b6ebbe995c03",
          "message": "test(layout): bound budget asserts and document the rebaseline steps (#1486)",
          "timestamp": "2026-09-09T10:58:57-03:00",
          "tree_id": "160488a987225accd1135bc87a09fc36fec28d60",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e14300d18da6b0446e77b4777027b6ebbe995c03"
        },
        "date": 1788963389495,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9085302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2217784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086652,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063647,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881996,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28240,
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
          "id": "d1b4dca6a7b536ca6d7a918a57b94e0740dff77d",
          "message": "ci: let autofix.ci apply rustfmt fixes on pull requests (#1495)",
          "timestamp": "2026-09-09T20:04:52-03:00",
          "tree_id": "262dbd53bcbcd75c599333282a3b1ef9b2408b5a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d1b4dca6a7b536ca6d7a918a57b94e0740dff77d"
        },
        "date": 1788996451242,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9085302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2217784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086652,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063647,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881996,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28240,
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
          "id": "da31edf7ee9a5aa49bd3093fbe064506de3a8120",
          "message": "fix(voip): treat enc reject as per-device so siblings keep ringing (#1494)",
          "timestamp": "2026-09-10T09:23:07-03:00",
          "tree_id": "74f0d898a43d267f34de9ddf63e86b4deef68e6d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/da31edf7ee9a5aa49bd3093fbe064506de3a8120"
        },
        "date": 1789043872480,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9085302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2217784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086652,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063647,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881996,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28240,
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
          "id": "7348e300f6bf1c125aa60d235de46e06973c03a7",
          "message": "fix(voip): aggregate video parameter sets into STAP-A (#1496)",
          "timestamp": "2026-09-10T13:18:34-03:00",
          "tree_id": "b81b081e963c8a4166f8c4cf4eb044160bebab38",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7348e300f6bf1c125aa60d235de46e06973c03a7"
        },
        "date": 1789057837098,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9085302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2222861,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086652,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2058570,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881996,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28240,
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
          "id": "70e0f8fcb534e544f97b4cb4273d6f6caf2c5389",
          "message": "feat(voip): add video resume and direction-local upgrade timeout (#1498)",
          "timestamp": "2026-09-10T23:05:08-03:00",
          "tree_id": "b2eb3d7028180507624a498766185878064cdfd5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/70e0f8fcb534e544f97b4cb4273d6f6caf2c5389"
        },
        "date": 1789093588667,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9085302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2217784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086652,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063647,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881996,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28240,
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
          "id": "6502b871e35664ffb80044ba7c6317a6427754e2",
          "message": "feat(storage): make bundled sqlite opt-out via sqlite-storage-bundled feature (#1499)",
          "timestamp": "2026-09-11T10:02:06-03:00",
          "tree_id": "c8d551f8f4ecf9149628fb45f6bdb61ac2246d14",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6502b871e35664ffb80044ba7c6317a6427754e2"
        },
        "date": 1789132386344,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11257784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9085302,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257690,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2217784,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086652,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063647,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 881996,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28240,
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
          "id": "1bd6cbceff9c39601dd06f7684c9415c70305611",
          "message": "feat(history): add generic sync admission policy (#1501)",
          "timestamp": "2026-09-13T13:44:24-03:00",
          "tree_id": "c95e01c3a4af5b4221a7771e5c2a545886adc087",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1bd6cbceff9c39601dd06f7684c9415c70305611"
        },
        "date": 1789318686213,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11258968,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9086454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257698,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2218730,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086843,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063663,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 882576,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28252,
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
          "id": "24652ea9e5fce77b56c2bc7a900ea06209a87da8",
          "message": "perf(voip): reduce MLOW encoding allocations and PCM staging (#1500)",
          "timestamp": "2026-09-13T15:16:49-03:00",
          "tree_id": "3a2724db3b4c64db7e2e3ebb4b763af042a6cb2c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/24652ea9e5fce77b56c2bc7a900ea06209a87da8"
        },
        "date": 1789324146832,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11258968,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9086454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257698,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2223807,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086843,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2058586,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 882576,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28252,
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
          "id": "99afc9bf21fdf2146549afa5126be02ec43c9683",
          "message": "chore(deps): bump the cargo group with 8 updates (#1502)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-09-14T19:04:23-03:00",
          "tree_id": "648842070fa1891863b917c01c9789d7b0a46105",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/99afc9bf21fdf2146549afa5126be02ec43c9683"
        },
        "date": 1789424239010,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11252824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9080246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11249530,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2215119,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 840031,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622008,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085202,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065820,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 588093,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 883728,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28251,
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
          "id": "7f0f3a6bfb8b17998f52002efd042479fb67930f",
          "message": "chore(deps): bump the cargo-major group with 2 updates (#1503)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-09-14T19:03:24-03:00",
          "tree_id": "d7e4f103ee2f9266584c34fba9654c623b18a6f3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7f0f3a6bfb8b17998f52002efd042479fb67930f"
        },
        "date": 1789424280174,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11258968,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9086454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11257698,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2218730,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 85759,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622029,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1086843,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063663,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 587665,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 882576,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28252,
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
          "id": "3a08e84e8d7a462f6f87eb98d28fe1ce889a653a",
          "message": "feat(storage): add account lifecycle API to SqliteStore (#1505)",
          "timestamp": "2026-09-16T14:57:15-03:00",
          "tree_id": "3fda5a9240cb9b117dd98bec659d237221fed4f1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3a08e84e8d7a462f6f87eb98d28fe1ce889a653a"
        },
        "date": 1789582784047,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11252824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9080246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11249530,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2215119,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 840031,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 622008,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41140,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12796,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1085202,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065820,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 588093,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19374,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 883728,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28251,
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
          "id": "639ca400a8f964ad98d5df6fadabdb913017d280",
          "message": "perf(sqlite): shrink msg_secrets storage, sweep on startup, resolver parent timestamp (#1504)",
          "timestamp": "2026-09-16T20:09:59-03:00",
          "tree_id": "ed07e5f3e9c8c52286ad72c7425c24f21ca05321",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/639ca400a8f964ad98d5df6fadabdb913017d280"
        },
        "date": 1789601048270,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11260856,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9083254,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11258030,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2218056,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839853,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1085532,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065616,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 588348,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19380,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 883885,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28274,
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
          "id": "4a0721b9aa944822807b95926e2770da4e670b2f",
          "message": "feat(iq): improve profile-picture protocol semantics and typing (#1508)",
          "timestamp": "2026-09-16T20:33:36-03:00",
          "tree_id": "c203c34a2c65281e684b074650c43f3cb28b3fac",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4a0721b9aa944822807b95926e2770da4e670b2f"
        },
        "date": 1789602579862,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11260856,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9083254,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11258030,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2218056,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839853,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1085532,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065616,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 589402,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19399,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 883885,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28274,
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
            "email": "65250731+FelipeMayerDev@users.noreply.github.com",
            "name": "Felipe Mayer",
            "username": "FelipeMayerDev"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ae3cefd86065872a577bbd0ad5ee21b60c86c616",
          "message": "Add chat lock verbs, LockChatUpdate event, and collection resync from version 0 (#1507)",
          "timestamp": "2026-09-17T10:23:40-03:00",
          "tree_id": "51544991db5a61e33b1c65828b47cc0e2e5cd130",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ae3cefd86065872a577bbd0ad5ee21b60c86c616"
        },
        "date": 1789652247726,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11261048,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9083446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11258030,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2218250,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839853,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1085529,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065616,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 589437,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19402,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 884076,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28277,
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
          "id": "7acae337a1acc8bff31e15db56880ca982128332",
          "message": "feat(voip): run the control plane without the resident media engine (#1509)",
          "timestamp": "2026-09-18T18:51:48-03:00",
          "tree_id": "b2be914f32715e709c9f3e5e81ee90aac86644ed",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7acae337a1acc8bff31e15db56880ca982128332"
        },
        "date": 1789769289205,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11261048,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9083446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11258030,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2223327,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839853,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1085529,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060539,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 589437,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19402,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 884076,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28277,
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
          "id": "c4dc16d223917d1aab759e9828f6ea82904f0c99",
          "message": "fix: reduce cache memory overhead and temporary allocations (#1510)",
          "timestamp": "2026-09-19T14:28:37-03:00",
          "tree_id": "643c62728fe590fc8709c4af44dff484d5838ba7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c4dc16d223917d1aab759e9828f6ea82904f0c99"
        },
        "date": 1789839631492,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11260920,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9083318,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11258030,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2218157,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 839853,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1085529,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065616,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 589980,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19424,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 884222,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28283,
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
          "id": "908e62b6a83e2ba714a758dcdc8f1c191e45a256",
          "message": "feat(appstate): add semantic per-mutation observability to sync dispatch (#1511)",
          "timestamp": "2026-09-19T23:05:50-03:00",
          "tree_id": "0d72d1da9abffb515d742c1c46267bceaa5f127d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/908e62b6a83e2ba714a758dcdc8f1c191e45a256"
        },
        "date": 1789870973318,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11300376,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9121014,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11299702,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2242350,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843354,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1085744,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061734,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 589836,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19423,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 891555,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28334,
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
          "id": "4b5f5fd89049f7d1d219a439c5a56c78fd8e1764",
          "message": "Redesign groups public surface around overviews, routing, and metadata (#1513)",
          "timestamp": "2026-09-20T15:48:48-03:00",
          "tree_id": "9bd726631c7f98ff673af761efdbb72e7f578c22",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4b5f5fd89049f7d1d219a439c5a56c78fd8e1764"
        },
        "date": 1789931171012,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11293016,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9114358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11291350,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2232995,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1084522,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065852,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 594167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19520,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 891804,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28342,
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
          "id": "38b98180e987dfa565274343d5facd144d74eb5d",
          "message": "feat(groups): expose cached routing info for participant rosters (#1514)",
          "timestamp": "2026-09-20T18:38:00-03:00",
          "tree_id": "60ce9c69b42355546c401a3786418294f5062f06",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/38b98180e987dfa565274343d5facd144d74eb5d"
        },
        "date": 1789941031519,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11293016,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9114358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11291350,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2232995,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1084522,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065852,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 594167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19520,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 891804,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28342,
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
          "id": "3ac0b7551adcbce846f466724fd87f7872e74c43",
          "message": "docs: fix private routing info rustdoc link (#1515)",
          "timestamp": "2026-09-20T19:24:03-03:00",
          "tree_id": "ae7b071b959d6864fca4f793d93a58ed2b085c84",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3ac0b7551adcbce846f466724fd87f7872e74c43"
        },
        "date": 1789944081223,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11293016,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9114358,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11291350,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2232995,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1084522,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065852,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 594167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19520,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 891804,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28342,
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
          "id": "539ed050817f351d080fb5092179b22cebaceffe",
          "message": "fix(pdo): retry pending lookup across PN/LID aliases (#1517)",
          "timestamp": "2026-09-21T05:32:48-03:00",
          "tree_id": "8b57ac663492d9838878a811f409e083d167917b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/539ed050817f351d080fb5092179b22cebaceffe"
        },
        "date": 1789980567596,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11293304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9114614,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11291398,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2233068,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1084714,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065852,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 594167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19520,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 891997,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28343,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b350504ebc0e922e50c6807b34c8a48bcac02ebd",
          "message": "Read every child of a newsletter message (#1518)\n\nCo-authored-by: João Lucas de Oliveira Lopes <jlucaso@hotmail.com>",
          "timestamp": "2026-09-22T01:43:58-03:00",
          "tree_id": "96afa3b4e2f8892853d4e6c7159736c6f7b19b3d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b350504ebc0e922e50c6807b34c8a48bcac02ebd"
        },
        "date": 1790052953068,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11293304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9114614,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11291398,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2233068,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1084714,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065852,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 594167,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19520,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 894468,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28384,
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
          "id": "9eb43b9bc2bc561cc34a672b4295528bf4c2d733",
          "message": "feat(community): support hidden subgroup visibility (#1522)",
          "timestamp": "2026-09-22T03:17:05-03:00",
          "tree_id": "d6b93b53360499f6379035aa55346773199cb16c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9eb43b9bc2bc561cc34a672b4295528bf4c2d733"
        },
        "date": 1790058739951,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11293304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9114614,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11291398,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2233068,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1084714,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065852,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 595005,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19556,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 894519,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28384,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c65b4028ace77da161824d95530e2b002b9ec332",
          "message": "fix(newsletter): address the history IQ to the server (#1523)",
          "timestamp": "2026-09-22T10:41:19-03:00",
          "tree_id": "6d6290a093f77a11d829778bf70e13489eec9e19",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c65b4028ace77da161824d95530e2b002b9ec332"
        },
        "date": 1790085100499,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11293304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9114614,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11291398,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2233068,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1084714,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065852,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 595005,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19556,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 894643,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28385,
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
          "id": "e0f8cd5b5f7266fa31a4a55a390873b4d487de3c",
          "message": "Emit events for favorite and recent sticker sync (#1538)",
          "timestamp": "2026-09-23T11:07:27-03:00",
          "tree_id": "24b27c3eec3558fd6a709c2c9b4eb707305cb098",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e0f8cd5b5f7266fa31a4a55a390873b4d487de3c"
        },
        "date": 1790173749865,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11296856,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9117814,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11295526,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2240617,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843234,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1085399,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060775,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 595089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19563,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 895528,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28394,
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
          "id": "55945b79343ec4174b8e0dbb8cfb04c4f9acd409",
          "message": "Expose the viewer's channel mute state in newsletter metadata (#1537)",
          "timestamp": "2026-09-23T11:10:48-03:00",
          "tree_id": "ee56cb1feba8137c2677175e83bd6ac27b4e0f8f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/55945b79343ec4174b8e0dbb8cfb04c4f9acd409"
        },
        "date": 1790173834580,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11299064,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9119990,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11299622,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2236730,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843387,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1086099,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065964,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 595089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19563,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 896925,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28458,
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
          "id": "96b8fcf96f5cd244d8cfe34e06f99d6e64c1a37f",
          "message": "Let applications watch and read boolean AB props (#1539)",
          "timestamp": "2026-09-23T11:09:18-03:00",
          "tree_id": "643ffa8233aeb2bd999d60623ceb63c33a8a3eb9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/96b8fcf96f5cd244d8cfe34e06f99d6e64c1a37f"
        },
        "date": 1790173833828,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11299064,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9119990,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11299622,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2236730,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843387,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1086099,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065964,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 595089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19563,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 896689,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28455,
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
          "id": "cbf3568167b2ca76c0737543966400785fc9ca1c",
          "message": "fix(polls): try known creator and voter aliases independently (#1540)",
          "timestamp": "2026-09-23T13:13:55-03:00",
          "tree_id": "9247392ee3707b3734e66b5dfe37807a3fee9d09",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cbf3568167b2ca76c0737543966400785fc9ca1c"
        },
        "date": 1790180974920,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11299064,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9119990,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11299622,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2236730,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 843387,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1086099,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065964,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 595089,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19563,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 897428,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28472,
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
          "id": "f4d73ebe6b19231993a58b5a151b4920dfad1306",
          "message": "fix(media): verify declared hashes on downloads (#1541)",
          "timestamp": "2026-09-23T14:18:45-03:00",
          "tree_id": "0545f74b5aa2e5e3dd0afdd9f6170bcfc8cda52e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f4d73ebe6b19231993a58b5a151b4920dfad1306"
        },
        "date": 1790184757043,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11303320,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9124214,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11303790,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2239269,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 844731,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1086455,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065964,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 595385,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19570,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 898273,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28476,
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
          "id": "b8f3308693656e2f1098bb1876ae051e57a2ba90",
          "message": "fix(voip): learn caller LID-PN from incoming offers (#1542)",
          "timestamp": "2026-09-23T18:28:54-03:00",
          "tree_id": "deb8ee6f14f3f28edd87bf9bee45d1f8849a7691",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b8f3308693656e2f1098bb1876ae051e57a2ba90"
        },
        "date": 1790199899598,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11318680,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9138166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11316190,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2255023,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 846418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1087977,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060877,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 596761,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19595,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 900682,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28552,
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
          "id": "b57ba06d77ac94b7ffb7779dcffb797a8b7706dc",
          "message": "fix(conn): probe pending IQs before watchdog reconnect (#1543)",
          "timestamp": "2026-09-23T20:38:25-03:00",
          "tree_id": "106939b8f6fc8d249a3529309b60fb7b2e2328ec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b57ba06d77ac94b7ffb7779dcffb797a8b7706dc"
        },
        "date": 1790207699308,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11319640,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9139126,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11320286,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2250881,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 846418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1087977,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065954,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 596761,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19595,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 900817,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28555,
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
          "id": "215123065e63e0a7ddefadf0c53740c90467769d",
          "message": "Emit an event for favorite chats sync (#1544)",
          "timestamp": "2026-09-23T20:40:04-03:00",
          "tree_id": "b8d716f8e73acaed3f8829d9ba6fc5fa92f46156",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/215123065e63e0a7ddefadf0c53740c90467769d"
        },
        "date": 1790207762002,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11321304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9140662,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11320326,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2252135,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 846418,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1088248,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065954,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 596802,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19601,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 901302,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28559,
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
          "id": "60d9828a5a2b65efdb219e4a4a701d3a7f0c055a",
          "message": "fix(conn): bound an IQ's write by its deadline and arm the watchdog before it (#1547)",
          "timestamp": "2026-09-24T13:27:33-03:00",
          "tree_id": "b57134a9ebeec65be1ecb2c2197cc3335728cd70",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/60d9828a5a2b65efdb219e4a4a701d3a7f0c055a"
        },
        "date": 1790268083276,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11321016,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9140214,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11320338,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2251080,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 846652,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1088597,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065954,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 596815,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19602,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 901347,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28572,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f7468ae2920a2b0aedbd0848cf121de56a07f93c",
          "message": "fix(appstate): mark a baseline bootstrapped when a sync finds it at the head (#1545)",
          "timestamp": "2026-09-24T13:28:13-03:00",
          "tree_id": "37a7d017ddd23ed8f030d0698181cb395b52447b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f7468ae2920a2b0aedbd0848cf121de56a07f93c"
        },
        "date": 1790268110276,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11320888,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9140086,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11320338,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2251080,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 846540,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82595,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1088597,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2065954,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 596815,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19602,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 901285,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 28570,
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
          "id": "09d195b2fd6b826bb0679ac67c5ae4400ba669a2",
          "message": "feat(groups): add opt-in history sharing on member add (#1550)",
          "timestamp": "2026-09-25T21:00:02-03:00",
          "tree_id": "50e2f471a9acde0ee63e82dbc5dd18c6d940ec65",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/09d195b2fd6b826bb0679ac67c5ae4400ba669a2"
        },
        "date": 1790381874230,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11383064,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9194678,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11382142,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2291889,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1094816,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 599462,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19656,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 926842,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29214,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fb113e441b14100236b1f076a2a1f1f8d4e724ba",
          "message": "feat(newsletter): vote in a channel poll (#1552)",
          "timestamp": "2026-09-26T12:18:03-03:00",
          "tree_id": "2474d23eaf334e0e8c022c27c416eefec52f0d93",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fb113e441b14100236b1f076a2a1f1f8d4e724ba"
        },
        "date": 1790436668683,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11383064,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9194678,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11382142,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2291889,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1094816,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 599462,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19656,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 928126,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29268,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2d8748fad06a97cf20bff1dee81ea19209c3f536",
          "message": "feat(newsletter): carry poll tallies and forwards in live updates (#1554)",
          "timestamp": "2026-09-26T12:18:22-03:00",
          "tree_id": "3472b75262c89bea228686ca8187ff6ed4d8278b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2d8748fad06a97cf20bff1dee81ea19209c3f536"
        },
        "date": 1790436683935,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11383832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9195446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11382142,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2292644,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1094837,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 599479,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19660,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 928902,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29301,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f9811768bdc61340222268bce279c39e962a9638",
          "message": "feat(newsletter): read this account's own reactions and poll votes (#1555)",
          "timestamp": "2026-09-26T13:22:56-03:00",
          "tree_id": "8e4ddb36602d133e2c6fb8f172c1d62b2d48da98",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f9811768bdc61340222268bce279c39e962a9638"
        },
        "date": 1790440688993,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11383832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9195446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11382142,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2292644,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1094837,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 601174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19714,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 928902,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29301,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c9ae1c59189d71fe757166c139a86e2a128291e4",
          "message": "fix(newsletter): read channel state and verification in the server's spelling (#1557)",
          "timestamp": "2026-09-26T18:21:17-03:00",
          "tree_id": "8316418c8851543989bc9ba8776a6ecfa224b0bf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c9ae1c59189d71fe757166c139a86e2a128291e4"
        },
        "date": 1790458246590,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11383832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9195446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11382142,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2292644,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1094837,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 601174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19714,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 929174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29309,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6f18f5d3c98a4e72a498eb4966c67a990da4a59d",
          "message": "feat(newsletter): keep the server's type and polltype tokens on history messages (#1558)",
          "timestamp": "2026-09-26T18:21:52-03:00",
          "tree_id": "1e54ea5d9aa8b45b37a08653950968ccd11f3846",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6f18f5d3c98a4e72a498eb4966c67a990da4a59d"
        },
        "date": 1790458300944,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11383832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9195446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11382142,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2292644,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1094837,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 601174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19714,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 929334,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29312,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3987f7c809a0b1ca3296a0e9a4fbb7ce96ea3181",
          "message": "feat(newsletter): return the stanza id from send_reaction (#1559)",
          "timestamp": "2026-09-28T09:32:28-03:00",
          "tree_id": "35aca5722645ad6ba3e16e16a6e30a1c00e32876",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3987f7c809a0b1ca3296a0e9a4fbb7ce96ea3181"
        },
        "date": 1790599683731,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11383832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9195446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11382142,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2292644,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1094837,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 601174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19714,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 929334,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29312,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "23846f7e34c9842ed93c7a099a8f87818089fab7",
          "message": "fix(newsletter): read a missing channel as NotFound and keep the list when one entry has no id (#1561)",
          "timestamp": "2026-09-28T11:20:47-03:00",
          "tree_id": "7a538b5385666afb4843f71d10fb75671dce0597",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/23846f7e34c9842ed93c7a099a8f87818089fab7"
        },
        "date": 1790605990212,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11383832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9195446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11382142,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2292644,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857941,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83477,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 193923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821946,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621902,
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
            "value": 1094837,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2060900,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 601174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19714,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 931061,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29372,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a214b14c45b83729ec24c7f8b562d751920456de",
          "message": "fix(app-state): tell a chat action with no timestamp apart from one sent at the epoch (#1562)",
          "timestamp": "2026-09-28T18:57:02-03:00",
          "tree_id": "b7eb86f11afd816a2d405da227357d93b957cb71",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a214b14c45b83729ec24c7f8b562d751920456de"
        },
        "date": 1790633511136,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11390040,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9201718,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11390382,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2293528,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857322,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194022,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822144,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621967,
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
            "value": 1096921,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061231,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 601174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19714,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 931236,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29369,
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
          "id": "7e7603282b9ddc9dd7ce975f1cd3cf0c1b7cd60a",
          "message": "chore(deps): bump the cargo group across 1 directory with 10 updates (#1563)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-09-28T18:55:50-03:00",
          "tree_id": "611c4241f2db6d835847e0105c9b7fb59014ca01",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7e7603282b9ddc9dd7ce975f1cd3cf0c1b7cd60a"
        },
        "date": 1790633631186,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11389144,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9200822,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11390398,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2292653,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857322,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194022,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822144,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621967,
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
            "value": 1096921,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061231,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 601174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19714,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 931044,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29369,
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
          "id": "008a68a4fa823e5a3f5e96715d7673dedd259e35",
          "message": "docs: refresh README and crate repository links (#1564)",
          "timestamp": "2026-09-28T21:53:19-03:00",
          "tree_id": "cbc009a67041e6bcd969294626895c81820e14e3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/008a68a4fa823e5a3f5e96715d7673dedd259e35"
        },
        "date": 1790644041708,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11390040,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9201718,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11390382,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2293528,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 857322,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194022,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 42616,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 24084,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1822144,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 621967,
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
            "value": 1096921,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061231,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 601174,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 19714,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 931236,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29369,
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
          "id": "5fbfa4605e9edbe8bcbd58884ef4584343748dc7",
          "message": "fix(send): preserve content metadata on pairwise retries (#1566)",
          "timestamp": "2026-09-29T22:15:37-03:00",
          "tree_id": "20abf7c87ee0b5a275db1d31d21e82c61b31f247",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5fbfa4605e9edbe8bcbd58884ef4584343748dc7"
        },
        "date": 1790731636683,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11429912,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233654,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11428298,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2297073,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874980,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194022,
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
            "value": 624604,
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
            "value": 1098113,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061355,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 607708,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20175,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 933530,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29445,
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
          "id": "7f7dc9f4f6a530ceb272196b5b7aebc8b71d1532",
          "message": "feat(status): surface synced status privacy audience (#1565)",
          "timestamp": "2026-09-29T22:14:42-03:00",
          "tree_id": "c20dc100b28cad3736d82059e5d0352f8f9f97ff",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7f7dc9f4f6a530ceb272196b5b7aebc8b71d1532"
        },
        "date": 1790731672759,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11429528,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233206,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11424162,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2297792,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 873786,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194022,
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
            "value": 624604,
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
            "value": 1098091,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061355,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 605915,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20170,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 933796,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29444,
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
            "email": "66478725+LLawli@users.noreply.github.com",
            "name": "Leonardo Lucas",
            "username": "LLawli"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6f07e3ab7f6a94764c23dfd6298c312b58419e7b",
          "message": "fix(keepalive): start the logged-in connection's keepalive from <success> (#1568)",
          "timestamp": "2026-09-30T14:19:22-03:00",
          "tree_id": "f811ac890713199d0666ac3a613ec4d182133a52",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6f07e3ab7f6a94764c23dfd6298c312b58419e7b"
        },
        "date": 1790789534663,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11429976,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9233654,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11428306,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2297042,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874980,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194022,
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
            "value": 624604,
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
            "value": 1098113,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061355,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 607708,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20175,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 933546,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29445,
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
          "id": "dd0763c4635b353c4ed52def39d030fb2f375f9a",
          "message": "fix(cache): release empty async tables during expiry maintenance (#1569)",
          "timestamp": "2026-09-30T18:19:23-03:00",
          "tree_id": "dac72d4200552669917f09ef44909a17bba5b839",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dd0763c4635b353c4ed52def39d030fb2f375f9a"
        },
        "date": 1790804376960,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11436696,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9239158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11432354,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2302468,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874980,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194022,
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
            "value": 624604,
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
            "value": 1098113,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061355,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 607708,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20175,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935262,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29522,
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
          "id": "c062e122475aa2a5f6865578e63a1c7e877daad6",
          "message": "feat(api)!: make call event receiver ownership explicit (#1570)",
          "timestamp": "2026-09-30T22:14:47-03:00",
          "tree_id": "480d0144195600a5a566f1967745d1e6527a9ad1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c062e122475aa2a5f6865578e63a1c7e877daad6"
        },
        "date": 1790818637350,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11436696,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9239158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11432354,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2302468,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874980,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86748,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 194022,
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
            "value": 624604,
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
            "value": 1098113,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061355,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 607708,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20175,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935262,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29522,
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
          "id": "49202fd48ede5744aeab2fa3d508627a8c7d8855",
          "message": "refactor(api): share client configuration across builders (#1571)",
          "timestamp": "2026-09-30T22:22:32-03:00",
          "tree_id": "61430170a2f1771fa7b136aa23bdf973272d5988",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/49202fd48ede5744aeab2fa3d508627a8c7d8855"
        },
        "date": 1790818654492,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11431864,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9234678,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11428158,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2299507,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874352,
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
            "value": 624604,
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
            "value": 1097217,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061248,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 607708,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20175,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 934109,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29504,
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
          "id": "83981bd79c49ce3638ada2558eed7e57f7f8089c",
          "message": "feat(api)!: execute MEX through consistent operation descriptors (#1572)",
          "timestamp": "2026-09-30T22:23:06-03:00",
          "tree_id": "695079164caf8819508b3ed334ace5ce9832acc2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/83981bd79c49ce3638ada2558eed7e57f7f8089c"
        },
        "date": 1790819513331,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11433144,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9235830,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11428190,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2300475,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874352,
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
            "value": 624604,
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
            "value": 1097425,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2061248,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 607708,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20175,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 934360,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29506,
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
          "id": "7a2a7e7bdd6db9bce34f4dadb3e4881c99a987ac",
          "message": "refactor(api): separate SQLite database and device-scoped stores (#1573)",
          "timestamp": "2026-09-30T22:23:47-03:00",
          "tree_id": "7396accab9e9eaafffe2c84b7d1ef767027230c8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7a2a7e7bdd6db9bce34f4dadb3e4881c99a987ac"
        },
        "date": 1790819765208,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11435352,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9237622,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11432366,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2298321,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874352,
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
            "value": 1097970,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063978,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 607708,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20175,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 934360,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29506,
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
          "id": "0c45252f496c4d7a04afa333c716222a00fd1acb",
          "message": "feat(api): consolidate profile picture lookup outcomes (#1574)",
          "timestamp": "2026-09-30T22:25:01-03:00",
          "tree_id": "1369cdbeaf85be8fb0a8ae4ecab84588534e4705",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0c45252f496c4d7a04afa333c716222a00fd1acb"
        },
        "date": 1790819918842,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11438232,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9240246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11436510,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2301322,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874699,
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
            "value": 1097604,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063631,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 608597,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20214,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935830,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29557,
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
          "id": "6334155c8aacb16d6dbff5db135860b5a418a67e",
          "message": "test: verify bounded newest message-secret retention (#1578)",
          "timestamp": "2026-09-30T22:25:49-03:00",
          "tree_id": "d6b422945050edee5a9018dfbcd7a31d9598fdeb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6334155c8aacb16d6dbff5db135860b5a418a67e"
        },
        "date": 1790820586768,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11438232,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9240246,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11436510,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2301322,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874699,
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
            "value": 1097604,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063631,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 608597,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20214,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935830,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29557,
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
          "id": "3ffa84b6af065d558bdfc208fe6a2412aca14da8",
          "message": "fix(lifecycle): preserve stream conflict kinds in completion reasons (#1579)",
          "timestamp": "2026-09-30T22:26:03-03:00",
          "tree_id": "4b483ff536fbfa04764df98fddbdfea6157565dc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3ffa84b6af065d558bdfc208fe6a2412aca14da8"
        },
        "date": 1790820649090,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11438360,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9240374,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11436510,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2301444,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874699,
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
            "value": 1097604,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2063631,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 608597,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20214,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935848,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29557,
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
          "id": "17be12738767160d1b261c668ff7cfe034d79368",
          "message": "Emit typed reachout timelock push updates (#1580)",
          "timestamp": "2026-09-30T22:28:54-03:00",
          "tree_id": "12b93391b959ef2bb9bbdcfbd9f4d40576cb7736",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/17be12738767160d1b261c668ff7cfe034d79368"
        },
        "date": 1790821037531,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11443032,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9244534,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11440694,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2304736,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874699,
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
            "value": 1098066,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2064035,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 608618,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20216,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 937149,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29606,
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
          "id": "69625f4bcb0ddb075826f8378706c96ec2cc32cd",
          "message": "fix(tools): patch Wasmtime fuel-accounting advisories (#1581)",
          "timestamp": "2026-09-30T22:31:19-03:00",
          "tree_id": "41b72ff4a930c78c3d93f66b2bda57cc4fe3e97c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/69625f4bcb0ddb075826f8378706c96ec2cc32cd"
        },
        "date": 1790821334028,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11443032,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9244534,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11440694,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2304736,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874699,
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
            "value": 1098066,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2064035,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 608618,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20216,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 937149,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29606,
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
          "id": "569c8e3f0b7fb4d87c688c1d23dcbd1c4a6f1de6",
          "message": "fix(cache): roll back automatic async empty-table shrinking (#1585)",
          "timestamp": "2026-09-30T23:45:58-03:00",
          "tree_id": "3bf949127ab9f1190312716ae8b8dfc27809572f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/569c8e3f0b7fb4d87c688c1d23dcbd1c4a6f1de6"
        },
        "date": 1790824365709,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11436376,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9239030,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11432486,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2299310,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874699,
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
            "value": 1098066,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2064035,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 608618,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20216,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935433,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29529,
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
          "id": "bffea385f460d473b18b4618f7d1605089560625",
          "message": "feat(api)!: make picture removal explicit (#1582)",
          "timestamp": "2026-10-01T00:22:10-03:00",
          "tree_id": "26714556769d9d6b32cc40080d222161a714ccf6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bffea385f460d473b18b4618f7d1605089560625"
        },
        "date": 1790825993791,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11436440,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9239094,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11432502,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2299374,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 874699,
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
            "value": 1098071,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2064035,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 608618,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20216,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935467,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29529,
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
          "id": "19eb64cac58bc2e36cc9e2bc3c92fe9af3a78729",
          "message": "Expose borrowed message context and unify context carriers (#1584)",
          "timestamp": "2026-10-01T00:24:29-03:00",
          "tree_id": "688d791d3ae2028c1dd0227e3239e28450602245",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/19eb64cac58bc2e36cc9e2bc3c92fe9af3a78729"
        },
        "date": 1790826791502,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11438360,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9241014,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11436622,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2298699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 876911,
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
            "value": 1098452,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2064035,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612269,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20232,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935467,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29529,
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
          "id": "18beb8c1fe5c0af75461d4849bc8ea6407ee876f",
          "message": "perf: reduce receive future footprint with shared message dispatch (#1588)",
          "timestamp": "2026-10-01T09:29:05-03:00",
          "tree_id": "43814d71dbbc8f3753311a2eee6b04362bf4e3b5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/18beb8c1fe5c0af75461d4849bc8ea6407ee876f"
        },
        "date": 1790858516780,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11438552,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9241014,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11436742,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2293416,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 876911,
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
            "value": 1019063,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2148677,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612269,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20232,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 935694,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29544,
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
          "id": "cd30c9e3af5de986e6200604341af867b9b30264",
          "message": "feat(api)!: expose run outcomes and terminal shutdown reports (#1576)",
          "timestamp": "2026-10-01T09:33:45-03:00",
          "tree_id": "e3c8e9e8d8af2fa551a997b6d81156564b1eef34",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cd30c9e3af5de986e6200604341af867b9b30264"
        },
        "date": 1790858703162,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11457784,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9254006,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11454110,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2308929,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 877764,
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
            "value": 1019871,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2144261,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612269,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20232,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 939749,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29694,
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
          "id": "ca967c5e1349f252680af0c7113e932a5f83e951",
          "message": "perf(sqlite-storage): retire idle WAL readers without replenishing them (#1589)",
          "timestamp": "2026-10-01T09:31:46-03:00",
          "tree_id": "2ff06e3daaac66693c8c681e066c564b9a7f39f0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ca967c5e1349f252680af0c7113e932a5f83e951"
        },
        "date": 1790858712040,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11453688,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9250230,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11449934,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2300121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 877764,
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
            "value": 1019852,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2149350,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612269,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20232,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 938703,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29641,
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
          "id": "a5c173aa1603ee344abf30938a4f6e710b2946ef",
          "message": "refactor(api)!: simplify downloads and expose final failures (#1575)",
          "timestamp": "2026-10-01T09:30:20-03:00",
          "tree_id": "1f0df4e106a0a6c1b2909922dd380228e8162dbc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a5c173aa1603ee344abf30938a4f6e710b2946ef"
        },
        "date": 1790858713987,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11452920,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 9249526,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11449950,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2300121,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 877764,
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
            "value": 1019753,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2148796,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 612269,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 20232,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 938703,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 29641,
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
      }
    ]
  }
}