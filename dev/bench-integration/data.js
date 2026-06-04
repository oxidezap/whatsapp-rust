window.BENCHMARK_DATA = {
  "lastUpdate": 1780590092625,
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
          "id": "88a0fe074fdc09d73be95cdd23743fba22ef172e",
          "message": "feat(groups): return full GroupMetadata from create_group / community.create (#615)",
          "timestamp": "2026-05-05T16:23:34-03:00",
          "tree_id": "51ef86f6dd0a8c9f3572fa800617baff75b0987e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/88a0fe074fdc09d73be95cdd23743fba22ef172e"
        },
        "date": 1778009144981,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9740,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3270341,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 432,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 261,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 52006,
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
            "value": 58709,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 396,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 109914,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 453,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117973,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 23,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1420,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 415824,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 49,
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
          "id": "65fb66ac56f4563c0997c34892fefc860d29b6fa",
          "message": "feat(message_edit): decrypt secretEncryptedMessage MESSAGE_EDIT envelope (#618)",
          "timestamp": "2026-05-11T10:54:07-03:00",
          "tree_id": "91e9d90dfa6f9ac627300353961ed58fbc7e0523",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/65fb66ac56f4563c0997c34892fefc860d29b6fa"
        },
        "date": 1778507786957,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9780,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3261430,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 390,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 260,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 51861,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 340,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 78513,
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
            "value": 109720,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 450,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 118074,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 29,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1418,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 415705,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 49,
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
          "id": "b2612d17c3ee2980f997a6b32affd63bc4c2f97f",
          "message": "chore(release): bump workspace to 0.6.0 (#619)",
          "timestamp": "2026-05-11T11:06:48-03:00",
          "tree_id": "75d7a205071747ea08d06a1192114d66fc10b2bc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b2612d17c3ee2980f997a6b32affd63bc4c2f97f"
        },
        "date": 1778508541694,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9779,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3330685,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 391,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 255,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 51227,
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
            "value": 58699,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 409,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 113066,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 455,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120261,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 51,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1419,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 415416,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 49,
            "unit": "milliseconds"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "fcc973995b056128434af9c2e4d733bb1cbb1959",
          "message": "chore: update all deps",
          "timestamp": "2026-05-11T11:10:49-03:00",
          "tree_id": "766aaca067131f33b853edd277b98c1f58059ad7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fcc973995b056128434af9c2e4d733bb1cbb1959"
        },
        "date": 1778509121780,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9735,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3328183,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 394,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1297,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 133957,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 362,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 87478,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1194,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 149510,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 454,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 119270,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 35,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1420,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 415512,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 49,
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
          "id": "56ed1b09fd1b5d140ec1edfc5e7f66203014a9a6",
          "message": "ci(release): install cargo-release via taiki-e action (#620)",
          "timestamp": "2026-05-11T11:26:39-03:00",
          "tree_id": "5021191c56b786cf899532fe5ce83cd242e3204c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/56ed1b09fd1b5d140ec1edfc5e7f66203014a9a6"
        },
        "date": 1778509704608,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9730,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3260057,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 395,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1317,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 136298,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 337,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 80432,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1171,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 148085,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 455,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120483,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 51,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1324,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 406935,
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
          "id": "358de21b8497e41b41a75eb4f5f24b05fb34c928",
          "message": "fix(send): admin revoke not applied on recipient devices (#621)\n\nCo-authored-by: Salientekill <Salientekill@users.noreply.github.com>\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-05-13T19:16:44-03:00",
          "tree_id": "77abc6a6dbf6ca71b02fa7acf7591b988b1ff26f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/358de21b8497e41b41a75eb4f5f24b05fb34c928"
        },
        "date": 1778710723624,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 10016,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3378293,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1070,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 91474,
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
            "value": 69400,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1246,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 163722,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 463,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 121878,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 55,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1421,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 415687,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 48,
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
          "id": "e52b2ec33d5e9dda6ecb278720dbecac136f633b",
          "message": "audit: WA Web protocol compliance fixes (#623)",
          "timestamp": "2026-05-13T21:55:16-03:00",
          "tree_id": "16a4dbe01232d8ee7d81dce3a56504d4593c3064",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e52b2ec33d5e9dda6ecb278720dbecac136f633b"
        },
        "date": 1778720249930,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 10399,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3421561,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1155,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 100373,
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
            "value": 51190,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1281,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 166432,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 475,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 122843,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 47,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1607,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 442819,
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
          "id": "8ecce8b9c27e045ab821b1de112f2d5cac8ab16f",
          "message": "fix: more WA Web protocol compliance fixes (#624)",
          "timestamp": "2026-05-14T10:15:56-03:00",
          "tree_id": "d45af787008645b1b404f57203831918dfdd1860",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8ecce8b9c27e045ab821b1de112f2d5cac8ab16f"
        },
        "date": 1778764669895,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9509,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3148607,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 308,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1272,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 139287,
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
            "value": 52835,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1701,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 226336,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 1,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 460,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120495,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1701,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 502948,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 49,
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
          "id": "946efd7812325271a0360cb20eb12caad4611589",
          "message": "fix(groups): correct change_number stanza shape (#625)",
          "timestamp": "2026-05-14T11:18:28-03:00",
          "tree_id": "3f2696be5705ec7f6dbb4327d0886b8ba6aa82f6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/946efd7812325271a0360cb20eb12caad4611589"
        },
        "date": 1778768443645,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9532,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3143186,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 309,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1267,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 135083,
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
            "value": 53393,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1681,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 235197,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 447,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117987,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1826,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 558040,
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
          "id": "07216cfbd0bc67861813abb88ec1a05c0b17e35b",
          "message": "feat(groups): capture display_name on participant info (#626)",
          "timestamp": "2026-05-14T11:25:35-03:00",
          "tree_id": "6344df0e1bd1b50dc87e48f2ff17336edca92214",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/07216cfbd0bc67861813abb88ec1a05c0b17e35b"
        },
        "date": 1778768873333,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 10142,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3311955,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 310,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1474,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 148449,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 349,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 80272,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1565,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 200612,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 462,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120394,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1543,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 439453,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 14,
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
          "id": "4de0d3136dc5831ade5fd129667ff2e9755568f0",
          "message": "fix: align inbound parsing with WA Web for receipts and messages (#627)",
          "timestamp": "2026-05-14T13:20:35-03:00",
          "tree_id": "a302bc4d531c75cfac524a0f8c4517db730d5fa2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4de0d3136dc5831ade5fd129667ff2e9755568f0"
        },
        "date": 1778775763177,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9843,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3290458,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 311,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1026,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 96895,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 430,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 113394,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1709,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 216106,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 466,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 121673,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1864,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 563110,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 49,
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
          "id": "f9a937613c4ed323793e3254c90bf8705f4b8565",
          "message": "fix(send): emit correct <biz>/<bot> stanza for native-flow buttons (#628)",
          "timestamp": "2026-05-14T14:00:10-03:00",
          "tree_id": "9c048a6a6104c19203837841fbef570287e1ad02",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f9a937613c4ed323793e3254c90bf8705f4b8565"
        },
        "date": 1778778193582,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9579,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3148326,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 309,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1008,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 120310,
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
            "value": 51596,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1516,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 203556,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 463,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120620,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1596,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 494027,
            "unit": "bytes"
          },
          {
            "name": "integration::reconnect::wall_ms",
            "value": 49,
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
          "id": "c0c2a4a589ec76426b98171d02a93e0f7c91688e",
          "message": "fix(offline): drive WA Web pull-batch loop for offline backlog (#629)",
          "timestamp": "2026-05-15T10:38:29-03:00",
          "tree_id": "42126df3d19c60eb820635321dee2de8e8a241ef",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c0c2a4a589ec76426b98171d02a93e0f7c91688e"
        },
        "date": 1778852418140,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9480,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3008852,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1103,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 99272,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 408,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 104112,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1331,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 168818,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 465,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 127850,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 947,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 212796,
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
          "id": "8b11141bbfe21c6c9108d9e0fc5ae867454194b9",
          "message": "fix: zombie connection after stream-error 500 (ClientPayload + receipt compliance) (#630)",
          "timestamp": "2026-05-16T15:02:33-03:00",
          "tree_id": "dc33bfdc230b636e5f6f4ccb442a6470f19f4f42",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8b11141bbfe21c6c9108d9e0fc5ae867454194b9"
        },
        "date": 1778954681825,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9328,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3041107,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1058,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 90801,
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
            "value": 68510,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1381,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 180964,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 461,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 126671,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1206,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 253026,
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
          "id": "20f1ce86acce3847dff999969d5fbc7169919bab",
          "message": "fix(message): nack unrecoverable decrypt errors instead of silent drop (#631)",
          "timestamp": "2026-05-17T12:32:31-03:00",
          "tree_id": "21b2c286489fc582dfa92911c5690a1f7bb2d27e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/20f1ce86acce3847dff999969d5fbc7169919bab"
        },
        "date": 1779032073538,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9300,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3038824,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 351,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1130,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 106620,
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
            "value": 70952,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1196,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 149898,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 464,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 127264,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 989,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 215010,
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
            "email": "sasha@micra.io",
            "name": "Sasha Alyushin",
            "username": "alexandme"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f39fc2a28207622710066635b5ad602682096ac8",
          "message": "fix(wacore): drop if-let guard so the pushname arm builds on Rust 1.93 (#632)",
          "timestamp": "2026-05-19T13:03:24-03:00",
          "tree_id": "b93e7184959d0d904326336251a4f4b12852d328",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f39fc2a28207622710066635b5ad602682096ac8"
        },
        "date": 1779206735480,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9693,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3159058,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1153,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 107649,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 379,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 91571,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1273,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 161329,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 461,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 128517,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1255,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 262829,
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
          "id": "2d33b687e5cf874814267e1e35cd78012fc95a6d",
          "message": "fix: self-DM stops working, WA Web compliance for fanout and BadMac (#634)",
          "timestamp": "2026-05-19T19:07:33-03:00",
          "tree_id": "9b1dad1703d852ba3cf3cc7d7ca14ebde04a3886",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2d33b687e5cf874814267e1e35cd78012fc95a6d"
        },
        "date": 1779228569908,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 9323,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3064736,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1064,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 91576,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 350,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 81748,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 1198,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 148998,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 463,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 127368,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 989,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 214984,
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
          "id": "bfc67fba98b41e1da368a1d82cd12da84ab926f1",
          "message": "fix: self-DM/sibling decryption deadlock (retry shape + session recovery + peer pkmsg identity) (#635)",
          "timestamp": "2026-05-20T22:16:49-03:00",
          "tree_id": "4590790186da967b465ebc5cd8b356f8da1b8c57",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bfc67fba98b41e1da368a1d82cd12da84ab926f1"
        },
        "date": 1779326343616,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12974,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3832499,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 351,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1944,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 314128,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 381,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 87557,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3451,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 580266,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 524,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 135752,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1215,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 256561,
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
            "email": "zizuukw@gmail.com",
            "name": "Zaidan Yusuf Akbar",
            "username": "kkzaadev"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0259de875dbd50f6e53bf01a6e1715a67b296e4a",
          "message": "fix(client): preserve recipient in <ack>, soften unknown stream:error (#633)\n\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-05-21T11:09:11-03:00",
          "tree_id": "aa9c7d6e6f3f6388ab9860013c1f3733c8ada4cb",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0259de875dbd50f6e53bf01a6e1715a67b296e4a"
        },
        "date": 1779372681612,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12999,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3783955,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 353,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1743,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 280612,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 424,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 105027,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4487,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 739640,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 474,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 128355,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 992,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217493,
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
          "id": "6a8abbd4b23344538b485a501e65e8c940b821af",
          "message": "fix(prekeys): re-upload after re-pair to heal stale server bundle (#641)\n\nCo-authored-by: Salientekill <Salientekill@users.noreply.github.com>\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-05-27T11:25:58-03:00",
          "tree_id": "3bfbbb9c51ca316d6ea2ba7fe9434ac2fe9af796",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/6a8abbd4b23344538b485a501e65e8c940b821af"
        },
        "date": 1779892074645,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13003,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3786627,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1953,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 311367,
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
            "value": 51404,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4629,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 754871,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 474,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 128689,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1203,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 254851,
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
            "email": "oon.arfiandwi@gmail.com",
            "name": "oon arfiandwi",
            "username": "oonid"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "cbcdd2a6b931d804f3c2693554dce75e654834e3",
          "message": "fix(send): align own devices to LID namespace for LID-addressed DMs (#636)\n\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-05-27T11:30:21-03:00",
          "tree_id": "58760ee94adb1766c7895ee7b23a800c486c5fcd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cbcdd2a6b931d804f3c2693554dce75e654834e3"
        },
        "date": 1779892349201,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12960,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3833144,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 355,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1585,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 248492,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 371,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 92504,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3560,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 593183,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 473,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 122417,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 989,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217338,
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
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "f553ecdd70cbe2e469ad86f89f1ec1cd3a71404d",
          "message": "ci: drop pull_request_target from e2e and claude review workflows\n\npull_request_target exposed base-repo secrets and a write token to\nfork-triggered runs. e2e additionally checked out and ran fork code, the\nclassic pwn-request vector. Both now use pull_request (fork runs get a\nread-only token and no secrets) and are gated to same-repo PRs.",
          "timestamp": "2026-05-27T11:44:29-03:00",
          "tree_id": "5d167b05b1d24309dd20dedf38a1998f4db55d01",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f553ecdd70cbe2e469ad86f89f1ec1cd3a71404d"
        },
        "date": 1779893278294,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13045,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3793974,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 351,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1540,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 244871,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 218,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 39145,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3947,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 650891,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 461,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120724,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1262,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 266543,
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
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "89bba0a208cc36e5141874fc1c47cccc020e07e2",
          "message": "chore(deps): bulk-update dependencies via cargo update\n\nSupersedes the open Dependabot PRs in one commit:\n- serde_json 1.0.149 -> 1.0.150 (#640)\n- log 0.4.29 -> 0.4.30 (#639)\n- cbc 0.2.0 -> 0.2.1 (#638)\n- http 1.4.0 -> 1.4.1 (#637)\n- libsqlite3-sys 0.36 -> 0.37 (#622)\n\nAlso pulls compatible transitive bumps (aes-gcm rc.4, aes, cipher,\ncrypto-common, diesel 2.3.9, wasm-bindgen, winnow, etc.). libsqlite3-sys\nand http manifest pins bumped to match. clippy --all-targets -D warnings\nand the non-e2e test suite pass.",
          "timestamp": "2026-05-27T12:01:20-03:00",
          "tree_id": "3d9a78f6f5e8468195ac2665e91840633ea52435",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/89bba0a208cc36e5141874fc1c47cccc020e07e2"
        },
        "date": 1779894311136,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13107,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3800179,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1119,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 184428,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 449,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 107329,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3320,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 552603,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 496,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 126314,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 991,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217384,
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
          "id": "f697e91e9e15c4b3ed9c8af4bf69497d9f7a1c58",
          "message": "fix(polls): align poll vote encryption/decryption to conversation addressing (#642)",
          "timestamp": "2026-05-27T14:27:02-03:00",
          "tree_id": "70c15cc23cc97619b2a406edff1a84837d333f8f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f697e91e9e15c4b3ed9c8af4bf69497d9f7a1c58"
        },
        "date": 1779902938028,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13085,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3739560,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 351,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1721,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 274195,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 381,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 95541,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4564,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 747544,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 430,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 116154,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 999,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 218719,
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
          "id": "8a3487c8f44de54e160a9a076120bccd1e5b9240",
          "message": "feat(history-sync): server-error receipt to request blob re-upload (#643)",
          "timestamp": "2026-05-27T14:46:15-03:00",
          "tree_id": "96fef8b6f3e1e6e29f1ec7d090bb871c35468d6e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8a3487c8f44de54e160a9a076120bccd1e5b9240"
        },
        "date": 1779904104759,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13372,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3904713,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1542,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 241427,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 382,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 94961,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4593,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 748685,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 430,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 116221,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 990,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217501,
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
          "id": "4644dcd47b317f35d0865b111f430be974830e70",
          "message": "feat(stickers): fetch first-party sticker pack data from the CDN (#644)",
          "timestamp": "2026-05-27T15:27:07-03:00",
          "tree_id": "59255dca8cdc8eaa0c293e0f6b9321b11ba8f6bc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4644dcd47b317f35d0865b111f430be974830e70"
        },
        "date": 1779906550599,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13361,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3900617,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 351,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 2304,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 363188,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 361,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 88378,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3304,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 553543,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 487,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 124713,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1009,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 219114,
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
          "id": "1ace7c7f3b3ffcb80dd59d3caf1340a5f4395d36",
          "message": "feat(secret-encrypted): decrypt poll-edit / poll-add-option / event-edit envelopes (#645)",
          "timestamp": "2026-05-27T15:36:36-03:00",
          "tree_id": "fafcc02456870f14985b2f85408ae42f9e6c50c5",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1ace7c7f3b3ffcb80dd59d3caf1340a5f4395d36"
        },
        "date": 1779907127098,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13075,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3794021,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1406,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 227305,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 363,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 79028,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3058,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 511250,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 498,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 126334,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 989,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217399,
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
          "id": "ddc13d17bb24b604e9df5daaf0824de2911eddbf",
          "message": "fix(security): only honor self-only protocol messages from our own account (#646)",
          "timestamp": "2026-05-27T15:58:12-03:00",
          "tree_id": "b5dca565c0e0ee0be1e5ebcb096902a23d614078",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ddc13d17bb24b604e9df5daaf0824de2911eddbf"
        },
        "date": 1779908407333,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12964,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3833621,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1661,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 259969,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 377,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 94870,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3124,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 521186,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 495,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 125908,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1244,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 264250,
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
          "id": "ebb9faf39874f124c5dae059531d18b67717c56e",
          "message": "fix(offline): drain offline queue by acking duplicate and undecryptable messages (#647)",
          "timestamp": "2026-05-27T21:55:27-03:00",
          "tree_id": "3855a7e4d19d53ab27f4817817cb8cfd9dbb9e03",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ebb9faf39874f124c5dae059531d18b67717c56e"
        },
        "date": 1779929835125,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13074,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3801165,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 357,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1396,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 218471,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 405,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 96853,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4368,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 712100,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 434,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117234,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1260,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 265544,
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
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "dfc0dcb4b463da0a838b4b8276e5e629eb24da55",
          "message": "chore: update Claude model version to claude-opus-4-7 in workflows",
          "timestamp": "2026-05-28T10:39:27-03:00",
          "tree_id": "c739bd7d4a3b576fd5a98b2cdd159d6833b1fcc1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/dfc0dcb4b463da0a838b4b8276e5e629eb24da55"
        },
        "date": 1779975690843,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13040,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3746531,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1639,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 258701,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 346,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 80743,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 2984,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 504449,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 513,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 128949,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 35,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 991,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217385,
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
          "id": "b7bdacf175b61a1a71590474cdf90841c84b5f11",
          "message": "fix(receipt): preserve sender device in delivery receipt `to` (#649)",
          "timestamp": "2026-05-28T10:51:51-03:00",
          "tree_id": "006e878b455666ed2464b5b26b4b15dfb0fded92",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b7bdacf175b61a1a71590474cdf90841c84b5f11"
        },
        "date": 1779976434828,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13029,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3777014,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 341,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1184,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 198069,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 392,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 98558,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4096,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 678964,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 455,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 119556,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1285,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 273925,
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
          "id": "f7a8afbf42416ad6356a37c83d17dca5c28adcb3",
          "message": "feat(msmsg): decrypt Meta AI / fbid bot replies (`<enc type=\"msmsg\">`) (#650)",
          "timestamp": "2026-05-28T17:42:43-03:00",
          "tree_id": "5d9d82a881539090e4cb0b6b241bb435d1d6f7cc",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f7a8afbf42416ad6356a37c83d17dca5c28adcb3"
        },
        "date": 1780001087192,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13139,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3817279,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 353,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1468,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 238065,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 409,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 103177,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3389,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 565978,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 489,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 124680,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 988,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217575,
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
          "id": "46c4094065b9dd1abd244fcdd281960de2426da0",
          "message": "fix(runtime): introduce Spawnable trait for WASM compatibility (#651)",
          "timestamp": "2026-05-28T18:38:59-03:00",
          "tree_id": "14fa918491daee94049b4fce7d520ace7aab0f18",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/46c4094065b9dd1abd244fcdd281960de2426da0"
        },
        "date": 1780004462948,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13144,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3867627,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 355,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 882,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 145413,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 396,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 98863,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3252,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 543830,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 497,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 125811,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 990,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217912,
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
          "id": "4c86f27ba25bdd8866d0ea3dbcf4022bd4443393",
          "message": "fix(messages): pad to uniform 1..=16 bytes matching WA Web (#653)",
          "timestamp": "2026-05-28T19:21:07-03:00",
          "tree_id": "acf5af9f9f25453819f9cbd633d668caad23b0f9",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4c86f27ba25bdd8866d0ea3dbcf4022bd4443393"
        },
        "date": 1780006997880,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13088,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3744856,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 2138,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 335851,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 400,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 101652,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3843,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 633550,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 474,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 122626,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1010,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 220551,
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
          "id": "cce5afd59b3bd4cdfaf496c43894bb0dd517d041",
          "message": "fix(send): don't hide decrypt-fail on SenderRevoke (#656)",
          "timestamp": "2026-05-28T19:42:27-03:00",
          "tree_id": "9edaf317134c15a747f9de186fd8ada03bbbde82",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cce5afd59b3bd4cdfaf496c43894bb0dd517d041"
        },
        "date": 1780008268437,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12975,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3833042,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 358,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1263,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 203152,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 224,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 39403,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4567,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 742375,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 440,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117915,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 982,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217445,
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
          "id": "e385f982e6135c15c6ff688ce2dde5d7ae201862",
          "message": "fix(send): hide decrypt-fail for conditional-reveal and poll-add-option (#655)",
          "timestamp": "2026-05-28T19:42:44-03:00",
          "tree_id": "f1f67380cde9f9ad21ac02c6cd9ff574c293ff25",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e385f982e6135c15c6ff688ce2dde5d7ae201862"
        },
        "date": 1780008281705,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13056,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3845169,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1667,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 264095,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 431,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 104503,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4578,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 744104,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 439,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117348,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 989,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217816,
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
          "id": "991a72f241a52af9d8a394803971ebb74ecb044b",
          "message": "fix(send): classify poll-add-option as poll and album as text (#654)",
          "timestamp": "2026-05-28T19:43:05-03:00",
          "tree_id": "526ad5519fef4f830aa279ef9b19289ef0c04655",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/991a72f241a52af9d8a394803971ebb74ecb044b"
        },
        "date": 1780008328564,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13142,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3863541,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 354,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1275,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 209245,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 405,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 102673,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4392,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 718337,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 443,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 118190,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 992,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217989,
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
          "id": "c786a33f1a5d64fbea3f261246f6b09a7a2a6a2e",
          "message": "fix(send): fail DM send when every per-device encrypt fails (#652)",
          "timestamp": "2026-05-28T19:43:30-03:00",
          "tree_id": "a25be7fa0ca981484c444e610cacc0c05c59f414",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c786a33f1a5d64fbea3f261246f6b09a7a2a6a2e"
        },
        "date": 1780008356841,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13090,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3845729,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1673,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 264646,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 416,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 101100,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4589,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 738318,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 440,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117326,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 988,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217712,
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
          "id": "57ce9a9dd8fe47fba64da8f3f8244622df96f9f0",
          "message": "fix(send): serialize the group sender-key chain per (group, sender) (#657)",
          "timestamp": "2026-05-28T20:50:26-03:00",
          "tree_id": "e2d2ffefa4472239612d9d5d763a0fd7f171da6f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/57ce9a9dd8fe47fba64da8f3f8244622df96f9f0"
        },
        "date": 1780012355843,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13391,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3913911,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1569,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 246658,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 390,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 96013,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3147,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 522975,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 503,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 126791,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1256,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 265737,
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
          "id": "7fbf220c61c5fca64e1a24e868dd37b817ae9d76",
          "message": "fix(offline): clear self-fanout with sender receipt, not a bare ack (#659)",
          "timestamp": "2026-05-29T12:46:13-03:00",
          "tree_id": "915682e404a8bc00484752353c49f3cc533c4447",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7fbf220c61c5fca64e1a24e868dd37b817ae9d76"
        },
        "date": 1780069702837,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13099,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3857564,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 354,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 2401,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 377102,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 331,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 77934,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3511,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 586047,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 485,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 123874,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 990,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217930,
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
          "id": "971dbecb3c3acc419d0133f60dc1108d3149199e",
          "message": "perf(retry): replace session-recreate Mutex with a TTL cache, atomic per-peer check+stamp (#658)",
          "timestamp": "2026-05-29T13:34:21-03:00",
          "tree_id": "dc058b52c4d0584e681f36b06ac682933c351bff",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/971dbecb3c3acc419d0133f60dc1108d3149199e"
        },
        "date": 1780072575651,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13034,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3881777,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 354,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 2313,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 367677,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 397,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 99208,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3695,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 613954,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 477,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 123294,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 989,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217875,
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
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "0cbc8a2578251704c0e9837fd1d41adf16c034f2",
          "message": "chore: update all deps",
          "timestamp": "2026-05-29T13:51:18-03:00",
          "tree_id": "30c52331b092163a5c8e3ee33c544fe23715f92b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0cbc8a2578251704c0e9837fd1d41adf16c034f2"
        },
        "date": 1780073620183,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13199,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3900461,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 2729,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 426035,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 358,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 84205,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3356,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 559192,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 492,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 125017,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1235,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 264144,
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
          "id": "33aae51b0d23e7ea3de590fe720727b7364b529e",
          "message": "perf(appstate): drop per-operand allocations in LTHash fold and update_hash (#660)",
          "timestamp": "2026-05-29T15:46:31-03:00",
          "tree_id": "1471d1e718df4ffeeacc9cbd5485cf744fd61682",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/33aae51b0d23e7ea3de590fe720727b7364b529e"
        },
        "date": 1780080532276,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13068,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3836246,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 353,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1569,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 246411,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 397,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 100924,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3771,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 620251,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 478,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 123182,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1281,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 267609,
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
          "id": "60df0ad17acb6ce09aa004bae7f0456cabdd8e89",
          "message": "perf(jid): add consuming into_non_ad for owned send-path JIDs (#661)",
          "timestamp": "2026-05-29T15:46:43-03:00",
          "tree_id": "a21de78c7b32e04ed7ee7060822374a950f9a283",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/60df0ad17acb6ce09aa004bae7f0456cabdd8e89"
        },
        "date": 1780080538140,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13163,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3845841,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 342,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1520,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 241133,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 389,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 93632,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4583,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 743119,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 439,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117266,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1019,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 226380,
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
          "id": "3f9dfe8f7f007ffdab066668c7651f1effc6365c",
          "message": "perf(send): build encrypt-task Signal address from a borrow, not a Jid clone (#662)",
          "timestamp": "2026-05-29T16:03:56-03:00",
          "tree_id": "ce8dc6caf17c9b2bee5624c2233875f7c0d168d6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3f9dfe8f7f007ffdab066668c7651f1effc6365c"
        },
        "date": 1780081576835,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13115,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3827810,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1463,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 230005,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 387,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 94307,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4583,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 746138,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 439,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117188,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1008,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 225226,
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
          "id": "4961b0e45daac64a2e066f463f06c01cd58883d7",
          "message": "feat(transport): surface the disconnect reason in logs (#663)",
          "timestamp": "2026-05-29T17:47:40-03:00",
          "tree_id": "346906234bb5c18a89275741da68845b94235977",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4961b0e45daac64a2e066f463f06c01cd58883d7"
        },
        "date": 1780087777175,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13000,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3863925,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 340,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 4232,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 658022,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 328,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 73362,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4444,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 722494,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 438,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117131,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1269,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 272924,
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
          "id": "379cc466ba79817f9dbb77cb948f7e9a28c5dfd1",
          "message": "fix(message): ack SKDM-only session decrypts (#664)",
          "timestamp": "2026-05-30T02:40:11-03:00",
          "tree_id": "2948f21726454241d1ad6ef28e1470e5196e2ddf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/379cc466ba79817f9dbb77cb948f7e9a28c5dfd1"
        },
        "date": 1780119722145,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13161,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3845951,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1096,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 179120,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 390,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 86682,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4466,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 731847,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 440,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 117370,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 992,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217832,
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
          "id": "21760101698d1395143ca94ed0f2f9a94623c8f1",
          "message": "fix(message)!: decrypt secret encrypted edits on receive (#665)",
          "timestamp": "2026-05-30T13:54:53-03:00",
          "tree_id": "b34e44a6d93b8d018f78d479b043ff3a89940aba",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/21760101698d1395143ca94ed0f2f9a94623c8f1"
        },
        "date": 1780160234056,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13057,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3888397,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1462,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 231048,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 346,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 81322,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4625,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 753693,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 447,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120974,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1198,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 255020,
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
          "id": "304d3325aaa5dbe6f7792d93245ffb425ee9c999",
          "message": "fix(send): set peer PDO push priority attrs (#666)",
          "timestamp": "2026-05-30T15:00:40-03:00",
          "tree_id": "54c7ef969384548c297ca950ca1e78483cbff348",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/304d3325aaa5dbe6f7792d93245ffb425ee9c999"
        },
        "date": 1780164170669,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13005,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3777788,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1165,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 187702,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 432,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 100806,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4609,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 749894,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 447,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120866,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 982,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217344,
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
          "id": "7183412fe060c9ab31cf44cc545cc6f9746886d7",
          "message": "feat(msg-secret)!: bound messageSecret retention by policy and event-time horizon (#668)",
          "timestamp": "2026-05-31T14:20:31-03:00",
          "tree_id": "c260fe464ade17ce0ef9012d0b83c85c36996a75",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7183412fe060c9ab31cf44cc545cc6f9746886d7"
        },
        "date": 1780248168662,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13122,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3798355,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 4286,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 663232,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 379,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 92932,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4452,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 726860,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 445,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 121398,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 989,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217833,
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
          "id": "b61dd9b44e95355152e650e3b9cd5252284398d6",
          "message": "perf(history-sync): free LazyHistorySync raw bytes after a successful decode (#669)",
          "timestamp": "2026-05-31T15:08:48-03:00",
          "tree_id": "d5eae448e194cc8312bb1d8a229893b83d2df404",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b61dd9b44e95355152e650e3b9cd5252284398d6"
        },
        "date": 1780251055653,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13119,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3837174,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1665,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 262328,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 400,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 100685,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3609,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 598484,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 486,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 127024,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1122,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 247400,
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
            "email": "zizuukw@gmail.com",
            "name": "Zaidan Yusuf Akbar",
            "username": "kkzaadev"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ba791cf78cef569d6618d91b3c7b6a9b2d85d12a",
          "message": "fix(message): decrypt incoming peer message edits (#667)\n\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-05-31T15:39:19-03:00",
          "tree_id": "3405dba86320307ff8807157596c213d7c27181e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ba791cf78cef569d6618d91b3c7b6a9b2d85d12a"
        },
        "date": 1780252863718,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13064,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3839529,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1772,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 275262,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 399,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 102220,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4616,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 752548,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 444,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 121063,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 987,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217745,
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
          "id": "714ea29424dc0af191efaaa8660d0b7c99b355bc",
          "message": "fix(connection)!: flush Signal cache on disconnect to stop SKDM re-fanout (#670)",
          "timestamp": "2026-05-31T15:39:54-03:00",
          "tree_id": "1a189efb9741047cd1aba0428066de6a8fe9108a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/714ea29424dc0af191efaaa8660d0b7c99b355bc"
        },
        "date": 1780252920794,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13081,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3896649,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1343,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 217111,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 389,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 96533,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 2787,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 476622,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 526,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 133228,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 988,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217757,
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
          "id": "3019778910c466c94df74a018b9f7c6e7cb5379f",
          "message": "ci(wasm): guard whatsapp-rust wasm32 build and fix two #668 regressions (#671)",
          "timestamp": "2026-05-31T22:34:47-03:00",
          "tree_id": "297d37a3b6e45ab7ebbea70c798248e09e70aac7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3019778910c466c94df74a018b9f7c6e7cb5379f"
        },
        "date": 1780277816303,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13214,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3920205,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1506,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 239407,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 370,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 89239,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 3520,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 584433,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 497,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 128839,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 990,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217959,
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
          "id": "495bd17fd81223ad3564cc0f7cb42833cc41fd49",
          "message": "perf: implement streaming decompression for history sync processing (#672)",
          "timestamp": "2026-06-01T08:07:41-03:00",
          "tree_id": "be1e1ba487a9899b36045b97966323e7d6327cac",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/495bd17fd81223ad3564cc0f7cb42833cc41fd49"
        },
        "date": 1780312204941,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 13135,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3847868,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1343,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 217150,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 442,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 104623,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4429,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 723130,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 444,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 120615,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1239,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 264205,
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
          "id": "8a933ec6480ac6417537d9a620eec85b51d53c49",
          "message": "perf(message): cut per-message allocation churn on the hot path (#673)",
          "timestamp": "2026-06-01T10:04:01-03:00",
          "tree_id": "4f94403bf619afe35b2401211afe505d051282a2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8a933ec6480ac6417537d9a620eec85b51d53c49"
        },
        "date": 1780319160721,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12891,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3706062,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1093,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 176077,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 435,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 78398,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 2825,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 448229,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 470,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 92987,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 968,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217377,
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
          "id": "7c1ea5c68dcf0129990e684f6a5e297b2614c51b",
          "message": "perf(send): Arc immutable device fields + recent-message bytes (#674)",
          "timestamp": "2026-06-01T10:53:39-03:00",
          "tree_id": "a040ab692b82f0a5f22e7821e900f3d8ed126387",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7c1ea5c68dcf0129990e684f6a5e297b2614c51b"
        },
        "date": 1780322152149,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12756,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3701579,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 357,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1272,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 207296,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 409,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 76651,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 2881,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 456344,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 452,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 90741,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1129,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 257057,
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
          "id": "f01d4727136a0f450114a475906dc5f1eadd179b",
          "message": "perf(send): box the phash-mismatch cold path out of the spawned future (#675)",
          "timestamp": "2026-06-01T11:24:15-03:00",
          "tree_id": "f23dd0096852fe8181f669bd8f09293b28daab05",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/f01d4727136a0f450114a475906dc5f1eadd179b"
        },
        "date": 1780323975310,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12738,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3656156,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 1245,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 203108,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 332,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 61002,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4018,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 623489,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 396,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 78959,
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
            "value": 245362,
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
          "id": "c5bef747fe219815ad4e06f5337107e8d94c70c2",
          "message": "perf(events): typed event subscription to skip boxing unwanted events (#676)",
          "timestamp": "2026-06-01T12:08:10-03:00",
          "tree_id": "6ca1ad09cd013c0f66e9981df6af093fc8bba506",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c5bef747fe219815ad4e06f5337107e8d94c70c2"
        },
        "date": 1780326632507,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 12725,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3752926,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 358,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 2387,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 372058,
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
            "value": 58425,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 4470,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 694904,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 1,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 381,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 76983,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 875,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210000,
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
          "id": "483a896ea96eff103735e50897b8408f1623f095",
          "message": "perf(lid-pn): skip PN->LID session migration for peers with no PN state (#677)",
          "timestamp": "2026-06-01T13:12:33-03:00",
          "tree_id": "797f4b7c6abafc0ed740d084c88ec606bf3c478d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/483a896ea96eff103735e50897b8408f1623f095"
        },
        "date": 1780330500314,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8359,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2953211,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 352,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 289,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 48738,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 340,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 62004,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 332,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 59054,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 379,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 76479,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1131,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 257123,
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
          "id": "1613e56c10e262a07833bde10411692da0601809",
          "message": "fix(send): correct group phash and mark full SKDM target set (WA Web parity) (#678)",
          "timestamp": "2026-06-01T14:34:04-03:00",
          "tree_id": "04009b0bc6ca165f8cb20857c5c1368131edeb7f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/1613e56c10e262a07833bde10411692da0601809"
        },
        "date": 1780335376061,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8411,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 3007344,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 296,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 51110,
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
            "value": 59283,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 528,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 88419,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 377,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 76252,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 878,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210390,
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
          "id": "29d4689ae5a2168723d376c93bcf6ea189b6760d",
          "message": "feat: WA Web phash parity (#679)",
          "timestamp": "2026-06-01T15:49:07-03:00",
          "tree_id": "0cff1428e8ce16acb0190d638ed8c6f3dee69f8b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/29d4689ae5a2168723d376c93bcf6ea189b6760d"
        },
        "date": 1780339901340,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8269,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2995408,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 354,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 283,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 50062,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 320,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 58262,
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
            "value": 70077,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 377,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 76304,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 855,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210774,
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
          "id": "bfdda0c6f044880f7e50f03e7489fd315cd7b53d",
          "message": "perf(usync): batch LID-PN re-learn from device response off the cold group-send path (#680)",
          "timestamp": "2026-06-01T16:55:24-03:00",
          "tree_id": "6e64ebae4796253f61649a8ca6bf8a72f9f8fdf6",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/bfdda0c6f044880f7e50f03e7489fd315cd7b53d"
        },
        "date": 1780343853930,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8312,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2887391,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 237,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 42085,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 334,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 61645,
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
            "value": 70961,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 376,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 75605,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1046,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 247127,
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
          "id": "af1202e7182fb7fd2958540ed70bce0ad1498ec1",
          "message": "perf(device-registry): borrow lookup keys in get_devices_from_registry (hot group-send path) (#681)",
          "timestamp": "2026-06-01T17:16:29-03:00",
          "tree_id": "66ab02fd25980558d100e0c1814ae0d691ebd0d7",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/af1202e7182fb7fd2958540ed70bce0ad1498ec1"
        },
        "date": 1780345108199,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8327,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2997126,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 355,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 274,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 48868,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 329,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 61240,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 503,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 87537,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 371,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 76044,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 854,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210669,
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
          "id": "856170a4ee5fb763d60a64e2550875b943be118e",
          "message": "perf(device-registry): build lookup keys with CompactString (inline, no heap) (#682)",
          "timestamp": "2026-06-01T17:51:38-03:00",
          "tree_id": "8abe51e109ebc48d09b526322f30232120c1e5ee",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/856170a4ee5fb763d60a64e2550875b943be118e"
        },
        "date": 1780347226263,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8321,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2987361,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 351,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 237,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43005,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 231,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 42972,
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
            "value": 56227,
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
            "value": 76425,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1091,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 256601,
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
          "id": "967ef4902f0e33ab72058c60806aa5de51ce51b4",
          "message": "perf(zlib): pool streaming InflateReader state across history-sync blobs (#683)",
          "timestamp": "2026-06-01T18:41:00-03:00",
          "tree_id": "d0c75fe83d82ccffee3eda393bab5d7b14ca4ebe",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/967ef4902f0e33ab72058c60806aa5de51ce51b4"
        },
        "date": 1780350196536,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8238,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2985848,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 356,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 257,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47380,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 249,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 45402,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 311,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 57543,
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
            "value": 76379,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1103,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 257750,
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
          "id": "0f53c2b768fe1e69a15bb33ebde0fe43faf954c0",
          "message": "feat(upload): constant-memory streaming upload + media streaming sidecar (#684)",
          "timestamp": "2026-06-02T00:17:33-03:00",
          "tree_id": "7f15489c839c02cd3a2b138b4b81bfe487d0bbcd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0f53c2b768fe1e69a15bb33ebde0fe43faf954c0"
        },
        "date": 1780370393523,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8280,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2880895,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 355,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 257,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47172,
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
            "value": 57214,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 423,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 74483,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 374,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 76776,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 849,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210694,
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
          "id": "a5583c90236c9d21a9c4ef1a0a27928dc10ff2b6",
          "message": "feat(upload): add encrypted_len() and UploadSource for Bytes (#685)",
          "timestamp": "2026-06-02T09:50:22-03:00",
          "tree_id": "55d43f77002a3c13d44225d6a68eb0688379d81a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a5583c90236c9d21a9c4ef1a0a27928dc10ff2b6"
        },
        "date": 1780404750421,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8228,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2979194,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 358,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 270,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 48023,
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
            "value": 57475,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 503,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 87640,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 369,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 75605,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 846,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210662,
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
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "ea8ac6a8dd3531aae71f7e318f7199d4e508d1d2",
          "message": "perf(build): optimize release profile settings and enhance Docker build process",
          "timestamp": "2026-06-02T11:04:12-03:00",
          "tree_id": "dc2a2a3359ab69ef9710208639c2caf0cce5ffcf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ea8ac6a8dd3531aae71f7e318f7199d4e508d1d2"
        },
        "date": 1780409397309,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8449,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2975875,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 255,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 48119,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 328,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 61282,
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
            "value": 56175,
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
            "value": 76414,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 846,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210615,
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
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "65bf7f64a3bc24bb80c2ff4d140f48f757910b66",
          "message": "perf(build): update RUSTFLAGS to target native CPU for optimized builds",
          "timestamp": "2026-06-02T11:08:35-03:00",
          "tree_id": "5216f03d00698f7e77075db90433a128473c9675",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/65bf7f64a3bc24bb80c2ff4d140f48f757910b66"
        },
        "date": 1780409526628,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8316,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2992578,
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
            "value": 34338,
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
            "value": 58654,
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
            "value": 84641,
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
            "value": 76433,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 850,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210955,
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
          "id": "0943cfb3e4ab4204a21c31ce8c6534947a88fba5",
          "message": "perf(history-sync): single-byte fast-path for read_varint (#686)",
          "timestamp": "2026-06-02T12:08:26-03:00",
          "tree_id": "f5627769ecd5a2708cc7001e95d788d753ac8f97",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0943cfb3e4ab4204a21c31ce8c6534947a88fba5"
        },
        "date": 1780413166043,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8314,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2941816,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 189,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 34120,
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
            "value": 47428,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 383,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 69867,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 370,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 76516,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 845,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 210642,
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
          "id": "74e2e62cc5b76e6c417593e0cd22006111d2b76d",
          "message": "perf(appstate)!: batch previous-MAC lookups (N+1 to 1 query) (#687)",
          "timestamp": "2026-06-02T13:07:34-03:00",
          "tree_id": "126f52ed5a9578eb9cc50d9d017cc6027960e658",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/74e2e62cc5b76e6c417593e0cd22006111d2b76d"
        },
        "date": 1780416713689,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8184,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2913996,
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
            "value": 36188,
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
            "value": 59065,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 435,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 77326,
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
            "value": 76405,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1059,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 254413,
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
          "id": "8614ae04e558bdec44e0947b72db0cbd0ccf0403",
          "message": "perf(appstate): parse sync response once (+ dedup external-blob download) (#688)",
          "timestamp": "2026-06-02T13:43:49-03:00",
          "tree_id": "44ddbb0a64fb7d78712b7391e3d23984269e35ab",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/8614ae04e558bdec44e0947b72db0cbd0ccf0403"
        },
        "date": 1780418878887,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8217,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2977141,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 252,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 46772,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 312,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 58132,
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
            "value": 71892,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 373,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 76471,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1071,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 254821,
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
          "id": "68d565dcd060b7e81faf8a4e93f27bd404d9da0f",
          "message": "perf(appstate)!: drop duplicate index/value MAC storage from Mutation (#689)",
          "timestamp": "2026-06-02T13:51:58-03:00",
          "tree_id": "4b35a595ea7ce6a4abdccb5dbca19e68acf5cb0d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/68d565dcd060b7e81faf8a4e93f27bd404d9da0f"
        },
        "date": 1780419382695,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8276,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2881401,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 350,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 276,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 49034,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 312,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 58826,
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
            "value": 87649,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 368,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 75595,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 847,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 208611,
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
          "id": "504961e7e35b73655597726a09a080541c1e7e0c",
          "message": "perf!: cut clones/allocs in LID resolution, single-device send, history-sync (#690)",
          "timestamp": "2026-06-02T15:20:53-03:00",
          "tree_id": "c1167d4c2ecc22b02aaa3f4351d579b8bbb83aa0",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/504961e7e35b73655597726a09a080541c1e7e0c"
        },
        "date": 1780424704149,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8340,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2905152,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 179,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 33789,
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
            "value": 60326,
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
            "value": 84209,
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
            "value": 76516,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 839,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 211200,
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
          "id": "9a82686014495c01d4ddd22f9cbb838dd73e9be6",
          "message": "perf: cut copies in PBKDF2, noise large-frame send, history-sync secret records (#691)",
          "timestamp": "2026-06-02T15:53:54-03:00",
          "tree_id": "70732aa3d1d5d2a64ed92a927797a1a269f0a671",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/9a82686014495c01d4ddd22f9cbb838dd73e9be6"
        },
        "date": 1780426689486,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8273,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2953277,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 297,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 52279,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 328,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 63137,
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
            "value": 55726,
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
            "value": 76511,
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
            "value": 213284,
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
          "id": "5449a69fbee85d979ace8ee18ba41fc06accba4a",
          "message": "fix(send): unwrap groupStatusV2 + align unwrap_message with WA Web (not text) (#692)\n\nCo-authored-by: Salientekill <Salientekill@users.noreply.github.com>\nCo-authored-by: João Lucas <jlucaso@hotmail.com>",
          "timestamp": "2026-06-02T16:40:12-03:00",
          "tree_id": "b82049006f542e175345dbf4afc765930985472a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/5449a69fbee85d979ace8ee18ba41fc06accba4a"
        },
        "date": 1780429464902,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8352,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2916930,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 186,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 34378,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 302,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 57945,
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
            "value": 70308,
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
            "value": 75773,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1062,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 257190,
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
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "committer": {
            "email": "jlucaso@hotmail.com",
            "name": "João Lucas",
            "username": "jlucaso1"
          },
          "distinct": true,
          "id": "ae1b84e81cfb80d277a0b8fc0ca35190e77de645",
          "message": "fix(retry): include chat context in retry receipt logs",
          "timestamp": "2026-06-02T17:17:05-03:00",
          "tree_id": "bd3a944cc039e57013362b4b306146255c6ed146",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/ae1b84e81cfb80d277a0b8fc0ca35190e77de645"
        },
        "date": 1780431657233,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8247,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2895422,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 263,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 47786,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 270,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 52325,
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
            "value": 87398,
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
            "value": 75952,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 841,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 213339,
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
          "id": "014c5f9b6003fcca937730de779fd28ace2f198a",
          "message": "fix(send): classify payment-family stanzas as text + add stanza-type override (#693)",
          "timestamp": "2026-06-02T23:33:00-03:00",
          "tree_id": "e99cacf0ff56d27c8562b0f09adbf469e2e0d349",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/014c5f9b6003fcca937730de779fd28ace2f198a"
        },
        "date": 1780454218460,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8232,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2947944,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 243,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 44255,
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
            "value": 55022,
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
            "value": 87024,
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
            "value": 75841,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 839,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 213037,
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
          "id": "072b1d6c57100dec896a336d92d5258e8fb30c93",
          "message": "fix(client): use portable_atomic::AtomicU64 in offline_resume (#694)",
          "timestamp": "2026-06-03T00:53:18-03:00",
          "tree_id": "95edf6b66b47f72eec3c0ad3f621a86954ce1b66",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/072b1d6c57100dec896a336d92d5258e8fb30c93"
        },
        "date": 1780459006249,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8222,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2892504,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 236,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41149,
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
            "value": 58016,
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
            "value": 70932,
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
            "value": 76859,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1020,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 253826,
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
          "id": "a1b29149f8b79ef08cce5d104ccc9d6fad324154",
          "message": "feat(prekeys): make pre-key upload batch size configurable via the builder (#695)",
          "timestamp": "2026-06-03T09:54:15-03:00",
          "tree_id": "f695b8d739cd9392135449aa00e71a0d7f7dfa43",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/a1b29149f8b79ef08cce5d104ccc9d6fad324154"
        },
        "date": 1780491481769,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8249,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2882482,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 260,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 49638,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 233,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 44116,
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
            "value": 87301,
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
            "value": 75628,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 990,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 245375,
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
          "id": "cbbe8e9e1896b7827e63587bd450ecdc524ba025",
          "message": "perf(send): cut throwaway allocations in DM phash and LID conversion (#697)",
          "timestamp": "2026-06-03T11:02:00-03:00",
          "tree_id": "5e2b4072f3e38d9b56484981aacf6005a80e8e99",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/cbbe8e9e1896b7827e63587bd450ecdc524ba025"
        },
        "date": 1780495565183,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8202,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2779594,
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
            "value": 296,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 58297,
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
            "value": 89051,
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
            "value": 75578,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 867,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 221603,
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
          "id": "4c9ff69e23d39750898c583fe481128bd95385fe",
          "message": "fix(send): hoist messageContextInfo to outer in DeviceSentMessage (WA Web parity) (#696)",
          "timestamp": "2026-06-03T11:02:14-03:00",
          "tree_id": "b3b3aabadb7677788da46525d32bb464792703b3",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/4c9ff69e23d39750898c583fe481128bd95385fe"
        },
        "date": 1780495623667,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8208,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2779802,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 276,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 50704,
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
            "value": 50587,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 299,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 57106,
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
            "value": 75989,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1075,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 262230,
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
          "id": "c2ed1289e6068ab2b453fa11d60b9a72c9eb51e5",
          "message": "fix(message): emit quote remoteJid only for cross-chat quotes (#698)",
          "timestamp": "2026-06-03T21:31:42-03:00",
          "tree_id": "97ec9cb15048b2714f6a919003378f459b92a70c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c2ed1289e6068ab2b453fa11d60b9a72c9eb51e5"
        },
        "date": 1780533365549,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8289,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2790997,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 347,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 216,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41502,
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
            "value": 60458,
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
            "value": 87266,
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
            "value": 76175,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 869,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 221799,
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
          "id": "3f5ec59ee7b4dd4443b90d3d5353fc45278297ea",
          "message": "perf(appstate): replace O(n²) in-patch overwrite scan with an O(1) map (#701)\n\nReplace the O(n²) reverse scan over patch.mutations[..idx] with an incremental HashMap<index_mac, value_mac> lookup in process_patch's update_hash callback. Behavior-identical to the prior scan (op-agnostic, strictly-prior, DB fallback). O(n²) -> O(n). Adds in-patch overwrite and remove-then-set regression tests.",
          "timestamp": "2026-06-04T00:42:26-03:00",
          "tree_id": "ab73283b01374ffac6bc69cece06ac057062dcbf",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/3f5ec59ee7b4dd4443b90d3d5353fc45278297ea"
        },
        "date": 1780544802155,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8365,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2808888,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 184,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35117,
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
            "value": 59143,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 475,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 81737,
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
            "value": 76416,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1089,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 259329,
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
          "id": "b75f523a7aa288b2a435581e0c8bc1afe7406f3a",
          "message": "fix(retry): send raw 32-byte prekey/signed-prekey values in retry receipts (#702)\n\nsend_retry_receipt emitted the retry <keys> prekey/signed-prekey <value> via PublicKey::serialize() (33 bytes), which a peer's prekey parser rejects, breaking session re-establishment on the NoSession/UnknownCompanion recovery path. Extract build_retry_keys_node into wacore::protocol::retry emitting the raw 32-byte public_key_bytes(), matching identity/upload and WA Web's xmppPreKey/xmppSignedPreKey. Adds a TDD round-trip test.",
          "timestamp": "2026-06-04T01:14:52-03:00",
          "tree_id": "8aa8bc39109d6428aa53c8dcdaa640b16ef2e385",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b75f523a7aa288b2a435581e0c8bc1afe7406f3a"
        },
        "date": 1780546741778,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8265,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2787490,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 345,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 217,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41705,
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
            "value": 56321,
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
            "value": 87032,
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
            "value": 75407,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 37,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 870,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 219749,
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
          "id": "10f0874be245d47bc99cc7d2d28bc5705fccfd33",
          "message": "fix(time): stop wall-clock default from panicking on wasm32 (#704)",
          "timestamp": "2026-06-04T07:18:48-03:00",
          "tree_id": "b4f0eb38e41a1393b11ea060d667d9ee8973ac0e",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/10f0874be245d47bc99cc7d2d28bc5705fccfd33"
        },
        "date": 1780568573280,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8565,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2902800,
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
            "value": 40663,
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
            "value": 60327,
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
            "value": 69701,
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
            "value": 75200,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 867,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 219587,
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
          "id": "93bb7fe0a64a22380a1585d04ccd23928513fc5c",
          "message": "perf(device-registry): cache Arc<DeviceListRecord> to avoid deep clone on warm hits (#703)",
          "timestamp": "2026-06-04T07:19:15-03:00",
          "tree_id": "eb0e70c50658a9dc1dc7bea5c7f97b448ade88dd",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/93bb7fe0a64a22380a1585d04ccd23928513fc5c"
        },
        "date": 1780568602393,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8207,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2882453,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 343,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 209,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 40897,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 299,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 59146,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 366,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 68868,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 353,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 75619,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1031,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 254958,
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
          "id": "2341d81916651d4cea675e0ae3c308502f91e599",
          "message": "fix(retry): match WA Web's bot gate so bot DM retry receipts aren't dropped (#705)",
          "timestamp": "2026-06-04T07:47:54-03:00",
          "tree_id": "d0f9ddd7bae28286524a5932d8e74ee126e10c6f",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/2341d81916651d4cea675e0ae3c308502f91e599"
        },
        "date": 1780570334233,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8165,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2816928,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 341,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 194,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 39834,
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
            "value": 55167,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 364,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 68728,
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
            "value": 76306,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 864,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 217344,
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
          "id": "0e3c5985f272bae3d2f311564b16d30e24098414",
          "message": "fix(prekeys): force the upload on prekey-low instead of re-querying the server count (#706)",
          "timestamp": "2026-06-04T07:50:35-03:00",
          "tree_id": "23e8203facc13270df82d20939d7952c6df8d87c",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/0e3c5985f272bae3d2f311564b16d30e24098414"
        },
        "date": 1780570468560,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8561,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2894100,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 346,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 177,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 36222,
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
            "value": 293,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 56908,
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
            "value": 76370,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1123,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 277162,
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
          "id": "7a869596eee3ac2e35ea9c8a9dd1a02ed09ecbc9",
          "message": "fix(receipt): downgrade delivery ack to \"sent\" on lid feature-incapable error (#708)",
          "timestamp": "2026-06-04T08:34:53-03:00",
          "tree_id": "c45f9c7a1e3d67bf03cd21ae28313fbbda852320",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7a869596eee3ac2e35ea9c8a9dd1a02ed09ecbc9"
        },
        "date": 1780573137811,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8270,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2790795,
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
            "value": 41591,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 302,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 59486,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 298,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 56969,
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
            "value": 76240,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 990,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 245232,
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
          "id": "e44ee46ac734c5fa1930b41b6cf9d7c66ee4f667",
          "message": "fix(blocking): resolve LID/PN before is_blocked compares, fixing PN-query false negatives (#707)",
          "timestamp": "2026-06-04T08:35:51-03:00",
          "tree_id": "e8118d3552e418f22c8c2ab9b36476e64d24c5d1",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/e44ee46ac734c5fa1930b41b6cf9d7c66ee4f667"
        },
        "date": 1780573166342,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8318,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2849814,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 342,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 211,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41271,
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
            "value": 51234,
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
            "value": 69648,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 41,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 354,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 74935,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 38,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 863,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 221227,
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
          "id": "61666de7471e44b77a4a1308bcc0c0a3ab968984",
          "message": "fix(retry): allocate retry-receipt prekey from the monotonic counter, not random (#709)",
          "timestamp": "2026-06-04T08:40:33-03:00",
          "tree_id": "5f81d6a314c6625b79d97c0b6a5c53798329fe9a",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/61666de7471e44b77a4a1308bcc0c0a3ab968984"
        },
        "date": 1780573457119,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8372,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2755628,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 233,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 43175,
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
            "value": 55523,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 276,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 56324,
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
            "value": 77111,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1071,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 260098,
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
          "id": "fab3ae1d1e8adc76aa414748288de0d2819cdfa3",
          "message": "chore(deps): bump log from 0.4.30 to 0.4.31 (#700)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-06-04T08:40:49-03:00",
          "tree_id": "ffc9df9c3456546682f04dcddba2f2a661d8e002",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/fab3ae1d1e8adc76aa414748288de0d2819cdfa3"
        },
        "date": 1780573510699,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8276,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2841637,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 344,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 197,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35703,
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
            "value": 59567,
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
            "value": 56399,
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
            "value": 77272,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1071,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 262079,
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
          "id": "d2a6e93656b429a2446b438c0646f3d37f880aaa",
          "message": "chore(deps): bump yoke from 0.8.2 to 0.8.3 (#699)\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-06-04T08:41:03-03:00",
          "tree_id": "3b2b20e3f4d8ec4f492d294b4f0af5efe3f1b2e2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d2a6e93656b429a2446b438c0646f3d37f880aaa"
        },
        "date": 1780573545182,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8078,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2752282,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 338,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 197,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 37144,
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
            "value": 52408,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 271,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 55894,
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
            "value": 76054,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1107,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 262998,
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
          "id": "c41024c21a2e3a9a73ad6c2c559f4ad97fcfad7b",
          "message": "perf(groups): cache Arc<GroupInfo> to avoid deep-cloning group metadata on warm sends (#710)",
          "timestamp": "2026-06-04T09:36:34-03:00",
          "tree_id": "21d741dd4d4757036f6d0e8ba8c7ac44c68d080b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/c41024c21a2e3a9a73ad6c2c559f4ad97fcfad7b"
        },
        "date": 1780576849256,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8231,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2786190,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 182,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 35265,
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
            "value": 59942,
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
            "value": 86841,
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
            "value": 75359,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 838,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 212900,
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
          "id": "7e3c1a927b902b7c845948c04b9ede6d1e7fbee0",
          "message": "fix(send): emit mediatype for interactive/list/order/product/native-flow sends (#711)",
          "timestamp": "2026-06-04T10:20:48-03:00",
          "tree_id": "fcc3dfcb21eff110160422072e51d3e844ed954b",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/7e3c1a927b902b7c845948c04b9ede6d1e7fbee0"
        },
        "date": 1780579503052,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8206,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2780266,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 217,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41752,
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
            "value": 49161,
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
            "value": 56926,
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
            "value": 76292,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 836,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 212697,
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
          "id": "b7a2fc94ed59077bfccdcd7f1f337ca652a6357e",
          "message": "perf(decrypt): trim per-SKDM allocations in the group fan-in path (#712)",
          "timestamp": "2026-06-04T11:57:11-03:00",
          "tree_id": "4c112f4b5085dd9087a7cfbf0e14df4cb925b7c2",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/b7a2fc94ed59077bfccdcd7f1f337ca652a6357e"
        },
        "date": 1780585287010,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8146,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2770489,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 348,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 216,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 41767,
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
            "value": 59731,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message_x20_amortized::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_message::alloc_count",
            "value": 370,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_message::alloc_bytes",
            "value": 69427,
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
            "value": 76411,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 43,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 1045,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 253505,
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
          "id": "d3a08d4222458dbc82a042b9eb982377242c7b00",
          "message": "perf(signal): cache Arc<SenderKeyRecord> to avoid deep-cloning the message-key backlog (#713)",
          "timestamp": "2026-06-04T12:27:07-03:00",
          "tree_id": "1be4da060857df2beb1d2f3183c1d27b896e5f6d",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/d3a08d4222458dbc82a042b9eb982377242c7b00"
        },
        "date": 1780587065195,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8204,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2828255,
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
            "value": 33668,
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
            "value": 55001,
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
            "value": 70050,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_message::wall_ms",
            "value": 40,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_count",
            "value": 353,
            "unit": "allocations"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::alloc_bytes",
            "value": 75131,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 39,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 866,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 221333,
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
          "id": "658d98c8ee053f9349b1162ad05ab6b63940948d",
          "message": "perf(store): batch session/identity/sender-key flush into one transaction per category (#714)",
          "timestamp": "2026-06-04T13:16:54-03:00",
          "tree_id": "0009f81d20acfbb4df4feb5b82c12524e6224210",
          "url": "https://github.com/oxidezap/whatsapp-rust/commit/658d98c8ee053f9349b1162ad05ab6b63940948d"
        },
        "date": 1780590089249,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "integration::connect_to_ready::alloc_count",
            "value": 8352,
            "unit": "allocations"
          },
          {
            "name": "integration::connect_to_ready::alloc_bytes",
            "value": 2819210,
            "unit": "bytes"
          },
          {
            "name": "integration::connect_to_ready::wall_ms",
            "value": 349,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message::alloc_count",
            "value": 274,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message::alloc_bytes",
            "value": 50049,
            "unit": "bytes"
          },
          {
            "name": "integration::send_message::wall_ms",
            "value": 0,
            "unit": "milliseconds"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_count",
            "value": 325,
            "unit": "allocations"
          },
          {
            "name": "integration::send_message_x20_amortized::alloc_bytes",
            "value": 63721,
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
            "value": 88673,
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
            "value": 77116,
            "unit": "bytes"
          },
          {
            "name": "integration::send_and_receive_x20_amortized::wall_ms",
            "value": 36,
            "unit": "milliseconds"
          },
          {
            "name": "integration::reconnect::alloc_count",
            "value": 839,
            "unit": "allocations"
          },
          {
            "name": "integration::reconnect::alloc_bytes",
            "value": 212780,
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