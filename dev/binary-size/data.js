window.BENCHMARK_DATA = {
  "lastUpdate": 1781289591819,
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
      }
    ]
  }
}