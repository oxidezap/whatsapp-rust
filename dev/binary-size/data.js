window.BENCHMARK_DATA = {
  "lastUpdate": 1789132388976,
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
          "id": "5428d33f118d2dd901c73fe60bc4b80cdc97ba3c",
          "message": "perf(socket): release the grown out buffer when the send queue drains (#1271)",
          "timestamp": "2026-08-10T19:02:05-03:00",
          "tree_id": "495c770285d59739f3c3eb7e8e55f4e4871dc3a1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5428d33f118d2dd901c73fe60bc4b80cdc97ba3c"
        },
        "date": 1786400039442,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439606,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1933015,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709631,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019522,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1988844,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533462,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17415,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 761982,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23767,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b2343e4313ad2977bff38dab5696fb150e82cb68",
          "message": "perf(store): reserve the prekey map for a batch insert (#1270)",
          "timestamp": "2026-08-10T19:56:17-03:00",
          "tree_id": "192193514add514df402729596d55170707a2ae6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b2343e4313ad2977bff38dab5696fb150e82cb68"
        },
        "date": 1786403384937,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439606,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1933015,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709319,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019522,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1989156,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 761982,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23767,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "af740033e45079ea3baad0a1d17507e0d84d663a",
          "message": "perf(sqlite): let sibling devices share one connection pool (#1274)",
          "timestamp": "2026-08-10T20:28:04-03:00",
          "tree_id": "8d4c27f8f13369eb91bed259b05cdcf3b9cae42a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/af740033e45079ea3baad0a1d17507e0d84d663a"
        },
        "date": 1786404978685,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439606,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1933015,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709319,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019522,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1989156,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 761982,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23767,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "763aea911ebcb416a5218bebc4945eb7c1baec03",
          "message": "docs(perf): inventory per-client retention and bounds (#1273)",
          "timestamp": "2026-08-10T20:45:46-03:00",
          "tree_id": "633abe8b82fdd1d2158ae796c169ba2d36c56f6d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/763aea911ebcb416a5218bebc4945eb7c1baec03"
        },
        "date": 1786405952364,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439606,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1933015,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709319,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019522,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1989156,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762446,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23771,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5c9e4dc6e75664cdc46b2da45a73356125e991f8",
          "message": "fix(send): stop redistributing the sender key on every admin revoke (#1278)",
          "timestamp": "2026-08-11T12:27:05-03:00",
          "tree_id": "411b7eca4e99fe1d56cdb398d8f98bf8f8c9debd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5c9e4dc6e75664cdc46b2da45a73356125e991f8"
        },
        "date": 1786462703900,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1932901,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709631,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019518,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1988844,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762401,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23771,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8f2beb702c7a194f88feb2f7df37b49ecff5d57f",
          "message": "perf(send): pin the warm group stanza as flat in group size (#1279)",
          "timestamp": "2026-08-11T13:47:28-03:00",
          "tree_id": "d0c648d92eef8fc46f0ec55a258c6c7ddbdd5e03",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8f2beb702c7a194f88feb2f7df37b49ecff5d57f"
        },
        "date": 1786467582603,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1932901,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709319,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019518,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1989156,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762401,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23771,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5fdf3cca1baba08f8e3c6437307e6d8ac8c90f6b",
          "message": "test(send): pin that a warm group send encodes nothing per recipient (#1281)",
          "timestamp": "2026-08-11T14:27:24-03:00",
          "tree_id": "9c853acb681e53db417bfefa98052a3c88f1f96f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5fdf3cca1baba08f8e3c6437307e6d8ac8c90f6b"
        },
        "date": 1786469814351,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1932901,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709631,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019518,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1988844,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762401,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23771,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "575c724dd5f15eebf1076e479c3056de08d90e44",
          "message": "docs(perf): measure allocation on the group stanza build (#1282)",
          "timestamp": "2026-08-11T15:16:48-03:00",
          "tree_id": "a77ad2edaa30f05e00247679b04b0262816c8621",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/575c724dd5f15eebf1076e479c3056de08d90e44"
        },
        "date": 1786472834958,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1932901,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709319,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019518,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1989156,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762401,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23771,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0e90a2a6ba5f3812be2fec7e418296a728da6ba0",
          "message": "docs(build): measure the codegen flags a consumer can set, and why we cannot (#1280)",
          "timestamp": "2026-08-11T15:17:35-03:00",
          "tree_id": "bc06376a618bbd1475d53d37dc7f1379610b7b8e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0e90a2a6ba5f3812be2fec7e418296a728da6ba0"
        },
        "date": 1786472838823,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1932901,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709319,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1019518,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1989156,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762401,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23771,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d76a7c4ac2c61e99ab1c3326f802aebc3ee68795",
          "message": "bench(client): measure the group send the client crate actually pays (#1283)",
          "timestamp": "2026-08-11T18:04:08-03:00",
          "tree_id": "f007bf2b861b25fd18dd1234165b6433df7e3b9a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d76a7c4ac2c61e99ab1c3326f802aebc3ee68795"
        },
        "date": 1786482912264,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1926865,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 708928,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1017564,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997537,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762401,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23771,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bc33aeed6353ec7058a6e1001169bdfe5271f135",
          "message": "bench(client): address the remaining review comments on #1283 (#1285)",
          "timestamp": "2026-08-11T18:39:04-03:00",
          "tree_id": "7191839fd9c682b4345b085f3a206fedd62f4abf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bc33aeed6353ec7058a6e1001169bdfe5271f135"
        },
        "date": 1786485123599,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1926871,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 708928,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1017564,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997537,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762380,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23766,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "139b3150e538ee85f2f79e830ed5c00bf73a89cc",
          "message": "feat(chat-store): add a session-wide arrival-ordered message page (#1284)",
          "timestamp": "2026-08-11T18:43:01-03:00",
          "tree_id": "e63dd709565fd8fee2a4eefdc15ca8b3766dfc70",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/139b3150e538ee85f2f79e830ed5c00bf73a89cc"
        },
        "date": 1786485435779,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10529240,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8439542,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10529126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1931948,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 708928,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1017564,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1992460,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 762380,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23766,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "fadlannizar5@gmail.com",
            "name": "Nizar Izzuddin Yatim Fadlan",
            "username": "nizarfadlan"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "575c6865f1c45a468c04371b3931e61bc62ed7b9",
          "message": "feat: expose public app-state resync API (#1267)",
          "timestamp": "2026-08-11T21:04:41-03:00",
          "tree_id": "a6f89e3761d1d2afda3688d257beef5da3f255e3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/575c6865f1c45a468c04371b3931e61bc62ed7b9"
        },
        "date": 1786493982627,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10565528,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8458678,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10566078,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1945253,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709631,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1018244,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996834,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 765732,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23848,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "186b6251050d91ebd04ff11ba5f7833ad5d34eb7",
          "message": "fix(client): announce a connection the critical sync left degraded (#1291)",
          "timestamp": "2026-08-11T21:50:23-03:00",
          "tree_id": "27c2fdb2635c23ec52856b87508ccd3c755a119c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/186b6251050d91ebd04ff11ba5f7833ad5d34eb7"
        },
        "date": 1786496381393,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10564728,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8457974,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10566030,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1944923,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709240,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1827341,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1017901,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997225,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533670,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17422,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 765718,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23856,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "145608489+Guflly@users.noreply.github.com",
            "name": "Guflly",
            "username": "Guflly"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8e1b50291687a680013329cbc51e8846c8787e8e",
          "message": "feat(appstate): surface call_log mutations as a CallLogSync event (#1275)\n\nCo-authored-by: João Lucas <55464917+jlucaso1@users.noreply.github.com>",
          "timestamp": "2026-08-11T22:14:09-03:00",
          "tree_id": "4a44422317a868772008a97027bd7b94787e3fd4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8e1b50291687a680013329cbc51e8846c8787e8e"
        },
        "date": 1786497967829,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10568760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8461494,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10570246,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1946229,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 708928,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1828706,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1018703,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997537,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533726,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17428,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 766615,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23863,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0c7626948ae11f6615cdf52c486688eea302e6c6",
          "message": "diag(send): measure which term invalidates each group-path device memo (#1292)",
          "timestamp": "2026-08-12T01:22:37-03:00",
          "tree_id": "946bbeda48dda49d2993764a9f04e82426e501a9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0c7626948ae11f6615cdf52c486688eea302e6c6"
        },
        "date": 1786509164857,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10569336,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8462070,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10570246,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1951862,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 709631,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22885,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1828706,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553538,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1018703,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1991757,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 533726,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17428,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 767656,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 23873,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 462,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "874328f375b4c5bb98b00d8a33f7259f4828adee",
          "message": "build(codegen): own the whatspec codegen and regenerate at 2.3000.1044659339 (#1293)",
          "timestamp": "2026-08-12T01:22:53-03:00",
          "tree_id": "c3a78b76f51a1b85a2636e4afed27ea43d2d051d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/874328f375b4c5bb98b00d8a33f7259f4828adee"
        },
        "date": 1786509585022,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10656120,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8542646,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10653302,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1947052,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 712655,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1026935,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996834,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 538872,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17689,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 770023,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24006,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ed7723a907c330eb376e045200dfb192a1b5de33",
          "message": "fix(call_log): take a synced call's direction from its creator, not the direction fields (#1295)",
          "timestamp": "2026-08-12T11:39:14-03:00",
          "tree_id": "94523bcc1fb7b5edaace49425f3830ddcda0081e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ed7723a907c330eb376e045200dfb192a1b5de33"
        },
        "date": 1786546199835,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10656312,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8542902,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10653238,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1947279,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 711952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1026935,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997537,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 538872,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17689,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 770018,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24010,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7c971c82eee0c94b866c24f4041f6ee352921dc6",
          "message": "perf(store): cap in-memory msg_secrets so wasm linear memory stops climbing (#1297)",
          "timestamp": "2026-08-13T08:56:45-03:00",
          "tree_id": "c487400423094969492c72f24d09d39a354dbf1a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7c971c82eee0c94b866c24f4041f6ee352921dc6"
        },
        "date": 1786623056095,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10657016,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8543606,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10653238,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1952356,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 711952,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027630,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1992460,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 543910,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17845,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 770316,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24013,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f1e2d43c8789e2da7a5162f0fe04c94fc5e2c4b9",
          "message": "fix(send): report a DM no recipient device could be encrypted for (#1299)",
          "timestamp": "2026-08-13T22:39:31-03:00",
          "tree_id": "eda660a61bcc040b4ecafe40c3f3cfe95d66dc46",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f1e2d43c8789e2da7a5162f0fe04c94fc5e2c4b9"
        },
        "date": 1786672363567,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10660824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8546294,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10658286,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1948394,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 713179,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027860,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997607,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544345,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17877,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 770650,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24024,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6f26cf8647dd7d887d0b83b5343343f7174679de",
          "message": "fix(send): seal NoRecipientDeviceError against new variants (#1301)",
          "timestamp": "2026-08-13T22:52:14-03:00",
          "tree_id": "820c058d5221decf481b0492819c90d6db0784c9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6f26cf8647dd7d887d0b83b5343343f7174679de"
        },
        "date": 1786672829686,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10660824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8546294,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10658286,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1948394,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 713570,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027860,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997216,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544345,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17877,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 770650,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24024,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6abb44886b87faec2a3673ef9204b2f65759bfe3",
          "message": "fix(retry): repair the sender key before the recent-message lookup (#1300)",
          "timestamp": "2026-08-13T23:07:54-03:00",
          "tree_id": "18d7854b9ee8ed90ebc5f9a1c3fb03a302e703a2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6abb44886b87faec2a3673ef9204b2f65759bfe3"
        },
        "date": 1786673675420,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10664824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8549878,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10662430,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1953509,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 711523,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1028223,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997216,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544345,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17877,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 772947,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24048,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5b1f0f516abc74fd386aca5312bfa9b615871156",
          "message": "fix(retry): repair the peer session before the recent-message lookup (#1303)",
          "timestamp": "2026-08-14T00:58:48-03:00",
          "tree_id": "bf098ca9de2a8ead0133aad8d82de2e3ce9031cb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5b1f0f516abc74fd386aca5312bfa9b615871156"
        },
        "date": 1786680506120,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10663480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8548662,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10662358,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1958513,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 711132,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027156,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1992530,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544345,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17877,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 772931,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24053,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c4254eb26734fb4af75564367df75acc77079f2f",
          "message": "feat(observability): count the devices a send could not key (#1304)",
          "timestamp": "2026-08-14T02:29:49-03:00",
          "tree_id": "f687212bd611ecd6729ab1d86595e834f38d5046",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c4254eb26734fb4af75564367df75acc77079f2f"
        },
        "date": 1786685916190,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10669080,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8553910,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10666470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1954285,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 715506,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027944,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996907,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544577,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17886,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774014,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24082,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "pedrobartolini@outlook.com",
            "name": "Pedro Bartolini",
            "username": "pedrobartolini"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3772e92bceedde327eb01d23403bd2b487c6aff5",
          "message": "feat(voip): expose CallHandle::set_video_orientation (#1302)\n\nCo-authored-by: pedrobartolini <pedrobartolini@users.noreply.github.com>\nCo-authored-by: João Lucas <55464917+jlucaso1@users.noreply.github.com>",
          "timestamp": "2026-08-14T02:31:10-03:00",
          "tree_id": "81db1d8280718381cfce25b4225caef9b1ea0dc4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3772e92bceedde327eb01d23403bd2b487c6aff5"
        },
        "date": 1786686271068,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10669080,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8553910,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10666470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1954285,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 714803,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027944,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997610,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544643,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17886,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774014,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24082,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
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
          "id": "b7139c8b67e8a826571c6399a9badccaf15c432d",
          "message": "fix(appstate): box traced resync future (#1306)",
          "timestamp": "2026-08-14T11:12:47-03:00",
          "tree_id": "06fd588ef7e2e3da6f4e41bfd528fe578dbdbf80",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b7139c8b67e8a826571c6399a9badccaf15c432d"
        },
        "date": 1786717666637,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10669336,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8554166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10666470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1959653,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 714803,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027944,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1992533,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544643,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17886,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774499,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24097,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
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
          "id": "b62ed8a74ea3760cf88b66fa886c7768334bfb9e",
          "message": "fix(receipt): address a read our own account authored (#1307)\n\nCo-authored-by: João Lucas <55464917+jlucaso1@users.noreply.github.com>",
          "timestamp": "2026-08-14T11:55:45-03:00",
          "tree_id": "1591de345494b960c37c69386f7dc9d093afbea7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b62ed8a74ea3760cf88b66fa886c7768334bfb9e"
        },
        "date": 1786720034876,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10670328,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8555126,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10666518,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1955495,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 714803,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027944,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997610,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 544643,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17886,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774836,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24099,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8b295bf19684d4de124603f7c139dfc207ffeb38",
          "message": "feat(recv): parse the envelope type, enc mediatype and report-to-admin group IQs (#1308)",
          "timestamp": "2026-08-14T17:01:54-03:00",
          "tree_id": "bd6b4d0d104ef19261b837c971e9a98d208ff43c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8b295bf19684d4de124603f7c139dfc207ffeb38"
        },
        "date": 1786738169654,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10684088,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8568566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10682842,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1964017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 718572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1029101,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997610,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 547338,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17955,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774967,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24100,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1ef22b9a5ab55e1eef7bab733669e2320edd37a1",
          "message": "feat(codegen): generate the protocol enums from the whatspec catalog (#1309)",
          "timestamp": "2026-08-15T15:17:33-03:00",
          "tree_id": "c1a40dd02ccb97082bd10b6a4fa14ece071c1c5c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1ef22b9a5ab55e1eef7bab733669e2320edd37a1"
        },
        "date": 1786818608218,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10684088,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8568566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10682842,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1969094,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 718572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1029101,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1992533,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 547338,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17955,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774967,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24100,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "52b19e13e82bb040c9544cdaa9ac59e4489bc0d5",
          "message": "feat(wacore): bind five more enums to the catalog and retire a dead event (#1310)",
          "timestamp": "2026-08-15T20:07:09-03:00",
          "tree_id": "27af79709bcdd412e0e3f11a9681b2fff4be4de9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/52b19e13e82bb040c9544cdaa9ac59e4489bc0d5"
        },
        "date": 1786835800060,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10683768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8568374,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10682850,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1969067,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 720057,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027450,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1992533,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 547304,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17952,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 774967,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24100,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "971ff4f4863299cf1974f0d03d0d271bc0aecf68",
          "message": "fix(props): revive two dead A/B gates, and derive w:g2 addressing from the IR (#1311)",
          "timestamp": "2026-08-15T23:26:21-03:00",
          "tree_id": "20a0097ec15fa8701c565195afd9bd359b879903",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/971ff4f4863299cf1974f0d03d0d271bc0aecf68"
        },
        "date": 1786847663542,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10683992,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8568566,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10682874,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1964018,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 720201,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027473,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997610,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 547304,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17952,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 775105,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24105,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ce65e8def5e0fcacfff6a976acc04aa627fc551e",
          "message": "feat(codegen): generate the stanza tag and notification vocabularies (#1312)",
          "timestamp": "2026-08-16T00:52:06-03:00",
          "tree_id": "2128848b1feeaa0388b00d90aa2a4da74e27a421",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ce65e8def5e0fcacfff6a976acc04aa627fc551e"
        },
        "date": 1786852761163,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10684952,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8569462,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10682866,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1964390,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 721080,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1027473,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997298,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17961,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 775424,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24111,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "117e921ec685648b57fbd043d16f9b7a66def341",
          "message": "feat(lid): act on the server's refresh_lid ack flag (#1313)",
          "timestamp": "2026-08-16T02:17:06-03:00",
          "tree_id": "956b8b8752972b5043546f76b07651f7beb32b87",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/117e921ec685648b57fbd043d16f9b7a66def341"
        },
        "date": 1786857984346,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10693304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8576950,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10691170,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1971236,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 720768,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 553540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1028023,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997610,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548056,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17961,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 777587,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24192,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ea4ea0b45a6bf480336493969707a770a1e951e5",
          "message": "feat(ib): act on the server's client_expiration deadline (#1314)",
          "timestamp": "2026-08-16T13:56:31-03:00",
          "tree_id": "75a8d736ebfce33e77da4c83d3d2518b5c21f994",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ea4ea0b45a6bf480336493969707a770a1e951e5"
        },
        "date": 1786899854305,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10704856,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8587254,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10703674,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1975048,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 723112,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1028258,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1998301,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548483,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17977,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 778357,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24206,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2debb71cbc1fccb58ffd185f4fe5d1f3e7b51d70",
          "message": "fix(session): coalesce concurrent prekey fetches for the same address (#1315)",
          "timestamp": "2026-08-16T17:37:58-03:00",
          "tree_id": "45330b1da03a704984799304d5f118861be4a31a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2debb71cbc1fccb58ffd185f4fe5d1f3e7b51d70"
        },
        "date": 1786913103535,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10725624,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8602294,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10724166,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1990708,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 724250,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031915,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1992838,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548483,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17977,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783916,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24401,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bc0f9246b4a5d6e5d6edfbc766e36493ad868491",
          "message": "fix(groups): coalesce concurrent metadata queries for one group (#1316)",
          "timestamp": "2026-08-16T20:56:13-03:00",
          "tree_id": "640a5357188934a09fa31eb81a22ac6dd0c7ffb7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bc0f9246b4a5d6e5d6edfbc766e36493ad868491"
        },
        "date": 1786925056010,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10751512,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8627062,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10748766,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2009785,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 723859,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897777,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1032342,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1998306,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548483,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17977,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 786753,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24507,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "52e253c8defb1aa88bea44f79950ac48f802d599",
          "message": "fix(signal): name the size floor a sender-key distribution missed (#1318)",
          "timestamp": "2026-08-16T22:02:58-03:00",
          "tree_id": "0ab43326b75b167a256fe10a8125342daed381e3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/52e253c8defb1aa88bea44f79950ac48f802d599"
        },
        "date": 1786929238249,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10752216,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8627702,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10748854,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2010506,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 723859,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897741,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1032342,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1998306,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548483,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17977,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 786950,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24508,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "48eeec450e7ea15a19ee54608c81d1eeca3cc0b5",
          "message": "fix(message): identify an undecryptable message by its sender too (#1317)",
          "timestamp": "2026-08-16T23:47:22-03:00",
          "tree_id": "e91d51e378a7b35450a7d056c1dd06cc5d88e5bd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/48eeec450e7ea15a19ee54608c81d1eeca3cc0b5"
        },
        "date": 1786935423358,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10780824,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8654582,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10777518,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2034792,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 724710,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897741,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033855,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1998306,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548487,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17979,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 797531,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24727,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0d54574c4e7845a2ea2c6cb253c99a0a61eb3cfd",
          "message": "fix(pdo): make the once-per-message gate per sender (#1319)",
          "timestamp": "2026-08-17T02:49:34-03:00",
          "tree_id": "2af2ce34233c82d7bbc63503e3ebf75830e049dc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0d54574c4e7845a2ea2c6cb253c99a0a61eb3cfd"
        },
        "date": 1786946203905,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10775960,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8650230,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10773494,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2031407,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 725022,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897741,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033916,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997029,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548487,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17979,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795486,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24653,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "33d90902c6571adce26d9c95c052373c52f8a242",
          "message": "perf(voip): make the MLow encoder attributable per stage, then cut what that showed (#1320)",
          "timestamp": "2026-08-18T10:37:32-03:00",
          "tree_id": "15f59189db8ec1a9b9378bc7c82e550b5d021044",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/33d90902c6571adce26d9c95c052373c52f8a242"
        },
        "date": 1787060801695,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10775960,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8650230,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10773494,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2031407,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 725413,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183220,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 22907,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1897741,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033916,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996638,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548487,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17979,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795486,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24653,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9fb1747c41058f2091b7cbc83035057c15f5ee76",
          "message": "perf(core): eliminate heap churn in sender keys, lthash and send fanout (#1321)",
          "timestamp": "2026-08-18T11:28:15-03:00",
          "tree_id": "4b2c53579e65b54029ad6baf2c14ec3fff2ee5e6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9fb1747c41058f2091b7cbc83035057c15f5ee76"
        },
        "date": 1787064053094,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10777912,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8652278,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10777602,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2038518,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 725170,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182810,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033983,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1992264,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548684,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17977,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795461,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24651,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2d9221c9a833d98217132b5223f6417b8d844840",
          "message": "perf(voip): halve MLow CELP instructions by slicing the beam-search loops (#1322)",
          "timestamp": "2026-08-18T12:34:51-03:00",
          "tree_id": "f8bd27691c72ebd042f0ba420ba8bf6a91121582",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2d9221c9a833d98217132b5223f6417b8d844840"
        },
        "date": 1787067634286,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10777912,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8652278,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10777602,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033441,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 725482,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182810,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556762,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033983,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997029,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 548684,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 17977,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795461,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24651,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d5c180d93013f97ef6eda58c28893802371fdffc",
          "message": "perf(core): eliminate allocation churn in addon crypto, ADV validation and prekeys (#1324)",
          "timestamp": "2026-08-18T13:39:59-03:00",
          "tree_id": "5f6d17c55a1a8ff74325569704969de6303be40f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d5c180d93013f97ef6eda58c28893802371fdffc"
        },
        "date": 1787072100302,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10782616,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8656310,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10781794,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033046,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 729104,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183577,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1034204,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996950,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 551598,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18008,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24651,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "315f699c500c7b140c9cdf3e1c8d619e7cd3b031",
          "message": "ci(codspeed): stop an apt stall from hanging a shard for hours (#1325)",
          "timestamp": "2026-08-18T15:19:56-03:00",
          "tree_id": "874b85821ab40d4513b774410e504a0ca9a972e8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/315f699c500c7b140c9cdf3e1c8d619e7cd3b031"
        },
        "date": 1787077979450,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10782616,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8656310,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10781794,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033046,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 729104,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183577,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1034204,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996950,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 551598,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18008,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24651,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "865d59097d783b578130127bc69d3698acf32a3f",
          "message": "perf(voip): opt-in half-length real FFT for the MLow encoder (#1323)",
          "timestamp": "2026-08-18T16:26:15-03:00",
          "tree_id": "b3ed8aacb598ef08a9d1b50c98e82141ee2e054b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/865d59097d783b578130127bc69d3698acf32a3f"
        },
        "date": 1787082447468,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10782616,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8656310,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10781794,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033046,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 729416,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183577,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1034204,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996638,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 551598,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18008,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24651,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6263ef6922d85665676077757a47a1b59d070006",
          "message": "ci(codspeed): reach for apt-get update last, not first (#1327)",
          "timestamp": "2026-08-18T16:46:30-03:00",
          "tree_id": "982a431a20fb8c06fc8f50c2944bc50383e7bc13",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6263ef6922d85665676077757a47a1b59d070006"
        },
        "date": 1787083254977,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10782616,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8656310,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10781794,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033046,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 728713,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183577,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1034204,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1997341,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 551598,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18008,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795532,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24651,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d46d4bf7188aa8ad84f9aa60dc23eefcf3f24445",
          "message": "perf(core): eliminate allocation churn in phash, ack attributes and message classify (#1326)",
          "timestamp": "2026-08-18T18:14:09-03:00",
          "tree_id": "2ab9a6e45f5ac3c42cca7c2d9b9c31c37716eae0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d46d4bf7188aa8ad84f9aa60dc23eefcf3f24445"
        },
        "date": 1787088447851,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10792472,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8666550,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10789842,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033142,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 739823,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183552,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033620,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996396,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 551620,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18010,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 796454,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24661,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ff4ac10c5005b43443001ea53740f18a89b1b374",
          "message": "fix(group): stop stranding a participant the bot could not key (#1328)",
          "timestamp": "2026-08-19T01:27:11-03:00",
          "tree_id": "355750ae281b15c9d23044759d8d55347b83d146",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ff4ac10c5005b43443001ea53740f18a89b1b374"
        },
        "date": 1787114167837,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10793432,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8667510,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10789866,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033324,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 740652,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183552,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033607,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996396,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552069,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18021,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 796500,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24662,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e0ea6dde8ddc22820a77c2f1fa1a6531662fcff3",
          "message": "perf(voip): make the half-length real FFT the MLow encoder's only path (#1330)",
          "timestamp": "2026-08-19T03:57:20-03:00",
          "tree_id": "8f005c01397ab8fbcf939775c40f39fe51f10bf9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e0ea6dde8ddc22820a77c2f1fa1a6531662fcff3"
        },
        "date": 1787123502961,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10793432,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8667510,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10789866,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033324,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 740652,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183552,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033607,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996396,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552069,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18021,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 796500,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24662,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fdf2214b716ccf3842f6e4f89f11d725688d7e19",
          "message": "perf(appstate): drop the generic sort from the collection batch order (#1331)",
          "timestamp": "2026-08-19T15:38:25-03:00",
          "tree_id": "146a248d38928f3266f17f5a40141bb0995c26b2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fdf2214b716ccf3842f6e4f89f11d725688d7e19"
        },
        "date": 1787165692875,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10763480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8654454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10761226,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2025359,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 740340,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183552,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21439,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1896506,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1033607,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1991631,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552069,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18021,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 794714,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24608,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "08a281ac3fbf5dc1091bbf94645b0f0ab86b229e",
          "message": "feat(client): tell a reconnecting client from a finished one (#1332)",
          "timestamp": "2026-08-20T21:18:41-03:00",
          "tree_id": "6ef52cf510b0a6ff5c6c4c340faabdf3dbf3c035",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/08a281ac3fbf5dc1091bbf94645b0f0ab86b229e"
        },
        "date": 1787272266811,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10713944,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8608950,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10711178,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1994173,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 732752,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183516,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24087,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1890264,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030963,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1994236,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552069,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18021,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 784272,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24359,
            "unit": "copies"
          },
          {
            "name": "deps crates (Cargo.lock)",
            "value": 463,
            "unit": "crates"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c1ffe8da2472970bcc780cc25ef39bbfd296e9de",
          "message": "feat(wam): emit WhatsApp Metrics on the regular channel from a generated catalog (#1333)",
          "timestamp": "2026-08-21T01:52:29-03:00",
          "tree_id": "42d58701958a127eb1f7e707c96d13d0ddfdfa36",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c1ffe8da2472970bcc780cc25ef39bbfd296e9de"
        },
        "date": 1787288678158,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10716632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8611638,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10715314,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1999266,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 733143,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183516,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1892896,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031008,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1988768,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552161,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18067,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 784389,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24371,
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
          "id": "c8b59c4a2c78c0be97052311b9ef59fcb1e8dca9",
          "message": "fix(wam): report what the store loses, and count only what the client saw (#1334)",
          "timestamp": "2026-08-21T02:35:56-03:00",
          "tree_id": "eb23aa05e01629ff11859fc35b1369ced7f70c03",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c8b59c4a2c78c0be97052311b9ef59fcb1e8dca9"
        },
        "date": 1787291092089,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10716632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8611638,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10715314,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1994189,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 732440,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183516,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1892896,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031008,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1994548,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552161,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18067,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 784389,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24371,
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
          "id": "4dff7ea0b3285624dbca24c1f496f70d2de46fa7",
          "message": "docs(client): say what Reconnecting promises, and what it does not (#1336)",
          "timestamp": "2026-08-21T11:32:29-03:00",
          "tree_id": "c099d5d4a59fd9ff00eeffe3b6fc6d6b5db67afd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4dff7ea0b3285624dbca24c1f496f70d2de46fa7"
        },
        "date": 1787323457491,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10716632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8611638,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10715314,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1994189,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 732752,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183516,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1892896,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031008,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1994236,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552161,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18067,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 784389,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24371,
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
          "id": "718ccb0a0760cf7c9c0d99d29da525d0c3d8fb42",
          "message": "chore(ci): compile once, wait on nothing, and report what every test cost (#1337)",
          "timestamp": "2026-08-21T14:42:50-03:00",
          "tree_id": "37bfe63b612a8b3dffa7a80b1ec27550005e5118",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/718ccb0a0760cf7c9c0d99d29da525d0c3d8fb42"
        },
        "date": 1787334814903,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10716632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8611638,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10715314,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1994189,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 732831,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 84159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183516,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24105,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1892896,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1031008,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1994157,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552161,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18067,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 784389,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24371,
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
          "id": "26e89c36b6cfe8a89c1fe079b1f1b7cac1c4deb2",
          "message": "fix(test): put the two handshake round trips back under Miri (#1339)",
          "timestamp": "2026-08-21T14:55:48-03:00",
          "tree_id": "b0dbdd14ec7d905b89c21517a3cbc712e848951d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/26e89c36b6cfe8a89c1fe079b1f1b7cac1c4deb2"
        },
        "date": 1787335570504,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10716376,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8611766,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10715322,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1994285,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 733831,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83101,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183685,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1892896,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030867,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1994076,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552377,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18068,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "dc344852d690314fd1c9f42ac1acf074ee7cc58c",
          "message": "perf(core): optimize dm ratchet skipping, fanout node mapping and marshal exact (#1341)",
          "timestamp": "2026-08-21T17:04:31-03:00",
          "tree_id": "417285b6cd0c587ce871338c57cfe8566654a073",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dc344852d690314fd1c9f42ac1acf074ee7cc58c"
        },
        "date": 1787343530894,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10710680,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8606518,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10706802,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1993829,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734043,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 78370,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1892896,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030867,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1994467,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552642,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18073,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "6fa8525caeb521790d8644b0c5f1179c801f142d",
          "message": "refactor(voip): move the relay media stack to the sans-io rtc crates (#1340)",
          "timestamp": "2026-08-21T17:53:52-03:00",
          "tree_id": "5dbe25b8581f297eccfd222ed2ae04b57623b678",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6fa8525caeb521790d8644b0c5f1179c801f142d"
        },
        "date": 1787346405081,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10710680,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8606518,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10706802,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1993829,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734746,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 78370,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1892896,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030867,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1993764,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552642,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18073,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "37535316bab5f4c77dea7eddc8b923c293b223b8",
          "message": "build(deps): bump ureq from 3.3.0 to 3.4.0 (#1290)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-08-21T18:06:07-03:00",
          "tree_id": "34a5735c4592d4625cc3121dda665a44daecb9ea",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/37535316bab5f4c77dea7eddc8b923c293b223b8"
        },
        "date": 1787347020446,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10713592,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8608502,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10711554,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1993829,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734358,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 78370,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1892896,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030912,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1995999,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552642,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18073,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "f0853f44a1e654cb543192d6e9d2523cf31a0bec",
          "message": "build(deps): bump zlib-rs from 0.6.6 to 0.6.7 (#1287)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-08-21T18:06:34-03:00",
          "tree_id": "51a65685c7b359399c603ebf2d54f9e8c85108af",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f0853f44a1e654cb543192d6e9d2523cf31a0bec"
        },
        "date": 1787347708403,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10716472,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8611254,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10715662,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1993711,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734110,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81245,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1893067,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030912,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996039,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552642,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18073,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "4ba258a2caeb881a2f7f6d0819b9855418c3447b",
          "message": "build(deps): bump zerocopy from 0.8.55 to 0.8.56 (#1289)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-08-21T18:06:51-03:00",
          "tree_id": "0279d33bfd9c85e73ed04a0f3a7611c034c55c41",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4ba258a2caeb881a2f7f6d0819b9855418c3447b"
        },
        "date": 1787347713598,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10716472,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8611254,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10715662,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1998788,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734501,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81245,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1893067,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030912,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1990571,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552642,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18073,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "599fa63863a6caaf1783c6d4fea6503be0ef1e6f",
          "message": "build(deps): bump async-trait from 0.1.91 to 0.1.92 (#1288)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-08-21T18:07:08-03:00",
          "tree_id": "7c65481b730f34fee3e231e657d3853e21aa4b35",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/599fa63863a6caaf1783c6d4fea6503be0ef1e6f"
        },
        "date": 1787348004109,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10715960,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8610742,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10711590,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1998460,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 733753,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81245,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1893067,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030841,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1991241,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552642,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18073,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "d69aca567edadc101838e759cf1985f11fa7a24c",
          "message": "build(deps): allow the base64 0.22/0.23 split cargo-deny now sees (#1342)",
          "timestamp": "2026-08-21T18:13:56-03:00",
          "tree_id": "efcc8aeba0c69224eab817b32d6fb7fe4d46f2b8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d69aca567edadc101838e759cf1985f11fa7a24c"
        },
        "date": 1787348347187,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10715960,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8610742,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10711590,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1993383,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 733753,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81245,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183060,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24274,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1893067,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41463,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030841,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1996318,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552642,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18073,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "e5f2241bb532918d7e518dac9eee203d361eef05",
          "message": "build(deps): bump base64 from 0.22.1 to 0.23.1 (#1286)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-08-21T19:13:44-03:00",
          "tree_id": "74163a094236e516676f6c1a7f703c91a0e4133c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e5f2241bb532918d7e518dac9eee203d361eef05"
        },
        "date": 1787351228693,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10715000,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8610166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10711278,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1998977,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734944,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81245,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182961,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24283,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1893067,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556279,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41510,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030841,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1989186,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 552885,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18079,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 783934,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24369,
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
          "id": "1489b7da9a6a7e3cadc4337ee46399da7de25915",
          "message": "perf(core): optimize appstate encode buffer, stanza parsing and metadata allocations (#1343)",
          "timestamp": "2026-08-21T19:21:38-03:00",
          "tree_id": "b40d07288476a327fdc74166be0184874ee7caee",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1489b7da9a6a7e3cadc4337ee46399da7de25915"
        },
        "date": 1787351655182,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10703256,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8597750,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10699254,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1995929,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 732961,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81245,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 183381,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1879099,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 555995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41510,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030540,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1995654,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 551868,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18058,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 785932,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24398,
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
          "id": "83e3eaa474da4b7f64ea1b1eaab0142ecc6f2e85",
          "message": "perf(core): optimize usync query builder, prekey extraction and sender key allocations (#1344)",
          "timestamp": "2026-08-24T12:39:42-03:00",
          "tree_id": "362d1200c9646685634916361aab90473bc58b33",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/83e3eaa474da4b7f64ea1b1eaab0142ecc6f2e85"
        },
        "date": 1787586786662,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10699192,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8594038,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10695134,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1995722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 730177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82260,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182281,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1879099,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 555995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41510,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030230,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1995412,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 551433,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18053,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 785927,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24398,
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
          "id": "82b23dbf8ca12696485ac0b0509354ab6f2b132b",
          "message": "Extract the chat store into its own repository (#1346)",
          "timestamp": "2026-08-24T13:35:34-03:00",
          "tree_id": "caeedb61db61b6779f0feb87ef47126b8d594ce1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/82b23dbf8ca12696485ac0b0509354ab6f2b132b"
        },
        "date": 1787590023809,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10699192,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8594038,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10695134,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1995722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 730177,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82260,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182281,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1879099,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 555995,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41510,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1030230,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1995412,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 551433,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18053,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 785927,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24398,
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
          "id": "2246a8b674f3dabb618dfccf67707a34234f4869",
          "message": "Update every dependency to its current release (#1347)",
          "timestamp": "2026-08-24T16:29:12-03:00",
          "tree_id": "fb4725895cba882c1730f07f7540c3eae84a9379",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2246a8b674f3dabb618dfccf67707a34234f4869"
        },
        "date": 1787600634404,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10737624,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8625142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10736874,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2005866,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 733321,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82062,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182380,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1881851,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556703,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41535,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1037635,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2002580,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 554233,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18126,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 786653,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24408,
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
          "id": "923a1425bb149b71e1afd4d7f37e84168dd3cd62",
          "message": "perf(core): optimize history sync lid mappings, tctoken candidates and group secret allocations (#1349)",
          "timestamp": "2026-08-24T21:27:00-03:00",
          "tree_id": "2e121041b0d178f087f9ebacdcec0a6c2ca0aecf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/923a1425bb149b71e1afd4d7f37e84168dd3cd62"
        },
        "date": 1787618208760,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10740120,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8627318,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10736982,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2006811,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734899,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81929,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182380,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1881851,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556703,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41535,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1037148,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2002763,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 554331,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18133,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 786873,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24414,
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
          "id": "aed9a4608c27788b738128709bf60d6e20b8c7d3",
          "message": "perf(voip): optimize audio and video RTP framing, in-place SRTP/WARP and H.264 packetization (#1348)",
          "timestamp": "2026-08-24T21:33:16-03:00",
          "tree_id": "426f67a491afb1702693d8c6f771d4797cb417ac",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/aed9a4608c27788b738128709bf60d6e20b8c7d3"
        },
        "date": 1787618475707,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10740120,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8627318,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10736982,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2006811,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 735290,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81929,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182380,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1881851,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556703,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41535,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1037148,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2002372,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 554331,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18133,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 786873,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24414,
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
          "id": "d3f1a5882bcff9475da4e32e0fad7058dc89f596",
          "message": "perf(media): optimize upload and download batch crypto, streaming I/O and in-place decryption (#1350)",
          "timestamp": "2026-08-24T22:29:22-03:00",
          "tree_id": "86ae6c3127ba6d645c6d0b45e20f3968348d8f0a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d3f1a5882bcff9475da4e32e0fad7058dc89f596"
        },
        "date": 1787622076985,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10740376,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8627446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10736950,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2011487,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734851,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81929,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182281,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1881851,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556437,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41535,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1037148,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1998479,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 555133,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18160,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 786468,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24415,
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
          "id": "ed543cba3572d941fe03d54612b4d3fb84efb270",
          "message": "fix(voip): make call-control actions reach the peer (#1351)",
          "timestamp": "2026-08-25T04:31:21-03:00",
          "tree_id": "62fef08e215d8025d76214f9e5a8899ddeb25afe",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ed543cba3572d941fe03d54612b4d3fb84efb270"
        },
        "date": 1787643834344,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10740376,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8627446,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10736950,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2011487,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734539,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81929,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182281,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1881851,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556437,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41535,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1037148,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1998791,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 555100,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18160,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 786468,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24415,
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
          "id": "d6a0455f052650e174140a7c8f84b3a184b0ab3a",
          "message": "fix(recv): dispatch a resent message once (#1352)",
          "timestamp": "2026-08-26T20:59:16-03:00",
          "tree_id": "d0577334df4015c13070bb67d70a33ba2ebfe4db",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d6a0455f052650e174140a7c8f84b3a184b0ab3a"
        },
        "date": 1787789511329,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10750872,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8636982,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10749350,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2014157,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734928,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81929,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 182281,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1881851,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 556437,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41535,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038444,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2003941,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 555102,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18160,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790156,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24539,
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
          "id": "0c06e195cd8ad44ab38554228ae2738b562c2b3a",
          "message": "perf(core): shrink the long-lived caches, group mappings, device records, archived sessions (#1353)",
          "timestamp": "2026-08-27T20:58:26-03:00",
          "tree_id": "865d733225112274302913dcc93785a42bd3f1b1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0c06e195cd8ad44ab38554228ae2738b562c2b3a"
        },
        "date": 1787875817259,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10759832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8643574,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10758154,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2016604,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 734764,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81934,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186836,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 562981,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1039063,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 1999303,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 556682,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18204,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 791247,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24562,
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
          "id": "a582b39a75df7eb2390661f8fc73391b0cadbd15",
          "message": "perf(core): shrink device-resolution and Signal bookkeeping caches (#1354)",
          "timestamp": "2026-08-28T03:12:57-03:00",
          "tree_id": "0b66ef3a2413d7acd9a37089cec1888d1f597fa7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a582b39a75df7eb2390661f8fc73391b0cadbd15"
        },
        "date": 1787898739891,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10763448,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8647030,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10762486,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2017500,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 736107,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186836,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 562981,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038531,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2000925,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 557529,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18263,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 793067,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24617,
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
          "id": "25c1e267318d1597a7379a2ebcaa23802eba56e2",
          "message": "fix(voip): pick the inbound codec from the negotiation, and never go silent without saying so (#1111)",
          "timestamp": "2026-08-28T10:55:09-03:00",
          "tree_id": "0546a09f62eeb863597d4d551dac51564e26617f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/25c1e267318d1597a7379a2ebcaa23802eba56e2"
        },
        "date": 1787926001277,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10763448,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8647030,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10762486,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2012423,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 736107,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186836,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 562981,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038531,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006002,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 557638,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18264,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 793067,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24617,
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
          "id": "6966ddc25d4f896cfefa8fb7aa2025ca824ce37f",
          "message": "fix(voip): make a video offer WhatsApp parses, and stop dropping video facts in silence (#1355)",
          "timestamp": "2026-08-28T12:16:44-03:00",
          "tree_id": "a742a49dae0cbc8404250632d99e02b4b127ad51",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6966ddc25d4f896cfefa8fb7aa2025ca824ce37f"
        },
        "date": 1787931017782,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10763960,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8647478,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10762470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2017476,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 736543,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186836,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 562981,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038531,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2000925,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 557806,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18268,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 793067,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24617,
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
          "id": "cab4d64c31e10a4aea62825bd2666e1a1717f639",
          "message": "fix: four wire/reporting divergences from WhatsApp Web — status receipts, prekey-low routing, encrypt acks, replay classification (#1356)",
          "timestamp": "2026-08-28T15:26:09-03:00",
          "tree_id": "b2ac4499a5674fef0fead3a32d66a1707929b64b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cab4d64c31e10a4aea62825bd2666e1a1717f639"
        },
        "date": 1787942747629,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10764632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8648118,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10762494,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2018152,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 736543,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 562981,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1038531,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2000925,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 557806,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18268,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 793210,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24619,
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
          "id": "01b63dbc22b8ca801a4ac8ef3ecd01844ce20e15",
          "message": "fix(notif): stop dropping four things WA Web's own parsers read (#1357)",
          "timestamp": "2026-08-28T16:25:07-03:00",
          "tree_id": "d161310f53668b41ee870bee9f09e6cccd874578",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/01b63dbc22b8ca801a4ac8ef3ecd01844ce20e15"
        },
        "date": 1787945880463,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10773656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8656182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10770902,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2022587,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 739587,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 562981,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1039647,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2000242,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 558509,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18284,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795253,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24693,
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
          "id": "94d6c95c918becf1742762a928dc97be0abc0fcc",
          "message": "feat(sqlite-storage): route connections and blocking work through a platform shim (#1358)",
          "timestamp": "2026-08-28T17:41:58-03:00",
          "tree_id": "d73abb16663f250ad029b9544503730a6f368ff0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/94d6c95c918becf1742762a928dc97be0abc0fcc"
        },
        "date": 1787950435899,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10775704,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8657846,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10775070,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2017510,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 739275,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 563731,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1039861,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006327,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 558509,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18284,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795253,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24693,
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
          "id": "654b4a733775b29360c9475eb203538dc38854a6",
          "message": "fix(version): resolve the app version where sw.js cannot be reached (#1359)",
          "timestamp": "2026-08-29T01:22:30-03:00",
          "tree_id": "5403a582e4ddcab3a61efe19922281643bd73651",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/654b4a733775b29360c9475eb203538dc38854a6"
        },
        "date": 1787978337124,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10774872,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8656886,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10775326,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2015043,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 740767,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 563731,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1039867,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006327,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 559082,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18302,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 795405,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24696,
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
          "id": "1ace865716726daec8d0f00335453fcb8de35d1b",
          "message": "fix(version): bound the sdk revision lookup, and survive a blocked browser source (#1360)",
          "timestamp": "2026-08-29T02:36:01-03:00",
          "tree_id": "e4e92f40378fb78f743e9aa63f8a164f2d7e0017",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1ace865716726daec8d0f00335453fcb8de35d1b"
        },
        "date": 1787982502340,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10778776,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8659894,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10780074,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2022812,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 740376,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 563731,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1040047,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2001711,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 559676,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18312,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 796377,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24748,
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
          "id": "890addb02af1112460aa8c4bd8d7656dc4d9612f",
          "message": "fix(send): let a caller see a DM fan-out that skipped devices (#1362)",
          "timestamp": "2026-08-29T20:26:55-03:00",
          "tree_id": "e8df1a713dc40a036e876be30b9680a75a5a8eec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/890addb02af1112460aa8c4bd8d7656dc4d9612f"
        },
        "date": 1788046755509,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10797208,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8672758,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10793126,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033441,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 740753,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 563731,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1042527,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2001078,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 559796,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18313,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 798927,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24821,
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
          "id": "535732f44a540a7017942a0481eba0a675f2bd0c",
          "message": "fix(send): resend to the devices the fan-out named but never reached (#1363)",
          "timestamp": "2026-08-29T22:02:42-03:00",
          "tree_id": "4d24d26ba0e3a90277919d48500f706dedcf5990",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/535732f44a540a7017942a0481eba0a675f2bd0c"
        },
        "date": 1788052380318,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10802872,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8678198,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10801318,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2031058,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 742545,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 563731,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043447,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006155,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560164,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18326,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 799461,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24827,
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
          "id": "b1e5e7a49652aa6bfb83c06802dabb905c6ca493",
          "message": "Let a call be placed where there is no UDP socket (#1364)",
          "timestamp": "2026-08-30T18:36:18-03:00",
          "tree_id": "3f7f05832584e094217804198ca7d73609a40f0a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b1e5e7a49652aa6bfb83c06802dabb905c6ca493"
        },
        "date": 1788126554722,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10802872,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8678198,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10801318,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2031058,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 742233,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 24582,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 563731,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043447,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006467,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560164,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18326,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 799461,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24827,
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
          "id": "60d80a272cd2f18798a214ef66f047e26b2c2b98",
          "message": "fix(appstate): key snapshot records by index, and stop a stranded collection from spending five round trips on it (#1365)",
          "timestamp": "2026-08-30T18:46:22-03:00",
          "tree_id": "8ecc059afbba38c06280bdbbb9d35b1180c19d18",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/60d80a272cd2f18798a214ef66f047e26b2c2b98"
        },
        "date": 1788127132582,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10820696,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8693686,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10818318,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2038992,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 744978,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29733,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043561,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2001149,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560405,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18335,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801038,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24878,
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
          "id": "0f4b95827fb6bc3751a7df75f6fa20e5eba252f4",
          "message": "fix(send): classify AI rich-response family as text, not the media default (#1366)\n\nCo-authored-by: Salientekill <Salientekill@users.noreply.github.com>",
          "timestamp": "2026-08-30T23:35:00-03:00",
          "tree_id": "058f8dbf482cc4d162904fe1671543325fa9644e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0f4b95827fb6bc3751a7df75f6fa20e5eba252f4"
        },
        "date": 1788144415481,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10820760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8693750,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10818326,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 744676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29733,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043561,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006617,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560503,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18335,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801038,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24878,
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
          "id": "ff0813ba5d420e4a8ce0fc6efb28f2d4036c463f",
          "message": "fix(tests): seed app-state fixtures as bootstrapped (#1367)",
          "timestamp": "2026-08-31T00:03:03-03:00",
          "tree_id": "0ab873eac0a37d3029db27b874f970207b8a629f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ff0813ba5d420e4a8ce0fc6efb28f2d4036c463f"
        },
        "date": 1788145782253,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10820760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8693750,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10818326,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 744364,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29733,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043561,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006929,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560503,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18335,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801038,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24878,
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
          "id": "4c39db90cd2dc6f7b6c4ec8340c0ce173401b43c",
          "message": "fix(tests): move the harness version bump ahead of the IQ answer (#1368)",
          "timestamp": "2026-08-31T00:49:54-03:00",
          "tree_id": "5f537457458b3f47f85c23a3bff7ccc34b9770b0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4c39db90cd2dc6f7b6c4ec8340c0ce173401b43c"
        },
        "date": 1788148838970,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10820760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8693750,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10818326,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2038992,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 744755,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29733,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043561,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2001461,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560503,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18335,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801038,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24878,
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
          "id": "210bf74226a450045064b3f980931e58567e82dc",
          "message": "diag(appstate): say whether a snapshot MAC mismatch is the key or the fold (#1369)",
          "timestamp": "2026-08-31T13:28:49-03:00",
          "tree_id": "5c58ae7e75c4d877a3c5ab17c35b3fec5979e7b0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/210bf74226a450045064b3f980931e58567e82dc"
        },
        "date": 1788194680955,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10821656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8694518,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10818334,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2038992,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 744927,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29733,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043753,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2001852,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560503,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18335,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801295,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24886,
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
          "id": "c6b9136b5c8462e0a94c8b02b7b58dff7a571253",
          "message": "perf(bench): measure jid identity and the caches keyed by it (#1370)",
          "timestamp": "2026-08-31T14:57:33-03:00",
          "tree_id": "391a40701eaf88be2b795a5d53ebc042eb07965f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c6b9136b5c8462e0a94c8b02b7b58dff7a571253"
        },
        "date": 1788200042257,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10821656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8694518,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10818334,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 745239,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 81935,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29733,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043753,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006617,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560503,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18335,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801295,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24886,
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
          "id": "2421037ef271d36280a30cb1980a423ab5509f86",
          "message": "fix(jid): treat the legacy phone-user spelling as one identity (#1371)",
          "timestamp": "2026-08-31T15:00:48-03:00",
          "tree_id": "a15363c1f2ea8272928fae834445b377dab63bf0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2421037ef271d36280a30cb1980a423ab5509f86"
        },
        "date": 1788200114545,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10841592,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8694134,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10841302,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033413,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 745272,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82376,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29733,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043416,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006582,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560368,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801171,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24890,
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
          "id": "5370f021de926fafb351a34ff883dc9ea0793128",
          "message": "Say how many snapshot records reached the fold without an index (#1373)",
          "timestamp": "2026-08-31T19:15:42-03:00",
          "tree_id": "9fb09752c06465add4080494ee67fbadd3b2e2ca",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5370f021de926fafb351a34ff883dc9ea0793128"
        },
        "date": 1788215275409,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10841656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8694198,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10841302,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033413,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 745670,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82376,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29733,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043416,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006270,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560368,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801182,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24890,
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
          "id": "0640fd2f20ce0c1bb67afc8c5d621e1f8c66551e",
          "message": "Fold, key and encode app state the way WhatsApp Web actually does (#1374)",
          "timestamp": "2026-08-31T21:52:57-03:00",
          "tree_id": "3903a1e491809428dcfebbb1ac97b8408da74698",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0640fd2f20ce0c1bb67afc8c5d621e1f8c66551e"
        },
        "date": 1788224780670,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10842232,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8694582,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10841406,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2038490,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 745072,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 82376,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29982,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878965,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1043416,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2001896,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560368,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18340,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801226,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 24889,
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
          "id": "860744fb634e9b1703d69626180c66ae07c1ff04",
          "message": "Ask the primary for a collection whose snapshot will not validate (#1375)",
          "timestamp": "2026-09-01T11:36:44-03:00",
          "tree_id": "0124b311dce5b72f534a3d0d38cd5f749820269b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/860744fb634e9b1703d69626180c66ae07c1ff04"
        },
        "date": 1788274240681,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10920760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8758966,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10917338,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2070052,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 759734,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880769,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053691,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006741,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560699,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 818488,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25381,
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
          "id": "3a98a87606b057f1381e64231ad6743ceab17971",
          "message": "fix(mex): stop sending a persisted query fewer variables than it declares (#1378)",
          "timestamp": "2026-09-01T14:15:40-03:00",
          "tree_id": "078c58404f88fe5c6a153237248083124074733c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3a98a87606b057f1381e64231ad6743ceab17971"
        },
        "date": 1788283594690,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10920760,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8758966,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10917338,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2075129,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 759734,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880769,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053691,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2001664,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560699,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18354,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 818666,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25382,
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
          "id": "4bfb0b1839c03044783e20265a904ec1a780385b",
          "message": "fix(keepalive): measure the dead-socket deadline with a clock that cannot jump (#1379)",
          "timestamp": "2026-09-01T15:28:03-03:00",
          "tree_id": "550fdc541f9d5efcd06c2cefbfc2733a079c523a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4bfb0b1839c03044783e20265a904ec1a780385b"
        },
        "date": 1788288389075,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10920888,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8759158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10917346,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2070411,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 759598,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880769,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053691,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006662,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560615,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18351,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 819068,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25392,
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
          "id": "79ebc1114b75ce8d0e88a0d3302031e2ef5d6965",
          "message": "chore(ci): batch dependabot updates into grouped pull requests (#1382)",
          "timestamp": "2026-09-01T16:32:40-03:00",
          "tree_id": "384a59a36a851c487cd3b540d6ed51725972374e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/79ebc1114b75ce8d0e88a0d3302031e2ef5d6965"
        },
        "date": 1788292196387,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10920888,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8759158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10917346,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2070411,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 759598,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880769,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053691,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006662,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 560615,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18351,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 819068,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25392,
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
          "id": "61f21c4332072188049ac7503d6b77970a038e28",
          "message": "feat(usync): surface the username the server already sends (#1381)",
          "timestamp": "2026-09-01T16:32:57-03:00",
          "tree_id": "a9ce8a9a9e7e1d37ca254efd99cdf119765bf76e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/61f21c4332072188049ac7503d6b77970a038e28"
        },
        "date": 1788292202167,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10921720,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8759990,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10921442,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2070411,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 760019,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880769,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053723,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2007053,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 562214,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18395,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 821497,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25495,
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
          "id": "97644475e774383a0133e2560790da6d6f450a8f",
          "message": "fix(offline): end an interrupted resume with a signal instead of silence (#1380)",
          "timestamp": "2026-09-01T16:43:21-03:00",
          "tree_id": "3bb2334523c3d2dace40f98ec5ba93d6efd9e20e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/97644475e774383a0133e2560790da6d6f450a8f"
        },
        "date": 1788292688008,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10924408,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8762102,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10921570,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2072045,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 760722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83159,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 186852,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880769,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568591,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053550,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2006869,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 562220,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18395,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 822638,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25510,
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
          "id": "82cb5f3cf74ea0439a29b8c032b05e846f4a0be8",
          "message": "chore(deps): bump the cargo group with 4 updates (#1383)",
          "timestamp": "2026-09-01T16:46:46-03:00",
          "tree_id": "2e1a7ea2fc537dd99a3b70dd682c9ac2d39dab39",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/82cb5f3cf74ea0439a29b8c032b05e846f4a0be8"
        },
        "date": 1788293181312,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10955768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8792630,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10954338,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2072175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 758833,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83162,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 191269,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880940,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568947,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053714,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2033833,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 565731,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18526,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 822685,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25512,
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
          "id": "9be10573aa47bc8dcae42918c553250879383d67",
          "message": "chore(deps): bump the actions group with 5 updates (#1384)",
          "timestamp": "2026-09-01T16:47:56-03:00",
          "tree_id": "ad124d2eca6752f044972e4366869a928808101d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9be10573aa47bc8dcae42918c553250879383d67"
        },
        "date": 1788293549640,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10955768,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8792630,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10954338,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2072175,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 758833,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83162,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 191269,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880940,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568947,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053714,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2033833,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 565731,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18526,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 822685,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25512,
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
          "id": "3896d9ca438d367f191000a212c8980e8e47b5c1",
          "message": "fix(ci): get the MSRV job and cargo-deny green again (#1386)",
          "timestamp": "2026-09-02T03:00:34-03:00",
          "tree_id": "36ef00cd6800130c6322aec96acedd21f8009ae7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3896d9ca438d367f191000a212c8980e8e47b5c1"
        },
        "date": 1788329969400,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10952632,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8789494,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10950218,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2072218,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 759323,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83162,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 191170,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878223,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568505,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053425,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2033741,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 565731,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18526,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 822685,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25512,
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
          "id": "0688c7a0fa721f4f58534b153ae88c1c8cf20482",
          "message": "fix(offline): close the ordering gaps the terminal lock still left open (#1387)",
          "timestamp": "2026-09-02T10:05:22-03:00",
          "tree_id": "6ca21e3a7a118b053be34d5ad24ae78bba99547c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0688c7a0fa721f4f58534b153ae88c1c8cf20482"
        },
        "date": 1788355051542,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10953912,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8790774,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10950194,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2078333,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 758932,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83162,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 191170,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 30016,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878223,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 568505,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053648,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2029055,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 565731,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18526,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 822801,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25512,
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
          "id": "ec72862c315cfea50c27404a0777a6f9bfae4d84",
          "message": "perf: trim per-message copies on the crypto, cache, registry and storage paths (#1388)",
          "timestamp": "2026-09-02T10:05:59-03:00",
          "tree_id": "5a01f725c90caf842663cc0d6de0b7a11ce151bc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ec72862c315cfea50c27404a0777a6f9bfae4d84"
        },
        "date": 1788355055407,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10928056,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8766454,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10925094,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2065386,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 759717,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83162,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 190017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878223,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 559175,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1053072,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2028983,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 565731,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18526,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 820848,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25513,
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
          "id": "b918e4d7cdb8fe4aef53459aff9980a51280a961",
          "message": "perf: shed resident per-chat and per-entry memory, trim per-message copies (#1389)",
          "timestamp": "2026-09-02T13:23:44-03:00",
          "tree_id": "8cb117ff8bf13479aa75b96b258abd53598d4ff5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b918e4d7cdb8fe4aef53459aff9980a51280a961"
        },
        "date": 1788367112122,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10852728,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8695094,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10851794,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1995868,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 759170,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83849,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 189232,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878223,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 559175,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1054032,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2027193,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 567011,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18583,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 789935,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25118,
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
          "id": "310b969be8bfbf5279f3df360d2d97d594f6a958",
          "message": "feat(voip): ask the peer for a keyframe, by RTCP PLI (#1385)",
          "timestamp": "2026-09-02T13:49:01-03:00",
          "tree_id": "dbe5ae1c16bf9c8a42a6a579057b7d9ed2a564e4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/310b969be8bfbf5279f3df360d2d97d594f6a958"
        },
        "date": 1788368435683,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10852728,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8695094,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10851794,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 1990791,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 758946,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83849,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 189232,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878223,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 559175,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1054032,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2032494,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 567011,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18583,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 789935,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25118,
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
          "id": "c9be92319d42d9d837e7b48f00fec09531ec8d9b",
          "message": "perf!: skipped keys out of the protobuf chain, MessageInfo 968→536 B, inline message ids (#1390)",
          "timestamp": "2026-09-02T15:08:38-03:00",
          "tree_id": "5107bea0dfcfb62d79b19cf35e9f489ef7a6d46d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c9be92319d42d9d837e7b48f00fec09531ec8d9b"
        },
        "date": 1788373246680,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10875960,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8716598,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10872678,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2001999,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 760920,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83849,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 192185,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880862,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 559175,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056363,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2032593,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 567419,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18610,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790888,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25177,
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
          "id": "c7860743e889e0df9199eeefa4beaee2faea40dc",
          "message": "fix(bot): carry the inbound-only metadata into MessageContext (#1391)",
          "timestamp": "2026-09-02T18:17:28-03:00",
          "tree_id": "2ec024df48947974f2de947da75ccb88b3a8b416",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c7860743e889e0df9199eeefa4beaee2faea40dc"
        },
        "date": 1788384627636,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10876600,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8717174,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10876798,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2002523,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 760696,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83849,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 192185,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880862,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 559175,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056446,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2032817,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 567419,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18610,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790937,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25177,
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
          "id": "1ee13b4487f6876f7f73710e563ce32223ac737e",
          "message": "bench(client): receive one message from the decoded stanza to the event (#1392)",
          "timestamp": "2026-09-02T18:52:05-03:00",
          "tree_id": "81fab9e88a475ac6204f53a3cd88e3b52fdd2c3a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1ee13b4487f6876f7f73710e563ce32223ac737e"
        },
        "date": 1788386572597,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10876600,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8717174,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10876798,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2007600,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 760920,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83849,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 192185,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1880862,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 559175,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056446,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2027516,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 567419,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18610,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790937,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25177,
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
          "id": "8f34dadb8bcf8e24b825396040d2d20d07249dc4",
          "message": "perf(store): batch the flush's deletes; drop the redundant device_id indexes (#1395)",
          "timestamp": "2026-09-03T05:06:05-03:00",
          "tree_id": "7fcb9795e683c7b94e00b8496e2a32c9858e3a7f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8f34dadb8bcf8e24b825396040d2d20d07249dc4"
        },
        "date": 1788423410299,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10907160,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8742326,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10907854,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2003403,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 762671,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83849,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 580429,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056234,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2032909,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 567495,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18615,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 789365,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25126,
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
          "id": "2072713f70ce70dcae482de9e10b10ab9da88d81",
          "message": "perf(appstate): inline external blobs once, hand them out as Bytes, move actions into events (#1394)",
          "timestamp": "2026-09-03T05:05:16-03:00",
          "tree_id": "971f0eb3b5b8fabd8bcf4d518d9c3dcf032b6391",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2072713f70ce70dcae482de9e10b10ab9da88d81"
        },
        "date": 1788423419837,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10879192,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8719926,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10877006,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2002628,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 761736,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83849,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 559807,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1055838,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2033221,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 567495,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18615,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 789376,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25126,
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
          "id": "803c5e6872228c7cbfdaf139e9b32546fe60f4d9",
          "message": "perf(send): resolve a DM's Signal addresses once per device-memo entry; client-level DM send bench (#1396)",
          "timestamp": "2026-09-03T05:07:25-03:00",
          "tree_id": "9d0098f81de90c87ba050fb59cbf1c81d24f8365",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/803c5e6872228c7cbfdaf139e9b32546fe60f4d9"
        },
        "date": 1788423440093,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10913752,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8744310,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10912074,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2003065,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 764321,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 83562,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 580429,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056931,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2033076,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 568301,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18649,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 789920,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25148,
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
          "id": "cab961fc0a5ba4f7124e8578bfb6522c393ad4d5",
          "message": "fix(e2e): hold B offline with pause instead of racing reconnect_immediately (#1403)",
          "timestamp": "2026-09-03T11:49:44-03:00",
          "tree_id": "591aa87629b41fcddcde65aa3b948fdb99492b2a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cab961fc0a5ba4f7124e8578bfb6522c393ad4d5"
        },
        "date": 1788447595266,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10919384,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8754230,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10919670,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2015141,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 763962,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 583695,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1058069,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2032898,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 569773,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18669,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790630,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25189,
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
          "id": "bdf8215932fceab9c8e12e9f3cece9c1f313f123",
          "message": "perf(handlers): shrink the inbound non-message stanza path; inbound_stanza bench (#1399)",
          "timestamp": "2026-09-03T11:47:18-03:00",
          "tree_id": "53e8b5b1d43dc11ca8c0d1fa775faa3b2da8427b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bdf8215932fceab9c8e12e9f3cece9c1f313f123"
        },
        "date": 1788447641721,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10914680,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8750966,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10915094,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2015392,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 765183,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 580429,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056864,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2032685,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 569773,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18669,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790451,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25172,
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
          "id": "af50b1c6e0a6744d3a03e58386a1faf1a685bf31",
          "message": "perf(binary): one-pass outbound marshal, single-pass message attribute resolution, one parked inflate state per thread (#1398)",
          "timestamp": "2026-09-03T11:46:29-03:00",
          "tree_id": "820d234dd251472071dcaef01c025d14ab79c786",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/af50b1c6e0a6744d3a03e58386a1faf1a685bf31"
        },
        "date": 1788447648028,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10907832,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8742838,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10907702,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2002830,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 768895,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 580429,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1056931,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2033300,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 569773,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18669,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 789920,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25148,
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
          "id": "e18d9b10b0d0108182f51c84b00a939828c4460e",
          "message": "perf(store): one-transaction app-state patch commit, cache-first msg-secret alternate JID, dead registry indexes, read pool default (#1401)",
          "timestamp": "2026-09-03T11:48:19-03:00",
          "tree_id": "947dd7db33418cfdfa4f8e411f185925bef3eec5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e18d9b10b0d0108182f51c84b00a939828c4460e"
        },
        "date": 1788447683618,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10919384,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8754230,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10919670,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2015141,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 764186,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 583695,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1058069,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2032674,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 569773,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18669,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790630,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25189,
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
            "email": "2136273+bryantebeek@users.noreply.github.com",
            "name": "Bryan te Beek",
            "username": "bryantebeek"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "00951ebeaf56a32493fb88f3482af18e6d139a09",
          "message": "feat(presence): let the host own the automatic available announcement (#1397)",
          "timestamp": "2026-09-03T12:26:46-03:00",
          "tree_id": "fbf14e6ab48d1d0c0e3fc0ca1ced8184c236000f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/00951ebeaf56a32493fb88f3482af18e6d139a09"
        },
        "date": 1788450003088,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10919864,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8754678,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10919694,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2015645,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 763795,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 583695,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1058085,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2033065,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 569773,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18669,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790856,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25201,
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
          "id": "8784f7f2d0ea3b91cfc7584232beffc444b9e227",
          "message": "perf(events)!: box the two variants that sized every dispatched event (#1402)",
          "timestamp": "2026-09-03T13:10:59-03:00",
          "tree_id": "949f8e19269ccde8751617d8239835b880832d3a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8784f7f2d0ea3b91cfc7584232beffc444b9e227"
        },
        "date": 1788452532076,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10918904,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8753782,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10919678,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2014757,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 764186,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195703,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 583695,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1058058,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2032674,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 569819,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18673,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 790899,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25205,
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
          "id": "197524bd0872f176916a1a98d5c60237c96e768c",
          "message": "perf(client): keep the group device memos warm at scale; batched registry reads; client_group_scale bench (#1400)",
          "timestamp": "2026-09-03T14:16:37-03:00",
          "tree_id": "46ff8f6f76b62315e7fbf9203e39e4f4621ab07b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/197524bd0872f176916a1a98d5c60237c96e768c"
        },
        "date": 1788456649868,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10940120,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8767990,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10936614,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2018389,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 763722,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195635,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 590481,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1061686,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2033269,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 570306,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18698,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 801122,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25574,
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
          "id": "30f13c9d91fd02e66646604b15c8d9a134149f52",
          "message": "perf(client): trim the connect and idle footprint: boxed post-login task, leaner per-client structure, batched presence re-subscribe (#1405)",
          "timestamp": "2026-09-03T14:17:46-03:00",
          "tree_id": "d5877866e2399cc08ea4b43f950aeec234a570d9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/30f13c9d91fd02e66646604b15c8d9a134149f52"
        },
        "date": 1788456668790,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10971480,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8791158,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10970298,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2031699,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 768311,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195635,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600129,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1062614,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2027972,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 573158,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18785,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 807582,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25902,
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
          "id": "961b230bc2aa9f3195232254a943290b94ddcba4",
          "message": "feat(send)!: SendResult carries the message the send encoded; edit, revoke and pin return one (#1406)",
          "timestamp": "2026-09-03T14:55:02-03:00",
          "tree_id": "094d2795eecbbeb2f76813114a386af5391734b6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/961b230bc2aa9f3195232254a943290b94ddcba4"
        },
        "date": 1788458630154,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10972248,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8791734,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10970466,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2013194,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 768311,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 77578,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195635,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29020,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600129,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1063231,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2046464,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 573011,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18785,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 807582,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 25902,
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
          "id": "b03a3defae85f44df2f877ce131e79a7b39fae90",
          "message": "perf(recv): stream large IQ responses, adopt owned frames, one inflate pool per thread (#1407)",
          "timestamp": "2026-09-03T15:33:46-03:00",
          "tree_id": "cf2fb0e292ff0a66b62314e57e614f899e538172",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b03a3defae85f44df2f877ce131e79a7b39fae90"
        },
        "date": 1788461279014,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11014968,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8831222,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11011798,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2033913,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769458,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86222,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195635,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600129,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1067512,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050874,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574324,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18808,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 812239,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26072,
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
          "id": "bb5aa3aa3f3881a4bb958aaa9b7daa66c6f863d7",
          "message": "perf(recv): one-pass streamed decode, one parser per streaming spec, stable contention benchmarks (#1409)",
          "timestamp": "2026-09-03T17:10:11-03:00",
          "tree_id": "6809b83f88627788a838012aa8fe1ca56541b0a9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bb5aa3aa3f3881a4bb958aaa9b7daa66c6f863d7"
        },
        "date": 1788466896896,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11009656,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8826038,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11007654,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2029192,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769765,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195635,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600129,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1067315,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050483,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574324,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18808,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 812328,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26075,
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
          "id": "2862b93965cf83ef50881c3ca897c036049a72f9",
          "message": "fix(conn): bound every teardown and retire what a connection leaves behind (#1410)",
          "timestamp": "2026-09-03T17:32:25-03:00",
          "tree_id": "33c857390c992530dedf9426208c32e3cf9f664c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2862b93965cf83ef50881c3ca897c036049a72f9"
        },
        "date": 1788468147383,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11041976,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8855414,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11041006,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2055534,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769765,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195635,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600129,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070091,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050488,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574324,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18808,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 822319,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26384,
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
          "id": "fa36dc61ef06b2d3d919f8a53051bb061da04f95",
          "message": "perf(memory): sweep expired cache entries on the maintenance tick, release online device-sync dedup, report LID/PN side maps and chat-lane backlog (#1408)",
          "timestamp": "2026-09-03T17:31:23-03:00",
          "tree_id": "e6046a4734bfe0f0a627aba2407e18e263b03c93",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fa36dc61ef06b2d3d919f8a53051bb061da04f95"
        },
        "date": 1788468156707,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11032024,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8846582,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11028350,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2048528,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769989,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195635,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 600129,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1068350,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050259,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574324,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18808,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 819922,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26302,
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
          "id": "b1ef5d9d5aec8f4e480fa61f0732fd4a9712f139",
          "message": "perf(store): make the persistence layer survive a month-long session (#1411)",
          "timestamp": "2026-09-03T18:10:24-03:00",
          "tree_id": "3a080c24af9a74f71b378a9bb29af8c831e39c4b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b1ef5d9d5aec8f4e480fa61f0732fd4a9712f139"
        },
        "date": 1788470604589,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 11059064,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8866294,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 11058542,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2064399,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769144,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86054,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195635,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1878470,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 608976,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 13058,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070144,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044255,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574592,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18816,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 822901,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26392,
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
          "id": "f7b248c57fddd79cc1eedd23752ab478b5705fa6",
          "message": "build(deps): bump buffa from 0.9.1 to 0.9.2 (#1412)",
          "timestamp": "2026-09-03T20:41:51-03:00",
          "tree_id": "a1bf02537ecc2439ea73499669aad415e54c30a6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f7b248c57fddd79cc1eedd23752ab478b5705fa6"
        },
        "date": 1788479568950,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10952664,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8810166,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10951050,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2058690,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 768970,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070346,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049570,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574553,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18815,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 822901,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26392,
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
          "id": "486fc3d4a75d128a9d470dfd19cb02a68d40d004",
          "message": "perf(appstate): keep the sync engine out of the server_sync task layout (#1413)",
          "timestamp": "2026-09-03T20:46:57-03:00",
          "tree_id": "409bf7fac2a82c054f2156555e14e24e0056008b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/486fc3d4a75d128a9d470dfd19cb02a68d40d004"
        },
        "date": 1788479755924,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10953368,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8810678,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10951130,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2059064,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 768970,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070450,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049570,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574553,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18815,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 823031,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26397,
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
            "email": "117304815+codspeed-hq[bot]@users.noreply.github.com",
            "name": "codspeed-hq[bot]",
            "username": "codspeed-hq[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "790809a50ccce33c4c56263fc276ded68bd7b40f",
          "message": "chore: Benchmark determinism (#1416)",
          "timestamp": "2026-09-03T22:08:45-03:00",
          "tree_id": "34c290303b7354268fc7f087a9aa6d2d5e3fb446",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/790809a50ccce33c4c56263fc276ded68bd7b40f"
        },
        "date": 1788485127115,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10953176,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8810486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10951106,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2058915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 768970,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070423,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049570,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574599,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18819,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 823082,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26401,
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
          "id": "d8d319cd36bcb8832a1c014c5c99aa70eef2817a",
          "message": "perf(events)!: box LoggedOut and TemporaryBan payloads (#1417)",
          "timestamp": "2026-09-03T22:08:12-03:00",
          "tree_id": "712d64b720cf7d2d39dc99cdf5464a5a904ba225",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d8d319cd36bcb8832a1c014c5c99aa70eef2817a"
        },
        "date": 1788485140658,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10953176,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8810486,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10951106,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2058915,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769194,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070423,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049346,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574599,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18819,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 823082,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26401,
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
          "id": "5b837f72605d264ac454f052c738ac561aaf1751",
          "message": "perf(media): single-flight concurrent refresh_media_conn fetches (#1415)",
          "timestamp": "2026-09-03T22:34:33-03:00",
          "tree_id": "3fb69bd69ac45c1b64d0efaa705c4e9de6153f95",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5b837f72605d264ac454f052c738ac561aaf1751"
        },
        "date": 1788486381684,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10960056,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8816758,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10959402,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2070086,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 768970,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070530,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044493,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574599,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18819,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 824855,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26468,
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
          "id": "38f23fdc9b60994e6aef799dd3d8ac96f2a659d0",
          "message": "perf(signal): pre-size flush batches to their exact upper bound (#1418)",
          "timestamp": "2026-09-03T22:36:16-03:00",
          "tree_id": "69389ccbebc7945fe4deafa21439d09deff6b35a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/38f23fdc9b60994e6aef799dd3d8ac96f2a659d0"
        },
        "date": 1788486403406,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10960440,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8817142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10959402,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2065281,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769118,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070530,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049570,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574599,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18819,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 824896,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26472,
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
          "id": "66589e0d72e44e74424711e6d4664210297feb1c",
          "message": "bench: cover the hot paths without a number (#1419)",
          "timestamp": "2026-09-04T00:39:10-03:00",
          "tree_id": "758c681d43db7a1db33b0c2cdedf6748fdd6675a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/66589e0d72e44e74424711e6d4664210297feb1c"
        },
        "date": 1788494014301,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10960440,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8817142,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10959402,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2070358,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769342,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21427,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609601,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_tokio_transport",
            "value": 41540,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_ureq_http_client",
            "value": 12983,
            "unit": "bytes"
          },
          {
            "name": ".text std",
            "value": 1070530,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044269,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574599,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18819,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 824896,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26472,
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
          "id": "c90e1ba1ef4462d15f6a14fe6e40e2f66d35a101",
          "message": "perf(feed): hand transports' large reads to the decoder as owned Bytes (#1420)",
          "timestamp": "2026-09-04T01:49:24-03:00",
          "tree_id": "a49cbd6a412013dd9a68f0f6664973e01e663d79",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c90e1ba1ef4462d15f6a14fe6e40e2f66d35a101"
        },
        "date": 1788498295782,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10960120,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8816694,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10959386,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2064276,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769118,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609601,
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
            "value": 1070530,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049570,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574599,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18819,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 824891,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26472,
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
          "id": "54a79b883ee0db9e0b03be86c215d58c0001ae92",
          "message": "perf(sqlite): route store_prekeys_batch through the shared single-transaction write path (#1421)",
          "timestamp": "2026-09-04T01:51:10-03:00",
          "tree_id": "e249bd9734defb91dcecd78342a487f1b6cec056",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/54a79b883ee0db9e0b03be86c215d58c0001ae92"
        },
        "date": 1788498333426,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10958744,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8815926,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10959386,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2064276,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 769342,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 609197,
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
            "value": 1070171,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049346,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 574599,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18819,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 824891,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26472,
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
          "id": "3de35ba672f40ce898342d91d77a3dca9e0f100f",
          "message": "perf(send): memoize per-device addresses and batch session loads (#1422)",
          "timestamp": "2026-09-04T02:02:54-03:00",
          "tree_id": "93ae03450b96778fb6ef3cfcd2c1b868bf7b2835",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3de35ba672f40ce898342d91d77a3dca9e0f100f"
        },
        "date": 1788499995363,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10983416,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8836598,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10985070,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2068960,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 774348,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619605,
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
            "value": 1070484,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049570,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575207,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18845,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 827404,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26568,
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
          "id": "0aa87c64ffd07fb288a7db8df5c46c30e92ff7fa",
          "message": "bench(cache): reuse identical key batches every sample (#1423)",
          "timestamp": "2026-09-04T02:08:00-03:00",
          "tree_id": "084a769d3dbd888682f49c71ad031e882f854114",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0aa87c64ffd07fb288a7db8df5c46c30e92ff7fa"
        },
        "date": 1788500002020,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10983416,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8836598,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10985070,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2068960,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 773957,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 619605,
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
            "value": 1070484,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049961,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575207,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18845,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 827404,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26568,
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
          "id": "7ee5ebf0044acf50d6dd4f8ef0e9c08bd0eb0712",
          "message": "perf(sqlite): reuse prepared statements for hot Signal upserts (#1426)",
          "timestamp": "2026-09-04T21:01:06-03:00",
          "tree_id": "d6300b53c37d3d975908bdd92321ed8e1bdd1b63",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7ee5ebf0044acf50d6dd4f8ef0e9c08bd0eb0712"
        },
        "date": 1788568319117,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10961464,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8818998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10960086,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2074037,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 773957,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1070344,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044884,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575207,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18845,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 827404,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26568,
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
          "id": "91b905ab7333021da9ce9d862e0600ad0808c38d",
          "message": "bench(signal): cover flush contention and durable cache batches (#1427)",
          "timestamp": "2026-09-04T21:01:37-03:00",
          "tree_id": "c5d05b713c1c8c184178cc4dced81e785c2e5821",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/91b905ab7333021da9ce9d862e0600ad0808c38d"
        },
        "date": 1788568603476,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10961464,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8818998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10960086,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2068960,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 774572,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1070344,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049346,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575207,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18845,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 827404,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26568,
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
          "id": "bf0fc768bd1d241bc19db543bfafd9bd8f848818",
          "message": "bench(recv): cover runtime bursts and production chat lanes (#1428)",
          "timestamp": "2026-09-04T21:02:05-03:00",
          "tree_id": "aedfe1b70c9d9c8a4c0681a0506a72ec17cf13f0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bf0fc768bd1d241bc19db543bfafd9bd8f848818"
        },
        "date": 1788568667764,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10961464,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8818998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10960086,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2068960,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 773957,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1070344,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049961,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575207,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18845,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 827404,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26568,
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
          "id": "48eb7794f2c7e5e0ccc50d0d153a57d003e4a95b",
          "message": "test(store): document Signal durability and recovery contract (#1431)",
          "timestamp": "2026-09-04T21:41:41-03:00",
          "tree_id": "76b2db9fc0c330ce37d27986063d59a39d66441c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/48eb7794f2c7e5e0ccc50d0d153a57d003e4a95b"
        },
        "date": 1788570158150,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10961464,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8818998,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10960086,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2068960,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 774348,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1070344,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049570,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575305,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18850,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 827404,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26568,
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
          "id": "ccafa1323e50e86396c5b68f4a2f9b8f37cea11c",
          "message": "fix(iq): preserve unclassified error origins (#1434)",
          "timestamp": "2026-09-04T21:42:16-03:00",
          "tree_id": "9bd22ed979d854d17fe79d6c18b6acf131e3932b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ccafa1323e50e86396c5b68f4a2f9b8f37cea11c"
        },
        "date": 1788570161764,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10963192,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8820150,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10964430,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2073770,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 775672,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1070621,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044269,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575305,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18850,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 827702,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26577,
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
          "id": "65b2e1e2e580f8e1a1d7dc74bfa0d453b78ac6e9",
          "message": "feat(events): report channel delivery outcomes (#1438)",
          "timestamp": "2026-09-04T22:15:51-03:00",
          "tree_id": "4456b220c42e920340a4c3e01a0637ea589d73af",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/65b2e1e2e580f8e1a1d7dc74bfa0d453b78ac6e9"
        },
        "date": 1788571790154,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10963192,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8820150,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10964430,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2068594,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 775156,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1070621,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2049961,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575811,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18861,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 827227,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26558,
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
          "id": "cddcf0e48862d32618d2c19fe08646262102fdd0",
          "message": "feat(client): expose supervision completion reasons (#1430)",
          "timestamp": "2026-09-04T22:29:46-03:00",
          "tree_id": "e4e83f5d5c14b9e7ab1e00bdc33aaf9e9dac7dc6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cddcf0e48862d32618d2c19fe08646262102fdd0"
        },
        "date": 1788572522707,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10969304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8825974,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10968542,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2072546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 775547,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1071709,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050325,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575811,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18861,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 828138,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26574,
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
          "id": "be351687870a9b6eb10c4052c4ccb8e310a053c5",
          "message": "refactor(protocol): clarify child parsing ownership (#1436)",
          "timestamp": "2026-09-04T23:15:41-03:00",
          "tree_id": "16500ce8adace5f1d609dd58c9711a840b779b30",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/be351687870a9b6eb10c4052c4ccb8e310a053c5"
        },
        "date": 1788575220041,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10969304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8825974,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10968542,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2072546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 775547,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1071709,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050325,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575811,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18861,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 828138,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26574,
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
          "id": "30fe2fd05c5b181474bd57b3ab311dd9d3962d1c",
          "message": "fix(observability): report event handlers and check nested state (#1433)",
          "timestamp": "2026-09-04T23:16:25-03:00",
          "tree_id": "f0cc05bac0b6825f90529f7578fb71eb581e73f4",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/30fe2fd05c5b181474bd57b3ab311dd9d3962d1c"
        },
        "date": 1788575254915,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10969304,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8825974,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10968542,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2072546,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 775156,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_binary",
            "value": 86150,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_libsignal",
            "value": 195676,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_appstate",
            "value": 29017,
            "unit": "bytes"
          },
          {
            "name": ".text wacore_noise",
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 602155,
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
            "value": 1071709,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2050716,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575836,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18862,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 828169,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26574,
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
          "id": "a5671942b3ed23a585e5bd9f75ab67b22e7ae460",
          "message": "fix(voip): preserve video capture timestamps across drops (#1437)",
          "timestamp": "2026-09-04T23:17:11-03:00",
          "tree_id": "eeae0c03207623ce6467b0f0d1e25a25f35d9af8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a5671942b3ed23a585e5bd9f75ab67b22e7ae460"
        },
        "date": 1788575672987,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10967256,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8824182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10968426,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2078792,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 775347,
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
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 599766,
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
            "value": 1071882,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044627,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575836,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18862,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 828169,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26574,
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
          "id": "d06986894ad71062a639ab333d52eef990ae7e4d",
          "message": "fix(sqlite): use portable retry backoff timers (#1435)",
          "timestamp": "2026-09-04T23:16:53-03:00",
          "tree_id": "6adbd049aeef8d3d819348ed38c07934436f1d53",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d06986894ad71062a639ab333d52eef990ae7e4d"
        },
        "date": 1788575678148,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10967256,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8824182,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10968426,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2078792,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 775347,
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
            "value": 21991,
            "unit": "bytes"
          },
          {
            "name": ".text waproto",
            "value": 1821983,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust_sqlite_storage",
            "value": 599766,
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
            "value": 1071882,
            "unit": "bytes"
          },
          {
            "name": ".text other deps",
            "value": 2044627,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575836,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18862,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 828169,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26574,
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
          "id": "0b37b2b47bb578795ad91619a2da36ee406fe635",
          "message": "feat(handshake)!: runtime Noise cert policy with cache provenance (#1441)",
          "timestamp": "2026-09-05T13:26:26-03:00",
          "tree_id": "10df5249f3ebb3c6a09982a36c29a329cbe22aa9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0b37b2b47bb578795ad91619a2da36ee406fe635"
        },
        "date": 1788626553933,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "bin size (stripped)",
            "value": 10968152,
            "unit": "bytes"
          },
          {
            "name": "bin .text",
            "value": 8824950,
            "unit": "bytes"
          },
          {
            "name": "bin allocated (text+data+bss)",
            "value": 10968410,
            "unit": "bytes"
          },
          {
            "name": ".text whatsapp_rust",
            "value": 2073338,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 775564,
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
            "value": 22739,
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
            "value": 2049480,
            "unit": "bytes"
          },
          {
            "name": "llvm-lines wacore",
            "value": 575836,
            "unit": "lines"
          },
          {
            "name": "llvm-lines wacore copies",
            "value": 18861,
            "unit": "copies"
          },
          {
            "name": "llvm-lines whatsapp-rust lib",
            "value": 828213,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26574,
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
          "id": "60b6163962d84ed5575a667a437faa2420340467",
          "message": "fix(adv): correct hosted pairing and device signature validation (#1440)",
          "timestamp": "2026-09-05T13:34:45-03:00",
          "tree_id": "11baf1384176d51bfb2a4649956af8fa2dd5ad42",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/60b6163962d84ed5575a667a437faa2420340467"
        },
        "date": 1788626649163,
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
            "value": 2073379,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 774795,
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
            "value": 22739,
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
            "value": 2049480,
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
            "value": 828282,
            "unit": "lines"
          },
          {
            "name": "llvm-lines whatsapp-rust lib copies",
            "value": 26578,
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
          "id": "7399711b0560c7847f32a59cfcc68a8ee1567478",
          "message": "fix(handshake): preserve strict verification across public builders and helpers (#1442)",
          "timestamp": "2026-09-05T13:51:33-03:00",
          "tree_id": "9e0358402547465e4275f02ab4d34e3d6dfddbfd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7399711b0560c7847f32a59cfcc68a8ee1567478"
        },
        "date": 1788628084895,
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
            "value": 2078481,
            "unit": "bytes"
          },
          {
            "name": ".text wacore",
            "value": 774962,
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
            "value": 2044220,
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
          "id": "41e81a00255b50b277c8efee4b2b7200a4345e7e",
          "message": "fix(handshake): clarify noise policy docs and drop duplicate test rationale (#1443)",
          "timestamp": "2026-09-05T14:17:02-03:00",
          "tree_id": "a783df9bbf0c9b9753430d31163286ae89d2cb20",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/41e81a00255b50b277c8efee4b2b7200a4345e7e"
        },
        "date": 1788629331234,
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
      }
    ]
  }
}