window.BENCHMARK_DATA = {
  "lastUpdate": 1781066652164,
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
          "id": "37a0bf2b9c1e1c8f685348e59bb1a97d795e1c87",
          "message": "perf(identity): gate identity-change reset behind had-prior-identity, like WA Web (#718)",
          "timestamp": "2026-06-04T19:24:12-03:00",
          "tree_id": "41b71f01dbe229c975ae39ae29a1d945c88b9c28",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/37a0bf2b9c1e1c8f685348e59bb1a97d795e1c87"
        },
        "date": 1780612060000,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8274,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2839688,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 179,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35790,
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
            "value": 56572,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 527,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 91733,
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
            "value": 77598,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1067,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 257416,
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
          "id": "c6385d0b6091cca3f8a6fc46bf3b4768bb4de8cf",
          "message": "feat(reaction): add Client::send_reaction and MessageContext::react for DM/group (#720)",
          "timestamp": "2026-06-04T21:26:29-03:00",
          "tree_id": "cecbfb4ca4a87c3d8eb42fcfe7d5f25b1aabffd6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c6385d0b6091cca3f8a6fc46bf3b4768bb4de8cf"
        },
        "date": 1780619442102,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8254,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2824783,
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
            "value": 44727,
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
            "value": 70077,
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
            "value": 102522,
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
            "value": 77715,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 866,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 221258,
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
          "id": "67df75d44fc9cf8612cfbc409bcbfb1422a7370c",
          "message": "perf(events): snapshot handlers behind an Arc to drop the per-event Vec clone (#719)",
          "timestamp": "2026-06-04T21:27:30-03:00",
          "tree_id": "1da9b08e14866d121783f7783394c5efe6cfc277",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/67df75d44fc9cf8612cfbc409bcbfb1422a7370c"
        },
        "date": 1780619479801,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8347,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2850480,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 342,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 198,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40355,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 317,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 62237,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 401,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 75019,
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
            "value": 77618,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 866,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 221890,
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
          "id": "f39f14911fa6dfceeda625cfb4a3503dd5af74dc",
          "message": "perf(signal-cache): amortize eviction and stop scanning on reads (#721)",
          "timestamp": "2026-06-04T21:49:54-03:00",
          "tree_id": "7bb520bdae64b808be1624f56bc9a56522c053cc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f39f14911fa6dfceeda625cfb4a3503dd5af74dc"
        },
        "date": 1780620825335,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8277,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2788587,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 234,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42086,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 277,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 54579,
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
            "value": 71806,
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
            "value": 77347,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1070,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 263743,
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
          "id": "e55fa4aca219c158b0d7b3de5c55b49e72280362",
          "message": "fix(prekey): remove the consumed one-time prekey atomically with the session flush (#722)",
          "timestamp": "2026-06-04T22:48:45-03:00",
          "tree_id": "c8d10ff7abd258ea4cb6ec8c14724cdc3752c1c3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e55fa4aca219c158b0d7b3de5c55b49e72280362"
        },
        "date": 1780624388507,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8271,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2789905,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 195,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37832,
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
            "value": 60010,
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
            "value": 88711,
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
            "value": 77494,
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
            "value": 210592,
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
          "id": "bae993c6d9022cfebb3f0ea8f84ceeec484b3bf6",
          "message": "fix(edit): send message edits as a top-level protocolMessage, matching WA Web (#723)",
          "timestamp": "2026-06-04T23:45:01-03:00",
          "tree_id": "a0e8bdbe4778eebac61a341ee7e383c53016b06f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bae993c6d9022cfebb3f0ea8f84ceeec484b3bf6"
        },
        "date": 1780627758107,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8205,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2784454,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 213,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42253,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 179,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 35681,
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
            "value": 91969,
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
            "value": 77364,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 833,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 212589,
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
          "id": "e310ec94755c1d2dbcf49fb50f22515f9c7f3762",
          "message": "fix: @call JID decode, receipt participant_pn, profile-pic empty→remove, has_device churn (#724)",
          "timestamp": "2026-06-05T00:42:45-03:00",
          "tree_id": "54444c6e7f207b1dc2f34d3ab1318bdfcb817be1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e310ec94755c1d2dbcf49fb50f22515f9c7f3762"
        },
        "date": 1780631212579,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8352,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2845295,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 259,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 48102,
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
            "value": 55714,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 285,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 55786,
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
            "value": 77890,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 840,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 213020,
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
          "id": "e5da892d0ba6920a22282b12befa7ff24fff66b7",
          "message": "feat(newsletter): plaintext channel edit/revoke + reject newsletter on the E2E send path (#725)",
          "timestamp": "2026-06-05T02:15:12-03:00",
          "tree_id": "7c96552632aa36b959a149849098dcfb802ef11b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e5da892d0ba6920a22282b12befa7ff24fff66b7"
        },
        "date": 1780636743761,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8294,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2797731,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 180,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 34903,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 206,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 39502,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 460,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 82501,
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
            "value": 77656,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 875,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 222905,
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
          "id": "8d35b042631c052f672dc78124eaa4bc1e15cc30",
          "message": "chore(proto): sync to WhatsApp Web 2.3000.1040878135 (#726)",
          "timestamp": "2026-06-05T09:48:31-03:00",
          "tree_id": "fffee46799539fb5f4242fdf4191c637934720af",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8d35b042631c052f672dc78124eaa4bc1e15cc30"
        },
        "date": 1780663986453,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8372,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2825474,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 245,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 50481,
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
            "value": 53299,
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
            "value": 60041,
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
            "value": 79886,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1038,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 259106,
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
          "id": "a0c7bc23eb243d31f102f32a97da208523202902",
          "message": "feat(mex): typed mex operations from the whatspec IR, drop hand-maintained mex_ids (#728)",
          "timestamp": "2026-06-05T12:06:36-03:00",
          "tree_id": "e2de4c72a438dfd2f0e68f4669eed1a8e1e20f39",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a0c7bc23eb243d31f102f32a97da208523202902"
        },
        "date": 1780672266451,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8209,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2798071,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 250,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47781,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 179,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 36288,
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
            "value": 58870,
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
            "value": 78701,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1039,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 256975,
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
          "id": "3f6ffe448dcfed040517191659c5cdaa78f28643",
          "message": "feat(abprops): vendor typed A/B-props registry, drop hand-maintained config_codes (#729)",
          "timestamp": "2026-06-05T13:40:13-03:00",
          "tree_id": "3ebcd28d8eddb86a55a57d81acd71facd0b5824a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3f6ffe448dcfed040517191659c5cdaa78f28643"
        },
        "date": 1780677883562,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8333,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2883818,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 181,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36466,
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
            "value": 63027,
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
            "value": 89549,
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
            "value": 78668,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 833,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215774,
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
          "id": "09a3b0c15afe760a56cf751f19713861661d25a6",
          "message": "fix(send): address LID-mapped DM by LID end to end so the server stops 400-rejecting it (#731)",
          "timestamp": "2026-06-05T18:56:19-03:00",
          "tree_id": "c6bae1759d294b9bbc727eb1bb7dbe078ab82e7d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/09a3b0c15afe760a56cf751f19713861661d25a6"
        },
        "date": 1780696856106,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8329,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2802528,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 338,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 291,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 50601,
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
            "value": 66269,
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
            "value": 91163,
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
            "value": 78544,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1082,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 268775,
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
          "id": "a4f8b45a7eca83653fa7a7f98ae2ec0dc41d8ff4",
          "message": "refactor: split message/client/send monoliths into per-theme modules (#732)",
          "timestamp": "2026-06-05T22:03:30-03:00",
          "tree_id": "d449a2a8b4ef60e14cf8a935c531871a3ae553af",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a4f8b45a7eca83653fa7a7f98ae2ec0dc41d8ff4"
        },
        "date": 1780708076721,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8142,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2836503,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 219,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 44235,
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
            "value": 53558,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 486,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 89185,
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
            "value": 78954,
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
            "value": 213888,
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
          "id": "3c3da276daad63a30eddf1478359c4e9fbde3482",
          "message": "feat(observability): optional tracing instrumentation (off by default, OTel-ready) (#733)",
          "timestamp": "2026-06-06T13:56:23-03:00",
          "tree_id": "c653bd7877da65fc690fd24769082fead775b4b9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3c3da276daad63a30eddf1478359c4e9fbde3482"
        },
        "date": 1780765250453,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8351,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2869482,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 219,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43162,
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
            "value": 62063,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 494,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 89607,
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
            "value": 78341,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 858,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 220073,
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
          "id": "f6c94fa0b8650d2bd4c2d4534eeb8f828d05f06a",
          "message": "feat(metrics): optional metrics layer (off by default, Prometheus/OTLP-ready) (#734)",
          "timestamp": "2026-06-06T15:14:13-03:00",
          "tree_id": "e1b378896d5df7c1c2ab765abf2392469bd1b92f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f6c94fa0b8650d2bd4c2d4534eeb8f828d05f06a"
        },
        "date": 1780769908015,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8371,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2885386,
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
            "value": 43623,
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
            "value": 56818,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 284,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 59096,
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
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1080,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 262562,
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
          "id": "a673a3e72ca790ca9f999e2c019b6021ff62a4a2",
          "message": "feat(api): mark public error enums #[non_exhaustive] for 1.0 forward-compat (#735)",
          "timestamp": "2026-06-07T19:59:59-03:00",
          "tree_id": "5e71435a4f6ae86a59094f79168c0587746fe433",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a673a3e72ca790ca9f999e2c019b6021ff62a4a2"
        },
        "date": 1780873486411,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8294,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2807444,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 230,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 45466,
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
            "value": 57116,
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
            "value": 90863,
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
            "value": 78815,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 833,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215832,
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
          "id": "df901a66e7825cecfd1a0aa597276a1b750dd5f5",
          "message": "feat(api): #[non_exhaustive] on public value enums + drop dead privacy enums (#736)",
          "timestamp": "2026-06-07T21:06:05-03:00",
          "tree_id": "08afd173f8687d55c49978e85ff90375bd2673d8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/df901a66e7825cecfd1a0aa597276a1b750dd5f5"
        },
        "date": 1780877436772,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8210,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2798093,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 261,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 50119,
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
            "value": 61319,
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
            "value": 89278,
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
            "value": 78917,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 836,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215828,
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
          "id": "435ad65e3b15ce6e091ec50caf21597fe13cd124",
          "message": "feat(groups): high-level set/remove group profile picture (#739)",
          "timestamp": "2026-06-07T21:06:35-03:00",
          "tree_id": "0a758782d609f3a505b355caebc8849973da3845",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/435ad65e3b15ce6e091ec50caf21597fe13cd124"
        },
        "date": 1780877464328,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8203,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2791389,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 199,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 38547,
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
            "value": 54512,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 381,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 73279,
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
            "value": 78343,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 834,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 213851,
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
          "id": "e36ad5497fb6cb3f5e35439d12cc4b815a8034e0",
          "message": "feat(send): add keep_message (keep-in-chat for everyone) (#740)",
          "timestamp": "2026-06-07T21:09:11-03:00",
          "tree_id": "2ba976a145d87957cbf97ce06fa304061835b66f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e36ad5497fb6cb3f5e35439d12cc4b815a8034e0"
        },
        "date": 1780877618532,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8297,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2884552,
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
            "value": 36910,
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
            "value": 62138,
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
            "value": 76535,
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
            "value": 80271,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 833,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215743,
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
          "id": "2c1c9ddcd23ef31e45a7c6918de9ba046bb45b47",
          "message": "feat(receipt): add mark_as_played for voice/video notes (#737)",
          "timestamp": "2026-06-07T21:09:33-03:00",
          "tree_id": "7aa946c3b4b0d1cef826bff02037c1c0026b182d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2c1c9ddcd23ef31e45a7c6918de9ba046bb45b47"
        },
        "date": 1780877699937,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8310,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2859114,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 220,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43186,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 248,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 49370,
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
            "value": 89431,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 359,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78971,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 990,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 251258,
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
          "id": "d840086961a254452baf5bad8076e939b79e2899",
          "message": "feat(send): add forward_message + MessageExt::prepare_for_forward (#738)",
          "timestamp": "2026-06-07T21:25:06-03:00",
          "tree_id": "060bc9308dc8a0db3ec3dc739ff36f641cccefd5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d840086961a254452baf5bad8076e939b79e2899"
        },
        "date": 1780878567139,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8288,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2827174,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 228,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43407,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 300,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 60245,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 373,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 71679,
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
            "value": 79031,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 833,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215819,
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
          "id": "f4bcd0ff678ffac1fdbbe7935671a92b61cc77e8",
          "message": "feat(usync): surface verified business name (verified_name) (#741)",
          "timestamp": "2026-06-07T22:38:31-03:00",
          "tree_id": "97589c602c590db9ed39ec166fb1b33c7630d941",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f4bcd0ff678ffac1fdbbe7935671a92b61cc77e8"
        },
        "date": 1780882971434,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8156,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2843775,
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
            "value": 43300,
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
            "value": 59968,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 288,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 59469,
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
            "value": 79425,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 834,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215871,
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
          "id": "813cd1e6200b8aea2ea2c3e647ba130bbc43a2da",
          "message": "feat(send): emit <meta view_once=true/> for view-once sends (#743)",
          "timestamp": "2026-06-07T22:39:00-03:00",
          "tree_id": "797f6275ced16a7fcb2025c30b5b82ae25ffd828",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/813cd1e6200b8aea2ea2c3e647ba130bbc43a2da"
        },
        "date": 1780883033152,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8291,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2856692,
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
            "value": 44813,
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
            "value": 55145,
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
            "value": 58711,
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
            "value": 79205,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 862,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 224277,
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
          "id": "2413271c56e75162df42ce307fccb9dedb6e24f3",
          "message": "feat(receipt): read-receipt status parity (read-self, context, peer_participant_pn) (#744)",
          "timestamp": "2026-06-07T22:41:38-03:00",
          "tree_id": "a57be890cb6746c56825f380820702d11342cf70",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2413271c56e75162df42ce307fccb9dedb6e24f3"
        },
        "date": 1780883114899,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8225,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2797071,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 205,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37419,
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
            "value": 60992,
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
            "value": 89450,
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
            "value": 78916,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1011,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 250703,
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
          "id": "ec6990ceee0d27b758fa1d4fa47e4d6141c8abb0",
          "message": "feat(client): set_chat_disappearing_timer for 1:1 disappearing messages (#745)",
          "timestamp": "2026-06-07T22:42:23-03:00",
          "tree_id": "391c7e5631bd6c2052e1ad538fe136a76ad46da8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ec6990ceee0d27b758fa1d4fa47e4d6141c8abb0"
        },
        "date": 1780883180014,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8204,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2842901,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 241,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47985,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 262,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 53246,
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
            "value": 60936,
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
            "value": 79817,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 833,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215821,
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
          "id": "8d34a6a7c63a8ef43b440dcee47e37bc6d5df695",
          "message": "feat(chat-actions): add save_contact (outgoing contact-name sync) (#742)",
          "timestamp": "2026-06-07T22:52:33-03:00",
          "tree_id": "035d3d1c88079862762f3d001648bcae1d232f36",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8d34a6a7c63a8ef43b440dcee47e37bc6d5df695"
        },
        "date": 1780883776575,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8248,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2848972,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 231,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43161,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 295,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 59960,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 285,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 59179,
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
            "value": 79260,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 860,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 218159,
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
          "id": "3ae79a4aa8d2b60e323621bc03ef55595d57f182",
          "message": "fix(binary): coerce \"1\"/\"0\" wire booleans in attr parsers (#746)",
          "timestamp": "2026-06-07T23:57:42-03:00",
          "tree_id": "6d85a92f781974dc1874be8489c1e3c1ccc55cff",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3ae79a4aa8d2b60e323621bc03ef55595d57f182"
        },
        "date": 1780887739235,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8243,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2854234,
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
            "value": 44569,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 204,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 40278,
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
            "value": 90027,
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
            "value": 78694,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1055,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 260443,
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
          "id": "7ee40455f72ced6ca51f4196213f86f2ae521d07",
          "message": "feat(iq): keep server error_type + backoff on IQ error responses (#747)",
          "timestamp": "2026-06-08T00:00:19-03:00",
          "tree_id": "97de6f841105c1d8bfd27e35d85bc25db898c2bf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7ee40455f72ced6ca51f4196213f86f2ae521d07"
        },
        "date": 1780887895876,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8221,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2794391,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 249,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47955,
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
            "value": 57862,
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
            "value": 89950,
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
            "value": 79006,
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
            "value": 215936,
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
          "id": "3b2bfa38db9cd14f9a849373a5cf05552c3d5bed",
          "message": "fix(polls): match WA Web poll wire shape (pollContentType=TEXT, no vote metadata) (#749)",
          "timestamp": "2026-06-08T00:01:15-03:00",
          "tree_id": "a89e23312bbf13c073b8fa898f3a8c7bf9336b2c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3b2bfa38db9cd14f9a849373a5cf05552c3d5bed"
        },
        "date": 1780887909206,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8276,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2862472,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 291,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 62193,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 281,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56412,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 373,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 72021,
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
            "value": 78389,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 863,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 224168,
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
          "id": "21e91b25abfe4652f78c303b4b3e3d40f5eb30a4",
          "message": "perf(appstate): return Arc from key-lookup callback (no per-mutation 160B clone) (#750)",
          "timestamp": "2026-06-08T00:03:07-03:00",
          "tree_id": "f63c6f203e65dfd8d991e2c25573bfff1e2b3f64",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/21e91b25abfe4652f78c303b4b3e3d40f5eb30a4"
        },
        "date": 1780888052927,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8262,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2803133,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 225,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43837,
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
            "value": 53152,
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
            "value": 72508,
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
            "value": 79272,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 834,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215915,
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
          "id": "832198a7de2457f5a146fa8e84d9a4be359195fd",
          "message": "fix(appstate): require snapshot MAC when validating (no silent skip) (#748)",
          "timestamp": "2026-06-08T00:05:08-03:00",
          "tree_id": "70911136feef7f3072d677c8775d09975729454d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/832198a7de2457f5a146fa8e84d9a4be359195fd"
        },
        "date": 1780888182340,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8312,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2810270,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 226,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43810,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 273,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 55713,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 427,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 79077,
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
            "value": 79436,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 832,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215604,
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
          "id": "d8d745a034079c18c38bdebda972375e1541fc83",
          "message": "fix(appstate): repair main build broken by #748/#750 merge (#751)",
          "timestamp": "2026-06-08T00:13:35-03:00",
          "tree_id": "fc818606066e4e57c6e3b620c987b0d678b456ee",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d8d745a034079c18c38bdebda972375e1541fc83"
        },
        "date": 1780888685656,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8384,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2821611,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 219,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43166,
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
            "value": 55323,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 405,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 76341,
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
            "value": 78518,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 987,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 248492,
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
          "id": "6b6b6c47b507fce0b59dcbaa110f847aad05d87c",
          "message": "feat(appstate): reject duplicate index within a patch (anti-tampering parity) (#752)",
          "timestamp": "2026-06-08T00:31:43-03:00",
          "tree_id": "cb2ab69cdd8f568134c03e826ed5ca07ce89672c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6b6b6c47b507fce0b59dcbaa110f847aad05d87c"
        },
        "date": 1780889777325,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8344,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2861600,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 227,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 44457,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 278,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 56025,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 508,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 92346,
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
            "value": 78897,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1069,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 267608,
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
          "id": "ae139dc2f58dba50f3e7382680b2cfd7ebb966f1",
          "message": "feat(appstate): guard snapshot apply against version rollback (#753)",
          "timestamp": "2026-06-08T00:39:58-03:00",
          "tree_id": "c7c3061d03f0cdeff75a0173954b88c0de0e146a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ae139dc2f58dba50f3e7382680b2cfd7ebb966f1"
        },
        "date": 1780890260844,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8172,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2729080,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 338,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 224,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43086,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 180,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 36385,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 493,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 89941,
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
            "value": 78834,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 863,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 222321,
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
          "id": "5068cb8eb660cd04736883f332e09c8e96df6bb7",
          "message": "feat(chat): support clearChat app-state action (incoming + outgoing) (#755)",
          "timestamp": "2026-06-08T00:49:59-03:00",
          "tree_id": "4b9bec160cd2ae496fc6aa509be3da9a6437115a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5068cb8eb660cd04736883f332e09c8e96df6bb7"
        },
        "date": 1780890877275,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8158,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2792191,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 247,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 48313,
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
            "value": 51786,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 464,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 83410,
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
            "value": 78356,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 964,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 244903,
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
          "id": "2c923bc26b73c9bb80fa1fe330afcef22b5b5073",
          "message": "fix(wasm): make cache backend target-aware so moka-cache defaults don't break wasm32 (#756)",
          "timestamp": "2026-06-08T07:25:04-03:00",
          "tree_id": "bed531c095f2fa24d5d484eb2f9cd8e95e07d4d7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2c923bc26b73c9bb80fa1fe330afcef22b5b5073"
        },
        "date": 1780914543570,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8165,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2777674,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 339,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 347,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 63214,
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
            "value": 61888,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 381,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 72077,
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
            "value": 78974,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 865,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 224459,
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
          "id": "0cb4ca5cd99efa87183b07519748621a43adce20",
          "message": "feat(polls): support quiz polls on send (create_quiz) (#754)",
          "timestamp": "2026-06-08T07:25:21-03:00",
          "tree_id": "36b148db4dc05eacdaeedff7a71861f5fba65830",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0cb4ca5cd99efa87183b07519748621a43adce20"
        },
        "date": 1780914588387,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8132,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2787119,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 185,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36802,
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
            "value": 53029,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 401,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 76074,
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
            "value": 79614,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 862,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 224366,
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
          "id": "0df74ff98e7b1933376c403b745f8a3240a024fe",
          "message": "feat(newsletter): mute/unmute channel notifications (#757)",
          "timestamp": "2026-06-08T07:47:04-03:00",
          "tree_id": "3f2437bf55b7387e85956ffd0a2a0acaa82a7fe0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0df74ff98e7b1933376c403b745f8a3240a024fe"
        },
        "date": 1780915868613,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8191,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2834469,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 335,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 190,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37056,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 252,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 50062,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 754,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 295601,
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
            "value": 78934,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1067,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 263592,
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
          "id": "74b1916e598f39c1b504dee23d8010c94f508851",
          "message": "fix(appstate): don't swallow external-blob download failures (#759)",
          "timestamp": "2026-06-08T08:13:42-03:00",
          "tree_id": "10e10b9a1161dff6875356725e4ee486473afe02",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/74b1916e598f39c1b504dee23d8010c94f508851"
        },
        "date": 1780917481575,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8203,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2843592,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 247,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 45634,
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
            "value": 52581,
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
            "value": 60114,
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
            "value": 78701,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1065,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 261467,
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
          "id": "db62f7a0f0f3e86c881a9a4a69122fd5c616eeac",
          "message": "feat(events): create and respond (RSVP) API (#758)",
          "timestamp": "2026-06-08T08:13:56-03:00",
          "tree_id": "4927004afd43353217d61a90fab514bf57c29baf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/db62f7a0f0f3e86c881a9a4a69122fd5c616eeac"
        },
        "date": 1780917535139,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8312,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2868100,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 269,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 49901,
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
            "value": 55482,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 281,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 58892,
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
            "value": 79580,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1088,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 263560,
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
            "email": "55464917+jlucaso1@users.noreply.github.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ca9329401b618f9010d991c22810f56975274aed",
          "message": "feat(chat): support userStatusMute app-state action (incoming + outgoing) (#760)",
          "timestamp": "2026-06-08T08:34:12-03:00",
          "tree_id": "c79dd58e3669a257d8992c81400b6a63dfed9ff8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ca9329401b618f9010d991c22810f56975274aed"
        },
        "date": 1780918709071,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8284,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2865270,
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
            "value": 42063,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 277,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57476,
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
            "value": 89226,
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
            "value": 78791,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 860,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 220050,
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
          "id": "0dc762dbf8d10d628d93a4792fa2a23b6115324a",
          "message": "feat(edit): support message-secret encrypted edits (secret_encrypted_message) (#762)",
          "timestamp": "2026-06-08T09:57:52-03:00",
          "tree_id": "dcdb2137e66783238ede485f5c0a041bc52665f9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0dc762dbf8d10d628d93a4792fa2a23b6115324a"
        },
        "date": 1780923703910,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8258,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2850064,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 218,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42871,
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
            "value": 63910,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 508,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 92406,
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
            "value": 78809,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 861,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 224248,
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
          "id": "015c99cdce5eb8ec3f681d49fccf320cbc092ae4",
          "message": "fix(groups): keep persisted group metadata in sync on membership change (#761)",
          "timestamp": "2026-06-08T10:22:35-03:00",
          "tree_id": "83265fa2907cf358e19a79a9a8a3d9d7fd5ad0e7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/015c99cdce5eb8ec3f681d49fccf320cbc092ae4"
        },
        "date": 1780925230881,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8279,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2809720,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 384,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 234,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42376,
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
            "value": 62104,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 522,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 92808,
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
            "value": 78801,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 864,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 224267,
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
          "id": "2ba94cf13ab7b2ae545bf73d2a598f8c95732247",
          "message": "perf(portable-cache): O(log n) remove_key instead of O(n) insertion scan (#763)",
          "timestamp": "2026-06-08T10:48:02-03:00",
          "tree_id": "553492a3bbbe704403b59c8851f665f20a74c820",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2ba94cf13ab7b2ae545bf73d2a598f8c95732247"
        },
        "date": 1780926644498,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8236,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2806066,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 237,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42745,
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
            "value": 57295,
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
            "value": 69071,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 359,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78428,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 879,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 226284,
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
          "id": "d64e10c038a1ed6c87440b189dd543d301596182",
          "message": "perf(cache): raise device_registry_cache default capacity 1000 -> 5000 (#765)",
          "timestamp": "2026-06-08T10:58:50-03:00",
          "tree_id": "2b1b80bd9b078d7e471ad28ac07f33542f3ef146",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d64e10c038a1ed6c87440b189dd543d301596182"
        },
        "date": 1780927352924,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8210,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2852533,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 271,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 49568,
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
            "value": 58877,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 425,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 80320,
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
            "value": 79230,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 834,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 213816,
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
          "id": "2239ee82770778b18663f73cbc7fa9122fc56094",
          "message": "feat(media): high-level media message builders from UploadResponse (#764)",
          "timestamp": "2026-06-08T11:01:43-03:00",
          "tree_id": "40e7d4307cf2f7aebff3c58e412b0135a08f3309",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2239ee82770778b18663f73cbc7fa9122fc56094"
        },
        "date": 1780927531454,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8259,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2856942,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 241,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47112,
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
            "value": 64039,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 380,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 72265,
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
            "value": 78279,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 832,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215774,
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
          "id": "a774e8a4bfdea347b875aa6eff9b0ac65348a9fd",
          "message": "refactor(groups): get_participating returns HashMap<Jid, GroupMetadata> (#767)",
          "timestamp": "2026-06-08T11:19:57-03:00",
          "tree_id": "4dacedf71f8580c83cd767f804e5e99fefde0e9a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a774e8a4bfdea347b875aa6eff9b0ac65348a9fd"
        },
        "date": 1780928613740,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8307,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2834724,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 290,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 52260,
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
            "value": 63299,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 283,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 58684,
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
            "value": 79229,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1107,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 270897,
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
          "id": "baf82cb89a928aa80f4a4a55c1aeccf9b8e602eb",
          "message": "refactor(download): take DownloadParams struct instead of 6 positional args (#768)",
          "timestamp": "2026-06-08T11:30:08-03:00",
          "tree_id": "6549bfdb905ac791ff19e527e537b23d92bafa10",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/baf82cb89a928aa80f4a4a55c1aeccf9b8e602eb"
        },
        "date": 1780929243187,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8307,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2757127,
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
            "value": 36917,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 326,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 64899,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 375,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 72148,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 359,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78308,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 955,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 242631,
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
          "id": "13a77a3ce67fb429d63c7a38410c8a2a70601bb6",
          "message": "fix(appstate): clear stale mutation MACs on snapshot re-sync (#766)",
          "timestamp": "2026-06-08T11:44:40-03:00",
          "tree_id": "e16685f0b5e5cf0b951beee10f9b8495933b87cc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/13a77a3ce67fb429d63c7a38410c8a2a70601bb6"
        },
        "date": 1780930165257,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8150,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2777900,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 339,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 192,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37124,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 306,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 60380,
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
            "value": 59491,
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
            "value": 78825,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 862,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 224284,
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
          "id": "a5dd94bff65f086b9f0876dedd8ab196f491f9f8",
          "message": "refactor(download): remove dead, full-buffering download_to_file (#770)",
          "timestamp": "2026-06-08T12:05:24-03:00",
          "tree_id": "44e62f08c60762e49b5152b13eae5197275c3da8",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a5dd94bff65f086b9f0876dedd8ab196f491f9f8"
        },
        "date": 1780931352190,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8307,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2852538,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 214,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41955,
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
            "value": 61021,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 302,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 59674,
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
            "value": 79190,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 834,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215835,
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
          "id": "7b443fdd25f86a8ed95a046dc4327538ef9b03cb",
          "message": "fix(appstate): validate index MAC even when the decrypted index field is absent (#769)",
          "timestamp": "2026-06-08T12:05:55-03:00",
          "tree_id": "aaf9f6877e4de95290cc68f327b1ff5c67a1f94c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7b443fdd25f86a8ed95a046dc4327538ef9b03cb"
        },
        "date": 1780931463943,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8290,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2799745,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 220,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41079,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 215,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 42581,
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
            "value": 89628,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 357,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78440,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1061,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 260756,
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
          "id": "0f62afe90787e8cd0419c1875d910de5ce24d10c",
          "message": "refactor(polls): group enc_payload + enc_iv into PollVoteCiphertext (#771)",
          "timestamp": "2026-06-08T12:30:26-03:00",
          "tree_id": "b16c7f4ddf04d8f6b574168d4483c055824e7b5e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0f62afe90787e8cd0419c1875d910de5ce24d10c"
        },
        "date": 1780932909128,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8276,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2799864,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 184,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35730,
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
            "value": 53964,
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
            "value": 58470,
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
            "value": 79026,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1020,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 258053,
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
          "id": "db753be5144318f51c6a5b9eb498ca59711d95d0",
          "message": "feat(prekeys): validate companion device-identity (ADV) on fetched bundles (#772)",
          "timestamp": "2026-06-08T12:42:47-03:00",
          "tree_id": "ac4d4c82f37f6ef19da5c32a412c6e667966660e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/db753be5144318f51c6a5b9eb498ca59711d95d0"
        },
        "date": 1780933636256,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8367,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2819202,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 210,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40810,
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
            "value": 64302,
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
            "value": 91932,
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
            "value": 79523,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1082,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 260036,
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
          "id": "97dff1167a78ac0f2b2a14a432099835bf87942d",
          "message": "refactor(api): consistent, alloc-aware message-id param types (#775)",
          "timestamp": "2026-06-08T13:25:03-03:00",
          "tree_id": "f6bdda4ddb89df959360edfe67fd6541ae4fd1db",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/97dff1167a78ac0f2b2a14a432099835bf87942d"
        },
        "date": 1780936146384,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8217,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2844732,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 235,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42444,
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
            "value": 61631,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 443,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 81515,
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
            "value": 78777,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 860,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 220178,
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
          "id": "7233f069e56370d0e987ca709b3401be87524bfc",
          "message": "perf(upload): slice ciphertext zero-copy instead of copying per attempt (#774)",
          "timestamp": "2026-06-08T13:30:21-03:00",
          "tree_id": "9d0cb07e7dbdc4abe83fa455fe3e6263ac5cf1fa",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7233f069e56370d0e987ca709b3401be87524bfc"
        },
        "date": 1780936515049,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8302,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2852537,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 341,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 216,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 45125,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 264,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 54375,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 294,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 59794,
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
            "value": 79208,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1072,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 267739,
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
          "id": "f1a8dff02320b71ab768d84c30d79f363ae71119",
          "message": "fix(appstate): re-sync unsynced collection that gets patches without a snapshot (#773)",
          "timestamp": "2026-06-08T13:38:55-03:00",
          "tree_id": "5c409068262db4ea6528916c296a25d9cc576de7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f1a8dff02320b71ab768d84c30d79f363ae71119"
        },
        "date": 1780937024803,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8275,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2857469,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 223,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43918,
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
            "value": 59465,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 373,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 72017,
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
            "value": 78837,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 831,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 213691,
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
          "id": "b66b1ee138d4a48be4ab85f4fb9df34807fd5be7",
          "message": "feat(usync): surface device list from get_user_info (#776)",
          "timestamp": "2026-06-08T13:56:35-03:00",
          "tree_id": "7533b12b4ae5df81aaae734b9f4dea3bc5ec7d52",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b66b1ee138d4a48be4ab85f4fb9df34807fd5be7"
        },
        "date": 1780938071179,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8268,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2799588,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 240,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 46946,
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
            "value": 63451,
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
            "value": 79906,
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
            "value": 79356,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 835,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215933,
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
          "id": "5e7213e69cb4b85009d45fd848a56cfcc57a6070",
          "message": "fix(receipt): chunk read/played receipts into 256 ids per stanza (#777)",
          "timestamp": "2026-06-08T14:06:36-03:00",
          "tree_id": "7897e2a737731e1fc9641d96ee1f770b155fdd9d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5e7213e69cb4b85009d45fd848a56cfcc57a6070"
        },
        "date": 1780938623934,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8107,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2823774,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 338,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 240,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 44556,
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
            "value": 47766,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 401,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 77162,
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
            "value": 78431,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 861,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 222254,
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
          "id": "26c6d00eeecde7591f8c03d0231dc465984b23f3",
          "message": "refactor(groups): set_description prev takes Option<&str> (#778)",
          "timestamp": "2026-06-08T14:06:47-03:00",
          "tree_id": "f6ca35ff242c0685f47bd907290c63b171e50582",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/26c6d00eeecde7591f8c03d0231dc465984b23f3"
        },
        "date": 1780938736635,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8218,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2860328,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 341,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 198,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 38404,
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
            "value": 63126,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 386,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 73119,
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
            "value": 78877,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1108,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 267036,
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
      }
    ]
  }
}