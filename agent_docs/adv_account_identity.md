# ADV account anchors and hosted devices

The ADV account anchor and the Signal device address are different identities.
For a `user:99@hosted` device the anchor is `user@s.whatsapp.net`; for
`user:99@hosted.lid` it is `user@lid`. The device session/identity retains its
original hosted namespace. `adv_account_jid` is restricted to selecting the
anchor; do not replace general JID normalization or Signal address rendering.

Static reference: whatspec commit
`ae3880925bacd23863cc30ec480c9f82bfd869f1`, schema `4.5.0`, WA
`2.3000.1052559453`, 676 bundles, set hash
`d6ab7ae601eda281cfac570a68de1d787adde8f4d778f0bb994416f661a21abf`.
The `bundle-store` asset `bundles-2.3000.1052559453-<set hash>.tar.xz` has
SHA256 `2958d99a3d84141fa8479f1f991e06246e68c063ca8f618a8d5d361287387f62`.
The archive and all 676 bundle hashes/sizes were checked against the pinned
lock. Complete modules were extracted using an AST, without executing them.

Relevant original byte offsets (start inclusive, end exclusive):

| Bundle | Module | Offsets |
| --- | --- | --- |
| `0RwvWNH8_qZ.js` | `WAWebAdvSignatureApi` | 767477–775940 |
| `0RwvWNH8_qZ.js` | `WAWebHandleAdvDeviceNotificationUtils` | 731532–733850 |
| `331d6f8883e8cd73ff267508b8e5889920b0219c7fd88325ee56ee22eefdcb00.js` | `WAWebWidFactory` | 545429–549200 |
| same | `WAWebWid` | 535329–545312 |
| same | `WAWebSignalAddress` | 3107699–3108968 |

`validateADVwithIdentityKey` reads the original device identity separately
from the anchor selected with `asUserWidOrThrow`. That helper maps Hosted to
the ordinary PN family and HostedLid to LID. SignalAddress preserves hosted
device addressing; Wid rejects a hosted primary with device 0. This is static
Web evidence, not a claim about the server's response to malformed IQs.

Account fences must select the same anchor as verification. Multi-lock
acquisition groups by account family, then user/device, retaining the original
server as a tie-breaker. Plain lexical server ordering would acquire a hosted
LID device before its ordinary LID anchor, opposite to inbound processing.
The comparator is only for Signal locks; phash and device-list ordering retain
their existing rules.

The paired own-account fallback remains trusted only for an exact account
family/user match. Embedded certificate keys cannot replace an existing
anchor, and both signatures still use the original device's hosted semantics.
Cold bootstrap retains bounded singleflight, generation cancellation and owned
durable publication. A hosted USync list follows its existing separately
authenticated policy and cannot replace our paired primary identity.
