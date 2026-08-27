// Loads the real WA Web WAM codec modules and dumps the bytes they produce, so
// the Rust encoder is checked against the source of truth rather than against a
// reading of it. The vectors it prints are the ones asserted in
// `plugins/wam-catalog/src/tests.rs`.
//
//   node plugins/wam-catalog/tools/wa-web-vectors.js modules.js
//
// `modules.js` is the concatenated source of the WA Web modules `WABinary`,
// `WAHex`, `WAWamTypes`, `WAWebWamLibProtocol` and `WAWamBuffer`, taken from the
// bundle set the pinned whatspec commit records in `generated/bundles.lock.json`
// (see `agent_docs/wa_web_reference.md` for where the bundle lives). Each is a
// `__d("Name", [deps], factory, flags)` call; paste them in whole, in any order.
//
// The loader below is the smallest thing that runs them: one registry, one
// require that hands the same function to every require-shaped parameter, and
// the handful of `babelHelpers` the codec touches. It is not a Metro
// implementation and does not try to be.
const fs = require('fs');

const registry = {};
const cache = {};
function __d(name, deps, factory) { registry[name] = factory; }
function req(name) {
  if (name in cache) return cache[name];
  const f = registry[name];
  if (!f) throw new Error('missing module ' + name);
  const exports = {};
  const module = { exports };
  cache[name] = exports;
  f(globalThis, req, req, req, module, exports, exports);
  return cache[name];
}
globalThis.__d = __d;
globalThis.babelHelpers = {
  inheritsLoose(sub, sup) { sub.prototype = Object.create(sup.prototype); sub.prototype.constructor = sub; sub.__proto__ = sup; },
  assertThisInitialized(x) { return x; },
  taggedTemplateLiteralLoose(s) { return s; },
  extends: Object.assign,
  objectWithoutPropertiesLoose(o, keys) { const r = {}; for (const k in o) if (!keys.includes(k)) r[k] = o[k]; return r; },
};
// `err` builds an Error; the codec only calls it on invalid input.
__d('err', [], function (g, r1, r2, r3, m, e, l) { m.exports = (msg) => new Error(msg); });
cache['err'] = (msg) => new Error(msg);

const src = fs.readFileSync(process.argv[2] || 'modules.js', 'utf8')
  .split('\n').filter(l => !l.startsWith('////')).join('\n');
// eval, because the modules are minified statements that register themselves
// through the `__d` above; there is nothing to import.
eval(src);

const { Binary } = req('WABinary');
const proto = req('WAWebWamLibProtocol');

function hex(b) { return Buffer.from(b.peek(x => x.readByteArrayView())).toString('hex'); }

// A buffer header exactly as WamContext writes one.
function header(channel, streamId, sequence) {
  const b = new Binary(undefined, true);
  b.writeString('WAM');
  b.writeUint8(5);
  b.writeUint8(streamId);
  b.writeUint16(sequence);
  b.writeUint8(channel === 'regular' ? 0 : channel === 'realtime' ? 1 : 2);
  return b;
}



const vectors = [];
function emit(name, channel, seq, build) {
  const b = header(channel, 1, seq);
  build(b);
  vectors.push({ name, hex: hex(b) });
}

// Every case below is a whole buffer, written the way WamContext writes one:
// the dirty globals and the per-event timestamp go out together in ascending id
// order (the object holding them is keyed by id, and JS iterates integer-like
// keys numerically), then the event record and its fields. That is what reaches
// the wire, so it is what the Rust encoder is compared against.
const TIMESTAMP_FIELD_ID = 47;
function withEvent(b, globals, ts, code, weight, fields) {
  const all = globals.concat([[TIMESTAMP_FIELD_ID, ts]]).sort((x, y) => x[0] - y[0]);
  for (const [id, v] of all) proto.writeGlobalAttribute(b, id, v);
  proto.writeEvent(b, code, -weight, fields.length > 0);
  fields.forEach(([id, v], i) => proto.writeField(b, id, v, i < fields.length - 1));
}

// UnknownStanza (3448): one integer field (id 3) swept across every size class.
const INTS = [0, 1, -1, 2, 127, -128, 128, -129, 32767, -32768, 32768, -32769,
              2147483647, -2147483648, 2147483648, -2147483649];
for (const v of INTS) {
  emit('int_' + v, 'regular', 1, b => withEvent(b, [], 1755000000, 3448, 1, [[3, v]]));
}
// UnknownStanza: one string field (id 1) at every length class, plus multi-byte
// UTF-8 (the length prefix counts bytes, not characters).
for (const len of [0, 1, 255, 256, 65535, 65536]) {
  emit('str_' + len, 'regular', 1, b => withEvent(b, [], 1755000000, 3448, 1, [[1, 'a'.repeat(len)]]));
}
emit('str_utf8', 'regular', 1, b => withEvent(b, [], 1755000000, 3448, 1, [[1, 'héllo·世界']]));
// MemoryStat (1336): a float field (id 3) that is not an integer.
emit('float_field', 'regular', 1, b => withEvent(b, [], 1755000000, 1336, 1, [[3, 2.5]]));
// A float that happens to be integral goes out as an integer, not a double.
emit('float_integral', 'regular', 1, b => withEvent(b, [], 1755000000, 1336, 1, [[3, 4]]));
// Call (462): a field id past 255, which widens the id.
emit('wide_field_id', 'regular', 1, b => withEvent(b, [], 1755000000, 462, 1, [[1016, 12]]));
// WebWamForceFlush (3264): no fields at all, so the event record ends the group.
emit('no_fields', 'regular', 7, b => withEvent(b, [], 1755000000, 3264, 1, []));
// ReceiptStanzaReceive (2496): the release weight is 2000, and a boolean field
// reaches the writer already coerced to 0 or 1. Fields in catalog order.
emit('weighted_event', 'regular', 258, b => withEvent(b, [], 1755000000, 2496, 2000,
  [[14, 1], [1, 42], [3, 0], [9, 1], [7, 3], [4, 'read']]));
// A buffer carrying globals, then two events: the unchanged global is written
// once, the timestamp before every event.
emit('two_events_dedup', 'regular', 3, b => {
  withEvent(b, [[17, '2.3000.1045368834'], [11, 8], [4473, '']], 1755000000, 4358, 1, [[1, 3448], [2, 1]]);
  withEvent(b, [], 1755000061, 1144, 1, [[28, 2]]);
});
// A cleared global writes an empty record rather than repeating the old value.
emit('cleared_global', 'regular', 4, b => {
  withEvent(b, [[779, 'x']], 1755000000, 3264, 1, []);
  withEvent(b, [[779, null]], 1755000001, 3264, 1, []);
});
// A realtime buffer differs from a regular one only in the header byte.
emit('realtime_channel', 'realtime', 9, b => withEvent(b, [], 1755000000, 5504, 1, [[6, 1]]));
console.log(JSON.stringify(vectors, null, 1));
