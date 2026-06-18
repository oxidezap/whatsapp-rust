window.BENCHMARK_DATA = {
  "lastUpdate": 1781784094729,
  "repoUrl": "https://github.com/oxidezap/whatsapp-rust",
  "entries": {
    "whatsapp-rust integration benchmarks": [
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6864a48858ebb2f532128db5359a3f15f780d5a4",
          "message": "perf(retry): peek the cached message on resend instead of take + re-add (#779)",
          "timestamp": "2026-06-08T14:29:48-03:00",
          "tree_id": "04fe5343cf669a6acf3bb7a166965aab2d58d3f1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6864a48858ebb2f532128db5359a3f15f780d5a4"
        },
        "date": 1780940036377,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8150,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2786103,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 195,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37203,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 311,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 61624,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 495,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 88612,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 79483,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 834,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215837,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "16dca20a8fa2b91c9b652d456b11739d569258bf",
          "message": "feat(receipt): expose the 'offline' attr on the Receipt event (#780)",
          "timestamp": "2026-06-08T14:34:20-03:00",
          "tree_id": "1ec6d305f012440a60d368b65ad940513bc847df",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/16dca20a8fa2b91c9b652d456b11739d569258bf"
        },
        "date": 1780940319108,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8359,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2818007,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 262,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 49854,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 286,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56964,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 303,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 60374,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 359,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78008,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1115,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 275440,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7eebdf37e8a24c108059afb27d1d4d23ba5630e6",
          "message": "feat(send): emit member_label meta attrs (appdata + tag_reason) (#781)",
          "timestamp": "2026-06-08T14:49:46-03:00",
          "tree_id": "a1f77eafa6015e0d0f47e4ba58896e0c343d63c2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7eebdf37e8a24c108059afb27d1d4d23ba5630e6"
        },
        "date": 1780941214392,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8349,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2823912,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 225,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43239,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 319,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 63937,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 499,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 90183,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 79481,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 835,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215938,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "90993f3e09216ab23eac1d3bc4c24d7dc1314439",
          "message": "fix(noise): error on frame counter exhaustion instead of wrapping (#782)",
          "timestamp": "2026-06-08T15:10:49-03:00",
          "tree_id": "c2f4b3ab1b79c587d4cc5ce610398772f4b004a6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/90993f3e09216ab23eac1d3bc4c24d7dc1314439"
        },
        "date": 1780942491484,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8155,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2843442,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 201,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37233,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 241,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 48067,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 497,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 89568,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78449,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1114,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 275387,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9b88b048005df5bdf3321ff3eaa2deb99e0570d8",
          "message": "fix(iq): pong server pings with an absent type, not only type=get (#783)",
          "timestamp": "2026-06-08T15:13:16-03:00",
          "tree_id": "05cab5dadbd54626b9594b262b6f08413e54f62d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9b88b048005df5bdf3321ff3eaa2deb99e0570d8"
        },
        "date": 1780942645538,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8207,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2841753,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 274,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 51522,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 291,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 59605,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 390,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 72612,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78867,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1045,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 259523,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ee1098792435a39f3966e202418a4e9180f9c931",
          "message": "refactor(framing): drop dead, would-desync oversize check in decode_frame (#784)",
          "timestamp": "2026-06-08T15:14:43-03:00",
          "tree_id": "37fe1becfc48b190c4d11d6fe7865d3f9f92b914",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ee1098792435a39f3966e202418a4e9180f9c931"
        },
        "date": 1780942757287,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8373,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2820174,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 184,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35755,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 296,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 58749,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 277,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 58167,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78669,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1044,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 263491,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7f1eaa24c9c16318ce6fe266ea3bdac9bd58c34c",
          "message": "fix(conn): log benign server recycles quietly without hiding real errors (#785)",
          "timestamp": "2026-06-08T16:09:22-03:00",
          "tree_id": "ab9787a5431bd749540e64eb459345b7fedb8ffb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7f1eaa24c9c16318ce6fe266ea3bdac9bd58c34c"
        },
        "date": 1780946043942,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8268,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2799684,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 188,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 38074,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 285,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57284,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 289,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 56829,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78841,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 835,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215894,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fd70f89e7b097e9d8ffc4b4ed000a9d0a2e7b390",
          "message": "perf(appstate): move snapshot mutations into the accumulator instead of extend (#786)",
          "timestamp": "2026-06-08T16:42:26-03:00",
          "tree_id": "0df1fff29d9725b32ce6488a6934d687e4d0fc41",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fd70f89e7b097e9d8ffc4b4ed000a9d0a2e7b390"
        },
        "date": 1780948021226,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8361,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2763653,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 235,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43605,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 298,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 59963,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 505,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 89623,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 77803,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1066,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 257291,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "177b9c615d5abe9712b6e9d7d9b77fb31c09c074",
          "message": "perf(send): encode DM content once, splice into recipient + DSM plaintexts [PoC] (#787)",
          "timestamp": "2026-06-08T18:06:59-03:00",
          "tree_id": "5b16fa9735c2b81a55e3720f88ece9a30d0cfaec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/177b9c615d5abe9712b6e9d7d9b77fb31c09c074"
        },
        "date": 1780953089876,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8214,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2736931,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 201,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 30812,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 246,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 42290,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 510,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 84919,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 357,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72361,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 833,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215780,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "440532189244415e4f34459cad3e7fd06fd6685f",
          "message": "perf(send): splice reporting context onto plaintexts, drop per-send Message clone (#788)",
          "timestamp": "2026-06-08T19:43:27-03:00",
          "tree_id": "ef2e29c5e47530d62f726983c50d2a1adc87fa20",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/440532189244415e4f34459cad3e7fd06fd6685f"
        },
        "date": 1780958863431,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8206,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2779242,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 209,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35041,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 286,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 52522,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 295,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 52590,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71667,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 861,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 216612,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "acc87142f0d25ef8bd26f00ca1efcf0f63346bb8",
          "message": "fix(adv): fall back to stored account identity for ADV account_signature_key (#790)",
          "timestamp": "2026-06-08T23:47:26-03:00",
          "tree_id": "6da1d2357f725979d10657210ee8a611f862c3c3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/acc87142f0d25ef8bd26f00ca1efcf0f63346bb8"
        },
        "date": 1780973533436,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8561,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2833839,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 186,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 29398,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 291,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 53015,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 500,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 83693,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 357,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72017,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 834,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 206268,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "62cfa82f9427403fcef65d218fc9a9cf7fb0626f",
          "message": "chore(session): remove dead SessionManager (zero production callers) (#791)",
          "timestamp": "2026-06-09T09:57:19-03:00",
          "tree_id": "10fcb50a24714b7509c05e2c614ce6ce2dc05484",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/62cfa82f9427403fcef65d218fc9a9cf7fb0626f"
        },
        "date": 1781010096927,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8299,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2745030,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 385,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 190,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 30188,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 313,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57016,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 279,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 52382,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71551,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1007,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 246337,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "582a4ee3a78937d92a7cc536ca8f6763537fbbf9",
          "message": "fix(wasm): relax EncHandler Send+Sync via MaybeSendSync, gate async_trait (#793)",
          "timestamp": "2026-06-09T09:57:48-03:00",
          "tree_id": "620ea2125cf0ce882352c66bf7382e681544ec3a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/582a4ee3a78937d92a7cc536ca8f6763537fbbf9"
        },
        "date": 1781010123221,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8207,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2823902,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 176,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 29698,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 259,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 44999,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 496,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 82892,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 355,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71375,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1083,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 255125,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b61b1df373db087083400a809929df75fa25c48b",
          "message": "perf(receive): make custom enc handlers an immutable set-once snapshot (#792)",
          "timestamp": "2026-06-09T10:09:32-03:00",
          "tree_id": "540471268c905365f91b73756f9524816e4d8073",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b61b1df373db087083400a809929df75fa25c48b"
        },
        "date": 1781010801195,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8319,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2748536,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 267,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42917,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 303,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55085,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 414,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 70186,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71950,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 835,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 208349,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "74115002e66ad1a202c0b54b3b751dfe0f0ec442",
          "message": "api: mark lib-constructed response/result structs #[non_exhaustive] for 1.0 (#794)",
          "timestamp": "2026-06-09T10:40:36-03:00",
          "tree_id": "587b3cd81cdcbb2d77a77f8cfcd7c193dc4844b7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/74115002e66ad1a202c0b54b3b751dfe0f0ec442"
        },
        "date": 1781012744432,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8189,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2816178,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 338,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 188,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 30499,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 290,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 51799,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 282,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 52644,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 357,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 70664,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 865,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 216935,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "208d04f7328c53be4fbfc8ff811904ae31ae6004",
          "message": "fix(wasm): relax networking traits Send+Sync via MaybeSendSync (#795)",
          "timestamp": "2026-06-09T10:40:26-03:00",
          "tree_id": "e302ff8c494551143356055b81e4c61b5c8ea144",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/208d04f7328c53be4fbfc8ff811904ae31ae6004"
        },
        "date": 1781012759713,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8281,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2839730,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 218,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36708,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 305,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55666,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 489,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 83219,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 355,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71153,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1001,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 239254,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "99c3ff9f2d325d10a2feed23011b1d5da7295ffb",
          "message": "refactor(handlers): split notification.rs god-file by domain (#796)",
          "timestamp": "2026-06-09T11:12:31-03:00",
          "tree_id": "77f36969e5110f9ccbf6143ed0cb1548fc0c91a7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/99c3ff9f2d325d10a2feed23011b1d5da7295ffb"
        },
        "date": 1781014574434,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8214,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2816695,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 341,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 183,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 29044,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 322,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 58636,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 556,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 231102,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71798,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1068,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 253914,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 6,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "de5b9431b9b85e188293b7c5e0aaa8144994fbc7",
          "message": "fix(device-list): always keep the primary device after a raw_id mismatch patch (#797)",
          "timestamp": "2026-06-09T12:19:04-03:00",
          "tree_id": "bc9c557f4e28a3f17d55410e385e50e5346218be",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/de5b9431b9b85e188293b7c5e0aaa8144994fbc7"
        },
        "date": 1781018761894,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8360,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2807598,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 223,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37919,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 237,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 40356,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 502,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 83735,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 357,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72006,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1015,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 241151,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "323a6e646314ced0f7cd3e775c9d81f21285183d",
          "message": "fix(retry): recover from unknown-device retries that carry a key bundle (#798)",
          "timestamp": "2026-06-09T12:32:26-03:00",
          "tree_id": "8384f351ded010c9a437a6f5076d6b5bc68259e9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/323a6e646314ced0f7cd3e775c9d81f21285183d"
        },
        "date": 1781019431186,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8304,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2851474,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 228,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36925,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 239,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 41678,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 490,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 83255,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 355,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71208,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 834,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 208294,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5e49575841798d773069fdfd2b3738208b6a7c7d",
          "message": "chore(send): log benign prekey-fetch skips at debug instead of warn (#800)\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-06-09T12:36:57-03:00",
          "tree_id": "14336d4b9637e747e10c76b73b3707a339469200",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5e49575841798d773069fdfd2b3738208b6a7c7d"
        },
        "date": 1781019692414,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8325,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2844025,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 192,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 32789,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 251,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 44181,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 369,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 65397,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 355,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71263,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1040,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 245463,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3cfe78c95b640a284b1bf947e373a29f19c1cc3b",
          "message": "fix(device-list): never drop the primary on a device-remove patch (#801)",
          "timestamp": "2026-06-09T12:47:41-03:00",
          "tree_id": "0342b2548bc836ab28261ca1c00e811307601dbf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3cfe78c95b640a284b1bf947e373a29f19c1cc3b"
        },
        "date": 1781020303181,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8215,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2825213,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 185,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 31160,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 283,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50784,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 492,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 83428,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 356,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71311,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1000,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 237372,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8386ca5c2b1a4813f15d07c29b8fd479b77e6024",
          "message": "fix(usync): refetch an empty device record instead of trusting it (#799)\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-06-09T13:00:57-03:00",
          "tree_id": "415b056822f0b8e2bfcf82d519f0ae6d5d9a5753",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8386ca5c2b1a4813f15d07c29b8fd479b77e6024"
        },
        "date": 1781021088690,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8225,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2833262,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 240,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40080,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 284,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50604,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 439,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 74908,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72510,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 853,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210866,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "20189cb0a9249d6126726d4c45b67ee03ecf19a3",
          "message": "fix(retry): resync the device list on a retry from an unknown device (#802)",
          "timestamp": "2026-06-09T14:30:54-03:00",
          "tree_id": "267aa3304f07276e50b01c09d4ca38e8d54ba3f9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/20189cb0a9249d6126726d4c45b67ee03ecf19a3"
        },
        "date": 1781026448679,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8166,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2813505,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 335,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 223,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36989,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 308,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56450,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 487,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 220915,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71784,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1070,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 254030,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 6,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "45be6259c829799f6f1a576f4c7a9c867d068b53",
          "message": "perf(receive): adopt inbound payloads zero-copy in FrameDecoder (#803)",
          "timestamp": "2026-06-09T15:44:40-03:00",
          "tree_id": "4e208195348eaa38c2fb0b3ace2d49d8b6ac243d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/45be6259c829799f6f1a576f4c7a9c867d068b53"
        },
        "date": 1781030933002,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8155,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2785365,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 335,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 265,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 39959,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 287,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 52217,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 484,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 82797,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 355,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71385,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1069,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 254012,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 6,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d445cad73589d9726e4d7cbbb358bff934ed86a0",
          "message": "perf(receive): resolve the noise socket once per read loop, drop ack re-encode copy (#804)",
          "timestamp": "2026-06-09T15:45:37-03:00",
          "tree_id": "6f3bf0dced0cd559c8d697781736b1a7d368e3b0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d445cad73589d9726e4d7cbbb358bff934ed86a0"
        },
        "date": 1781031015394,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8295,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2811517,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 196,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 30468,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 183,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 30597,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 304,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 54544,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 372,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 74342,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1002,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 239472,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "89f5487135aa6dba9f84b3b3ad2ca2f45eaa1487",
          "message": "perf(send): establish sessions before taking the sender-key chain lock (#807)",
          "timestamp": "2026-06-09T16:13:20-03:00",
          "tree_id": "2101331382753eec729d4013bf45fc5569855865",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/89f5487135aa6dba9f84b3b3ad2ca2f45eaa1487"
        },
        "date": 1781032671510,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8157,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2743126,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 231,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35815,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 310,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56690,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 280,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 52429,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 359,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71938,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 860,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 214387,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "62ca9b973094ac4d25415801fe06c13fc25ea5e2",
          "message": "perf(signal-cache): share cached sessions via Arc, peek without deep clone (#809)",
          "timestamp": "2026-06-09T17:22:30-03:00",
          "tree_id": "0226c71567fc4a1de79826754ce630a92e86e0d7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/62ca9b973094ac4d25415801fe06c13fc25ea5e2"
        },
        "date": 1781036811279,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8234,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2804436,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 232,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36862,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 309,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57386,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 400,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 69710,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72181,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 862,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 216553,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2b380e800c2b629ac35f9fb45afc983104934d35",
          "message": "perf(store): cache the device snapshot as Arc&lt;Device&gt; (#808)",
          "timestamp": "2026-06-09T17:24:34-03:00",
          "tree_id": "0f3fda228e08660212e28c12e3c1175fb51e71ec",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2b380e800c2b629ac35f9fb45afc983104934d35"
        },
        "date": 1781036917521,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8283,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2705154,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 256,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 44937,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 211,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 40186,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 424,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 75487,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 335,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 70054,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1011,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 237708,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "68cd7ae7928e4af908b86a3efbff47020eaf2668",
          "message": "perf(group): derive the PN-to-LID reverse index, stop persisting it (#810)",
          "timestamp": "2026-06-09T17:36:23-03:00",
          "tree_id": "b0fe70c29437994c8fa283dade53b4cb22094539",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/68cd7ae7928e4af908b86a3efbff47020eaf2668"
        },
        "date": 1781037668310,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8156,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2795435,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 158,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 29707,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 263,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 51470,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 431,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 77713,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 333,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 69729,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 781,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 196133,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a189af6f61f4c53644b2a10c220bdf6e6ffcce56",
          "message": "perf(lid-pn): share identifier strings between cache keys and entries (#811)",
          "timestamp": "2026-06-09T17:43:29-03:00",
          "tree_id": "eddb728ed09ba883dc429fc570d879eb8f1bb866",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a189af6f61f4c53644b2a10c220bdf6e6ffcce56"
        },
        "date": 1781038089291,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8270,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2762418,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 178,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35281,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 260,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 49827,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 371,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 66062,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 333,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 70385,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1023,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 240526,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "87894660a068e1af72da71dd941420b7f1f87b3b",
          "message": "fix(protocol): tighten error handling gaps (#817)",
          "timestamp": "2026-06-09T20:20:11-03:00",
          "tree_id": "cdcfa8558052036f714f22e591da0002a3e17d27",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/87894660a068e1af72da71dd941420b7f1f87b3b"
        },
        "date": 1781047502147,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8118,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2835546,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 187,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 34063,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 282,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55791,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 352,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 65323,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 331,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 69514,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 812,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 204535,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fb4657cd269710bcc7b05a82ab56b480084a1d34",
          "message": "perf(appstate): move snapshot and patches into the blocking handoff instead of deep-cloning (#818)",
          "timestamp": "2026-06-09T20:57:33-03:00",
          "tree_id": "89cb2fd18a73ee87e306270c75f43a15eeba7f7e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fb4657cd269710bcc7b05a82ab56b480084a1d34"
        },
        "date": 1781049729052,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8138,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2784278,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 191,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35174,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 243,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 48718,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 265,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 52524,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 334,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 69809,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 784,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 198145,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
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
          "id": "cb87c199c1a1c2c8557ef2add65e24a1f0407763",
          "message": "chore(deps): bump chrono from 0.4.44 to 0.4.45 (#813)",
          "timestamp": "2026-06-09T20:57:52-03:00",
          "tree_id": "bacbbf32c8971d09fa25813861e0459f7e6acb76",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cb87c199c1a1c2c8557ef2add65e24a1f0407763"
        },
        "date": 1781049763501,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8262,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2821598,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 233,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41568,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 216,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 41466,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 346,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 64716,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 332,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 69458,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 956,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 230227,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
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
          "id": "b668ef57887b52d5ac0e02b0dcdd0cc02d426888",
          "message": "chore(deps): bump http from 1.4.1 to 1.4.2 (#814)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-06-09T20:57:59-03:00",
          "tree_id": "f8d08a2fca62ca966fbcc519ac3e3918f03725a9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b668ef57887b52d5ac0e02b0dcdd0cc02d426888"
        },
        "date": 1781049776004,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8228,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2742543,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 340,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 214,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43361,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 279,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 54336,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 66880,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 331,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 69438,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1021,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 244554,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
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
          "id": "67f22379cbdb60ac068f7166d8869b4827f4662f",
          "message": "chore(deps): bump diesel from 2.3.9 to 2.3.10 (#816)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-06-09T20:58:34-03:00",
          "tree_id": "a782ab9fb18b271089676fd91733a7b22f26580b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/67f22379cbdb60ac068f7166d8869b4827f4662f"
        },
        "date": 1781049901111,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8259,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2754780,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 198,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36255,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 228,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 43744,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 348,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 65140,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 333,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 69575,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 784,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 197878,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 84,
            "unit": "milliseconds"
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
          "id": "9530745a3fa1b08f28f5c6ae81043753707b31cc",
          "message": "chore(deps): bump prost from 0.14.3 to 0.14.4 (#815)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-06-09T20:58:19-03:00",
          "tree_id": "c45e12af8c40fd31ad65e096fcebb9505d27da3c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9530745a3fa1b08f28f5c6ae81043753707b31cc"
        },
        "date": 1781049910647,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8119,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2730023,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 272,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47284,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 226,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 43044,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 265,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 53451,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 334,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 70117,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 958,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 230308,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b65fd0d9a8ae0b839461530f2fa9e14d8de700b7",
          "message": "perf(binary): store small attribute lists inline, dropping the per-node heap allocation (#819)",
          "timestamp": "2026-06-09T21:57:55-03:00",
          "tree_id": "1cc04e1394565dda1292ded86cda9d27ad91b1b8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b65fd0d9a8ae0b839461530f2fa9e14d8de700b7"
        },
        "date": 1781053303583,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8204,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2751854,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 335,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 246,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43501,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 244,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50326,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 463,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 84111,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 325,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 69801,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1012,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 249629,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 6,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1162b1344a2eb1507ae4d40b39b346e9ad41d5fb",
          "message": "perf(send): hash the participant list from one arena instead of a String per device (#822)",
          "timestamp": "2026-06-10T01:13:20-03:00",
          "tree_id": "d1baf38e5b409d4c1f79b300cc2a5a4566da0037",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1162b1344a2eb1507ae4d40b39b346e9ad41d5fb"
        },
        "date": 1781065070975,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8102,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2683581,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 338,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 166,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 32497,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 283,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57533,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 317,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 63276,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 329,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71081,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 801,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 211635,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fc6157b23e7dac9792917e76a69cff3e3090e227",
          "message": "perf(receipt): aggregate offline delivery receipts per chat like WA Web (#820)",
          "timestamp": "2026-06-10T01:13:46-03:00",
          "tree_id": "e96b18954470830bab0e5b96ca85e0e8d205f37b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fc6157b23e7dac9792917e76a69cff3e3090e227"
        },
        "date": 1781065120836,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8126,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2749739,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 237,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43462,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 251,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 51882,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 456,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 83786,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 327,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71795,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 778,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203356,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 9,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a0649f2429bbd3a580c0aaa87edab1ecd07f2e4d",
          "message": "perf(appstate): batch the previous-MAC prefetch in build_patch like the inbound path (#821)",
          "timestamp": "2026-06-10T01:15:49-03:00",
          "tree_id": "26283695ac1ad04f7fe607ac1e99479a1a1ae28e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a0649f2429bbd3a580c0aaa87edab1ecd07f2e4d"
        },
        "date": 1781065220779,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8120,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2744580,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 186,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36522,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 261,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 53621,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 418,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 74850,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 326,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71121,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 804,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 211855,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "324afb774f413861803933b0f9d6d61b8dda4ec2",
          "message": "perf(send): probe the LID-PN map in one direction on warm device lookups (#823)",
          "timestamp": "2026-06-10T01:40:25-03:00",
          "tree_id": "20d15d28dc79a462b9e0a263b698e031ca6224e1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/324afb774f413861803933b0f9d6d61b8dda4ec2"
        },
        "date": 1781066650548,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8173,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2810157,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 150,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 29931,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 244,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 49071,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 341,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 65902,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 325,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71108,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 776,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 201435,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fb114384815824a0cca09b3312f12f0b9abb5897",
          "message": "perf(send): memoize the per-group device list behind a topology generation (#824)",
          "timestamp": "2026-06-10T08:08:03-03:00",
          "tree_id": "b3f2aedc6ea25ec8e455daef57acba10a709d434",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fb114384815824a0cca09b3312f12f0b9abb5897"
        },
        "date": 1781089994454,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8185,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2745441,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 262,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47617,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 257,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 51748,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 255,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 53218,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 327,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71049,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 785,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203582,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "59cccb8766a9d7cc4bb3dd6e3e6ef54ace836f18",
          "message": "fix(contacts): use fn items for LID mapping extractors so boxed futures compile (#826)",
          "timestamp": "2026-06-10T08:31:45-03:00",
          "tree_id": "c82efde6ea08210301b0beee963f1f8bac7b470c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/59cccb8766a9d7cc4bb3dd6e3e6ef54ace836f18"
        },
        "date": 1781091337242,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8272,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2864977,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 187,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36804,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 242,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 49254,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 344,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 66010,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 326,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71995,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 780,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203342,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "50e0db4688af347d96296c0c5cb56762e32eafbb",
          "message": "perf(client): borrow the ack id instead of allocating a String per stanza (#827)",
          "timestamp": "2026-06-10T09:28:42-03:00",
          "tree_id": "1fcb603e2ab198f185133dd46fd2eb87b5fedd9f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/50e0db4688af347d96296c0c5cb56762e32eafbb"
        },
        "date": 1781094764180,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8154,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2793956,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 227,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 45287,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 272,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55478,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 457,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 83532,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 323,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 70744,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 779,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 201208,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "316662881a0d6f13db070907f6b293b09eb1913a",
          "message": "fix(appstate): match WA Web index-mode ltHash for SET+REMOVE on the same index (#829)",
          "timestamp": "2026-06-10T09:56:53-03:00",
          "tree_id": "7d9dfdc48910349540b9c2b7896f6324af6c325b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/316662881a0d6f13db070907f6b293b09eb1913a"
        },
        "date": 1781096495701,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8221,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2799426,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 150,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 30087,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 250,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 51770,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 337,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 65363,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 322,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 70171,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 806,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 209730,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2b2d7744bdaaf1d87af15c1df213928618d7ecb8",
          "message": "feat(messages): encrypted CAG reactions and channel comments, both directions (#830)",
          "timestamp": "2026-06-10T11:36:59-03:00",
          "tree_id": "9538afd6ed87e7d3998a545ccac4dfbfcdde7a9a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2b2d7744bdaaf1d87af15c1df213928618d7ecb8"
        },
        "date": 1781102518245,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8074,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2843661,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 183,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36384,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 276,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55892,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 340,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 66077,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 324,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71125,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 779,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203326,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "14458994c1f210b01452ab24ae09727a58d502ad",
          "message": "perf(messages): write-behind buffer for messageSecret persistence (#831)",
          "timestamp": "2026-06-10T12:43:22-03:00",
          "tree_id": "084f0f180732f115f072c7a1335cbec4ce27d669",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/14458994c1f210b01452ab24ae09727a58d502ad"
        },
        "date": 1781106445932,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8194,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2845241,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 229,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40184,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 166,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 31099,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 502,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 86806,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73168,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 780,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203384,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ef9c5017e283cd1a6a769c1dc2c1de5b93f4a7f8",
          "message": "fix(storage): serialize msg_secret reads through the db semaphore (#832)",
          "timestamp": "2026-06-10T13:44:42-03:00",
          "tree_id": "660f52b1645ae6ee680c2d368ab75ec665f89942",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ef9c5017e283cd1a6a769c1dc2c1de5b93f4a7f8"
        },
        "date": 1781110052030,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8297,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2829085,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 195,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 32679,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 237,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 44143,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 287,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 55361,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73584,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 805,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 207806,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3c5a753afafd8f62aa5d3ebf54379eaf0f33605e",
          "message": "feat(prekeys): track the first un-uploaded prekey and reuse the window like WA Web (#833)",
          "timestamp": "2026-06-10T15:04:00-03:00",
          "tree_id": "ea227e9d74e121ce7d9ed4b902b8b60ed13dac75",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3c5a753afafd8f62aa5d3ebf54379eaf0f33605e"
        },
        "date": 1781114941145,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11503,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3014730,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 223,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42159,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 269,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50184,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 500,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 86550,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73057,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 779,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203696,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "753d2ae3cb760832721a09ce9ec7c8b3962ce46c",
          "message": "feat(api): adopt the impl Into<Jid> convention across the public surface (#834)",
          "timestamp": "2026-06-10T16:24:54-03:00",
          "tree_id": "1e7bec8d49a88173756577b2431c43d7c106a7fd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/753d2ae3cb760832721a09ce9ec7c8b3962ce46c"
        },
        "date": 1781119784332,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11525,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2964482,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 191,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37709,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 292,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56153,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 297,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 56435,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73336,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 806,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 209944,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
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
          "id": "36c23e3d08a0e213289b745f5a89c5fb1a38f939",
          "message": "Add CodSpeed performance measurement setup (#828)\n\nCo-authored-by: codspeed-hq[bot] <117304815+codspeed-hq[bot]@users.noreply.github.com>\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-06-10T17:10:47-03:00",
          "tree_id": "63c04c7485679d8e6bffde7dd2ccf74390345c08",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/36c23e3d08a0e213289b745f5a89c5fb1a38f939"
        },
        "date": 1781122435696,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11508,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2951219,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 211,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41233,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 318,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 60222,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 398,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 70723,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73124,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 949,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 233945,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4e664f848216742d63b34218cc519cc9a1a99e64",
          "message": "ci(codspeed): run simulation and memory instruments in a single job (#835)",
          "timestamp": "2026-06-10T17:46:12-03:00",
          "tree_id": "73943542b1bfe691013823a9b18f7e1bfabe5cba",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4e664f848216742d63b34218cc519cc9a1a99e64"
        },
        "date": 1781124547892,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11399,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2936770,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 247,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43352,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 292,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56442,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 326,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 58878,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73325,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 952,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 236141,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d3645a8f16a5f1bc38b7bd5a3ab4f2eda1a463e3",
          "message": "perf(history-sync): gate prost decode behind a secret-presence scan, share chat ids, schema-pinned wire tags (#836)",
          "timestamp": "2026-06-10T22:43:07-03:00",
          "tree_id": "df3336fcddc67efe4f14fb01589244d83ec7869c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d3645a8f16a5f1bc38b7bd5a3ab4f2eda1a463e3"
        },
        "date": 1781142487753,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11773,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3049003,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 185,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36460,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 301,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57882,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 391,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 69658,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72639,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 809,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 212404,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b9e0941cba0da3bb7bb352b82711a6c7ae9070f2",
          "message": "bench: measure the operation, not the harness (#837)",
          "timestamp": "2026-06-10T23:45:29-03:00",
          "tree_id": "f0e32ef1e7b170c055bac32cbe762cf8b2c32477",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b9e0941cba0da3bb7bb352b82711a6c7ae9070f2"
        },
        "date": 1781146100370,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11562,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2911054,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 201,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 38827,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 314,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 59959,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 470,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 80835,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 362,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73159,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 808,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 212347,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6e734677a4699dc05f910984eac627cea08a9eac",
          "message": "perf(libsignal): memoize the sender signing key with a pre-warmed XEdDSA cache (#838)",
          "timestamp": "2026-06-11T00:29:37-03:00",
          "tree_id": "c8acd4ee087bf7fbdf8448c8598cb499c4c846dc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6e734677a4699dc05f910984eac627cea08a9eac"
        },
        "date": 1781148892133,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11413,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2932596,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 236,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42724,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 297,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56845,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 290,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 56140,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73082,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 949,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 236001,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e5c660fe8bf17964c738f902dc207bd947e80993",
          "message": "perf(libsignal): cache the verify-side Edwards derivations per sender key (#839)",
          "timestamp": "2026-06-11T01:27:20-03:00",
          "tree_id": "6ab3f45b7d9a8ece1985a714179dd9ddaaf0b57d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e5c660fe8bf17964c738f902dc207bd947e80993"
        },
        "date": 1781152345891,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11344,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2975450,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 285,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 46687,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 309,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57747,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 379,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 68189,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73100,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 986,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 241767,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8ab270225cb4ff8d3f9d38acea718970ad40b8d3",
          "message": "perf(send): memoize the group phash on the device-list memo entry (#840)",
          "timestamp": "2026-06-11T02:25:21-03:00",
          "tree_id": "2848da045cdd11be9193d497eacb6f0ba5b2bfd1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8ab270225cb4ff8d3f9d38acea718970ad40b8d3"
        },
        "date": 1781155796099,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11462,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2886640,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 222,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40696,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 164,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 31049,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 513,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 88089,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73908,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 777,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 201700,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4edba602e9bf72829e16da992ce8190802608bbb",
          "message": "fix(pdo): request a placeholder resend at most once per message (#841)",
          "timestamp": "2026-06-11T02:43:32-03:00",
          "tree_id": "b4313e7d4a3de10e1a37dd65d97c7c283504c0b8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4edba602e9bf72829e16da992ce8190802608bbb"
        },
        "date": 1781156852961,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11413,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2969533,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 177,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 31622,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 287,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55063,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 447,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 79008,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73750,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 780,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203729,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d472d04815c33e9894077135037ff8bd1631e530",
          "message": "perf(waproto): pin the Message codec to one instantiation via non-generic helpers (#842)",
          "timestamp": "2026-06-11T04:05:07-03:00",
          "tree_id": "baa127e668471d181efe1f570e0d1702224bef81",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d472d04815c33e9894077135037ff8bd1631e530"
        },
        "date": 1781161772447,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11566,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3038870,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 225,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43168,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 164,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 31027,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 518,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 88069,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73202,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 904,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 230730,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "317ae83f0cdc75bbb903e3e2d49964348dc729f6",
          "message": "perf(retry): fuse the retry count and reason into one cache entry (#844)",
          "timestamp": "2026-06-11T04:27:13-03:00",
          "tree_id": "581f9498feb7ac3f35f7cda7a0820cb9c928200b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/317ae83f0cdc75bbb903e3e2d49964348dc729f6"
        },
        "date": 1781163039244,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11583,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2973046,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 188,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36763,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 319,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 60418,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 309,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 58366,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73570,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1002,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 243090,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0aa6cb97d2a380b86e1be7eaa3bf6244e83f8647",
          "message": "perf(api): box the cold entry-point futures so consumers stop re-codegening the graphs (#843)",
          "timestamp": "2026-06-11T04:28:00-03:00",
          "tree_id": "6b348a00af28a0fa1948298c6ad3352c07073f4a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0aa6cb97d2a380b86e1be7eaa3bf6244e83f8647"
        },
        "date": 1781163108151,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11549,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2973473,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 190,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36554,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 292,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56091,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 511,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 87645,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73136,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 958,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 238431,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7c465136f88716e28206e8315a1fa1636ce97f10",
          "message": "perf(docker): enable -Zshare-generics in the image build (#845)",
          "timestamp": "2026-06-11T09:11:24-03:00",
          "tree_id": "64d8bd8edca034e4eeffd0cd6fd019411a6fef5a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7c465136f88716e28206e8315a1fa1636ce97f10"
        },
        "date": 1781180031387,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11759,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3003076,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 225,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 39717,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 309,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57845,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 307,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 57258,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 365,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 74270,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 781,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 205669,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a3a0a571a7a7045751125290cc0704bac7e251aa",
          "message": "perf(wacore): drop the proto PartialEq anchor from the skdm-only check (#846)",
          "timestamp": "2026-06-11T09:33:02-03:00",
          "tree_id": "d940efa4113b66b60ef3a9f035da7ee77de54958",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a3a0a571a7a7045751125290cc0704bac7e251aa"
        },
        "date": 1781181423908,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11503,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2960070,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 247,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 46858,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 254,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 47656,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 436,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 78010,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 365,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 74316,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 782,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 205722,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "be3d2559f9cef9c1503be51ac6c8dc93b67263de",
          "message": "perf(appstate): pre-key the ltHash HKDF extract once (#847)",
          "timestamp": "2026-06-11T10:18:56-03:00",
          "tree_id": "a46808741d66a0128ad06015a3243205491231d0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/be3d2559f9cef9c1503be51ac6c8dc93b67263de"
        },
        "date": 1781184171683,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11424,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2989748,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 196,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 33916,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 269,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50737,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 297,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 53602,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 365,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73543,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1004,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 244872,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d68af6c608297c864669850b9bc05d4a54410d15",
          "message": "perf(binary): emit the Jid display as one write_str (#848)",
          "timestamp": "2026-06-11T10:36:43-03:00",
          "tree_id": "4225d88e42037678f296b7338bad520973e0bc96",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d68af6c608297c864669850b9bc05d4a54410d15"
        },
        "date": 1781185264550,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11486,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3002742,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 168,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 31819,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 301,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56049,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 320,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 57708,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73599,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 963,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 238905,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0bcc3a0cd7ba79db11a0daa078b9012866887dc7",
          "message": "bench: borrow inputs in the benches that only read them (#849)",
          "timestamp": "2026-06-11T10:44:22-03:00",
          "tree_id": "b6339cfa9f3d13be236166535a4c7e9d12ea76b5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0bcc3a0cd7ba79db11a0daa078b9012866887dc7"
        },
        "date": 1781185650587,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11352,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2987133,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 215,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42056,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 297,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56039,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 518,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 89128,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73280,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 779,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 205601,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "47b7f9635d92c0903e999f9f6bda0f2b26d6c48a",
          "message": "perf(noise): pre-key the transport AES-GCM once per connection (#850)",
          "timestamp": "2026-06-11T11:52:27-03:00",
          "tree_id": "65524fc1c9a2acb87c6ab345c0f2b6b775f44823",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/47b7f9635d92c0903e999f9f6bda0f2b26d6c48a"
        },
        "date": 1781189818780,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11497,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2948669,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 214,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37865,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 265,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 49705,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 371,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 69526,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 365,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73953,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 810,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 216108,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fd0e1716c7c9a3c2a71c1636b111b18c484c361d",
          "message": "deps: upgrade curve25519-dalek and x25519-dalek to the 5.0/3.0 release candidates (#851)",
          "timestamp": "2026-06-11T12:22:16-03:00",
          "tree_id": "90d7c5c3143b98483c18a05d69a6acd44ea7a681",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fd0e1716c7c9a3c2a71c1636b111b18c484c361d"
        },
        "date": 1781191600143,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11335,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2983084,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 260,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 48653,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 316,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 59425,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 411,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 73010,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 365,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73955,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 783,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 205555,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 9,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9e8fca9a67568c682f9e2c2282951b9bfe581816",
          "message": "feat!: overhaul the public bot API ahead of 1.0 (#852)",
          "timestamp": "2026-06-11T14:24:34-03:00",
          "tree_id": "4b2d901dc9dca9f5421264e537c3f43ae718180c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9e8fca9a67568c682f9e2c2282951b9bfe581816"
        },
        "date": 1781198908919,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11566,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2917367,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 150,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 28861,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 269,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50336,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 491,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 86030,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 73222,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 811,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 212317,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "eecda89dbcf51e9aec7a92e43a4d3336c1974e96",
          "message": "perf(history-sync)!: store the compressed payload and expose a streaming reader (#853)",
          "timestamp": "2026-06-11T18:26:31-03:00",
          "tree_id": "4a1edc67d8fd1f282923e50a8a34fa35bf75fa41",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/eecda89dbcf51e9aec7a92e43a4d3336c1974e96"
        },
        "date": 1781213458042,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11395,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3000267,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 212,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37537,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 230,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 43020,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 457,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 78934,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 366,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72614,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 810,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 205186,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "38cc31df19873ff48d68be7d31cb31e585a4805c",
          "message": "chore(reporting-token): draw the message secret from the thread RNG (#855)",
          "timestamp": "2026-06-11T22:32:13-03:00",
          "tree_id": "2a403d3ef1318aff02b25ae3fcdc594307d4aeae",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/38cc31df19873ff48d68be7d31cb31e585a4805c"
        },
        "date": 1781228140097,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11361,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2888302,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 327,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 232,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 39239,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 313,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 59028,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 656,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 203797,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 371,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72743,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 809,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203224,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 5,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b956722b74255705ec34a7dda1023b27ac4ad0ea",
          "message": "bench: cover the receive path — plaintext decode and appstate index-MAC dedup (#856)\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-06-11T23:24:09-03:00",
          "tree_id": "f27caea12272daa252648788518b84f1dfaae0c2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b956722b74255705ec34a7dda1023b27ac4ad0ea"
        },
        "date": 1781231276538,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11532,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3027546,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 207,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37470,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 166,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 31162,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 510,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 84939,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72353,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 808,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 203058,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "53c7975eaefd6eb0a2d57b603d7ee8e76e581129",
          "message": "bench: cover four inbound/group hot paths for CodSpeed baselines (#858)",
          "timestamp": "2026-06-12T12:59:10-03:00",
          "tree_id": "dc941d01181204221562291223fb5d722afc853d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/53c7975eaefd6eb0a2d57b603d7ee8e76e581129"
        },
        "date": 1781280113095,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11446,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3009791,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 237,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 45305,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 265,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 49800,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 506,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 87298,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 362,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 71711,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1058,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 248283,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781289307367,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11493,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3152140,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 238,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 39686,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 308,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 58664,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 291,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 55616,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 364,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72247,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 811,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 201279,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781392052737,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11635,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3028138,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 222,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40251,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 288,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 53713,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 506,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 85217,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72476,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 788,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 191192,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781395953717,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11251,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2530202,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 340,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 189,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 31197,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 315,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57762,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 496,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 173711,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 364,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 67913,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 770,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 179959,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781396004950,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11459,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3117965,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 340,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 158,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 31824,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 303,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55673,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 407,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 70455,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 366,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 72044,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 818,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 199880,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781442777192,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11194,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2636260,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 338,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 251,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 46517,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 305,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55908,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 296,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 55145,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 362,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 67621,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 769,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 180071,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781449275900,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11328,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2568196,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 338,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 203,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37389,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 298,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 54731,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 451,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 168535,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 365,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 68167,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 769,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 179903,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781451911559,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11226,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2639882,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 177,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 33194,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 167,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 30353,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 379,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 66371,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 42,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 67306,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 770,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 179961,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781473635144,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11194,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2585215,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 244,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42628,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 259,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 45581,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 507,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 80239,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 63050,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 770,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168672,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781478785636,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11340,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2504299,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 339,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 225,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43416,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 305,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 53142,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 511,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 80766,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 62952,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 770,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168722,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781484550294,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11206,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2587501,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 255,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43159,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 245,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 43038,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 403,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 66229,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 359,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 63133,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 768,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168677,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781528719635,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11198,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2585658,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 184,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35413,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 253,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 43982,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 377,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 61556,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 365,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 63758,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 941,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 202083,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781533108730,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11334,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2494615,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 206,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 34348,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 307,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 53878,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 400,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 64669,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 62950,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 768,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168552,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781538690869,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11260,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2479416,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 230,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37709,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 297,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 51578,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 462,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 74983,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 360,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 63351,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 768,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168729,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781543974770,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11195,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2585516,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 191,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35916,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 304,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 52136,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 313,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 52952,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 362,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 63404,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 773,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 169341,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781548830647,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11223,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2587981,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 156,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 30937,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 324,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56315,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 406,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 65072,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 62995,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 772,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168906,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781553765400,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11109,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2464325,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 237,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 39009,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 251,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 44385,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 521,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 81390,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 354,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 62724,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 905,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 197797,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781558103105,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11290,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2492264,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 217,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 38308,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 276,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 49790,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 431,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 69991,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 357,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 63264,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 759,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168223,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781561298775,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11228,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2591238,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 201,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37790,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 288,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50707,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 391,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 64315,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 349,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 62500,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 981,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 207102,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781565975244,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11238,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2607146,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 385,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 233,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40050,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 165,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 28245,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 426,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 68074,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 350,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 60882,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 760,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168184,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781571387736,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11172,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2585809,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 229,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40065,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 226,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 38588,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 286,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 49347,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 350,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 60755,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 760,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168186,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781719250272,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11247,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2479111,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 218,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36203,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 239,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 41008,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 510,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 79635,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 347,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 60410,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 759,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168137,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781748757479,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11225,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2591117,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 207,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 33982,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 268,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 46334,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 483,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 76775,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 347,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 60433,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 780,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 170153,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781748918237,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11156,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2470266,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 158,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 28213,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 268,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 45236,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 377,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 61025,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 347,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 60462,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 765,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168677,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781749280994,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11281,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2496400,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 155,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 28859,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 293,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50702,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 438,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 68370,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 350,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 60947,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 760,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168198,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781750032692,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11302,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2379303,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 334,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 153,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 27772,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 166,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 28280,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 501,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 78326,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 65233,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 967,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 205738,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 6,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781750697153,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11175,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2584602,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 340,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 263,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43180,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 296,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 51203,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 488,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 146332,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 365,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 63507,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 760,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 168236,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 7,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "55464917+jlucaso1@users.noreply.github.com",
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
        "date": 1781784093402,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 11177,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2586180,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 236,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 39239,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 289,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 49103,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 306,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 51543,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 349,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 60642,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 933,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 201927,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 8,
            "unit": "milliseconds"
          }
        ]
      }
    ]
  }
}