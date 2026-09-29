// Regenerate the golden file using the actual archived modules, not a JS port.
// Usage: node retry_metadata.mjs /path/to/verified/restored/bundles > retry_metadata.json
// See agent_docs/retry_metadata.md for archive verification and stub scope.
import fs from 'node:fs';
import vm from 'node:vm';
import crypto from 'node:crypto';
const bundles = process.argv[2];
if (!bundles) throw new Error('pass the verified restored bundle directory');
const sources = {
  WAWebVerifyProtobufMsgObjectKeys: ['f1Q5yyJIapQ.js', '430cb8979511aada95178605faa35a8483446f677ce6d1fe0fd66333c7879eee', 434797, 442805],
  WAWebSendMsgMetaNode: ['4oJRvhb3yiE.js', 'f588ee2f033b87e018a3be34a37ec1347d8b6b8700de539e6be31481b29c4690', 985968, 990146],
};
const exports = {};
const DROP = Symbol('drop');
const stubs = {
  '$InternalEnum': x => Object.assign(x, {cast: v => v}),
  // Disable the diagnostic-only 3-level recursion experiment in this snapshot.
  justknobx: {_: () => false},
  WAWebCurrentUser: {isEmployee: () => false},
  WAWebHandleMsgError: {MessageProtobufInvalidMessageTypes: class InvalidMessageTypes extends Error {}},
  err: message => new Error(message),
  WAWebBotUtils: {isMetaAiBot: () => false},
  WAWebMsgType: {MSG_TYPE: new Proxy({PROTOCOL: 'protocol', RICH_RESPONSE: 'rich_response', MESSAGE_HISTORY_NOTICE: 'history'}, {get: (obj, key) => obj[key] ?? key})},
  WAWebCommonMsgSubtypeTypes: {MsgSubtype: {EphemeralSyncResponse: 'ephemeral_sync_response'}},
  WAWebUsernameTypes: {LidOriginType: {PNH_CTWA: 'pnh_ctwa'}},
  WAWebPollResultSnapshotPollTypeEnvelopeEnabled: () => false,
  WAWebHandleMsgCommon: {POLL_TYPES: {creation: 'creation', vote: 'vote', result_snapshot: 'result_snapshot', edit: 'edit'}, EVENT_TYPES: {creation: 'creation', response: 'response', edit: 'edit'}},
  'WAWebProtobufsE2E.pb': {Message$SecretEncryptedMessage$SecretEncType: {EVENT_EDIT: 1, POLL_EDIT: 3}},
  WAWap: {DROP_ATTR: DROP, CUSTOM_STRING: x => x, wap: (tag, attrs) => ({tag, attrs: Object.fromEntries(Object.entries(attrs).filter(([,v]) => v !== DROP))})},
};
const load = name => {
  if (exports[name]) return exports[name];
  if (stubs[name]) return stubs[name];
  throw new Error(`unexpected stub requested: ${name}`);
};
for (const [name, [file, sha256, begin, end]] of Object.entries(sources)) {
  const bytes = fs.readFileSync(`${bundles}/${file}`);
  if (crypto.createHash('sha256').update(bytes).digest('hex') !== sha256) throw new Error(`bundle checksum mismatch: ${file}`);
  vm.runInNewContext(bytes.subarray(begin, end).toString('utf8'), {__d: (id, deps, factory) => {
    if (id !== name) throw new Error(`wrong module at pinned offset: ${id}`);
    const result = {};
    factory({}, load, load, load, {}, {}, result);
    exports[id] = result;
  }});
}
const cases = [];
const append = (kind, flag, wrappers = []) => {
  let proto, mediaData;
  switch (kind) {
    case 'image': case 'video': case 'audio': case 'ptv':
      proto = {[kind + 'Message']: flag == null ? {} : {viewOnce: flag}};
      // Synthetic media records model inspected parsers/generator, NOT an
      // execution of upload, consumption, authorization or receiving policies.
      mediaData = {isViewOnce: flag === true || wrappers.some(w => w.startsWith('v'))};
      break;
    case 'text': proto = {conversation: 'synthetic'}; break;
    case 'extended_text': proto = {extendedTextMessage: {viewOnce: flag}}; break;
    case 'buttons': proto = {buttonsMessage: {}}; break;
    case 'interactive': proto = {interactiveMessage: {header: {imageMessage: {viewOnce: true}}}}; break;
    case 'quoted': proto = {imageMessage: {contextInfo: {quotedMessage: {viewOnceMessage: {message: {imageMessage: {}}}}}}}; mediaData = {isViewOnce: false}; break;
    case 'poll_v1': proto = {pollCreationMessage: {}}; break;
    case 'poll_v2': proto = {pollCreationMessageV2: {}}; break;
    case 'poll_v3': proto = {pollCreationMessageV3: {}}; break;
    case 'poll_vote': proto = {pollUpdateMessage: {vote: {}}}; break;
    case 'poll_empty_vote': proto = {pollUpdateMessage: {}}; break;
    case 'poll_snapshot': proto = {pollResultSnapshotMessage: {}}; break;
    case 'event': proto = {eventMessage: {}}; break;
    case 'event_response': proto = {encEventResponseMessage: {}}; break;
    case 'event_edit': proto = {secretEncryptedMessage: {secretEncType: 1}}; break;
    case 'member_label': proto = {protocolMessage: {}}; break;
    case 'empty': proto = {}; break;
    default: throw new Error(kind);
  }
  const keys = {dsm: 'deviceSentMessage', ephemeral: 'ephemeralMessage', v1: 'viewOnceMessage', v2: 'viewOnceMessageV2', v2ext: 'viewOnceMessageV2Extension'};
  for (const w of [...wrappers].reverse()) proto = {[keys[w]]: kind === 'empty' ? {} : {message: proto}};
  const data = {type: kind === 'member_label' ? 'protocol' : 'ordinary', mediaData};
  if (kind === 'member_label') Object.assign(data, {subtype: 'member_label', memberLabelData: {label: flag ? 'synthetic' : ''}});
  let meta, invalid = false;
  try {
    meta = exports.WAWebSendMsgMetaNode.genMetaNode({chatId: {isLid: () => false}, msgProtobuf: proto, msgRecord: {type: 'message', data}});
  } catch (error) {
    if (!(error instanceof stubs.WAWebHandleMsgError.MessageProtobufInvalidMessageTypes)) throw error;
    invalid = true;
  }
  cases.push({kind, flag: flag ?? null, wrappers, invalid, expected: meta?.attrs ?? {}});
};
for (const type of ['image', 'video', 'audio', 'ptv']) {
  for (const flag of [true, false, null]) append(type, flag);
  for (const w of ['v1','v2','v2ext']) append(type, null, ['dsm','ephemeral',w]);
}
for (const kind of ['text','extended_text','buttons','interactive','empty']) {
  append(kind, true);
  append(kind, true, ['v1']);
}
for (const kind of ['quoted','poll_v1','poll_v2','poll_v3','poll_vote','poll_empty_vote','poll_snapshot','event','event_response','event_edit']) append(kind, null);
append('event', null, ['dsm','ephemeral']);
append('member_label', true);
append('member_label', false);
console.log('[\n' + cases.map(c => '  ' + JSON.stringify(c)).join(',\n') + '\n]');
