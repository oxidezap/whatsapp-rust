// Capture the already-qualified upstream harness with its actual group payloads.
// No upstream checkout is modified. The sole instrumentation adds an output key.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const Module = require('node:module');
const script = path.resolve(process.argv[2], 'tests/conformance/groups/versions/2.3000.1052559453/normalization.cjs');
const source = fs.readFileSync(script, 'utf8');
assert.equal(crypto.createHash('sha256').update(source).digest('hex'), '8629e44df910c573a28c5552ecd87c97efa22c004a040b42d214bbda528a70a0', 'review upstream harness before recapture');
const needle = '          reads,\n          calls\n';
assert.equal(source.split(needle).length, 2, 'capture anchor changed');
const capture = source.replace(needle, '          groupDatas: actual === null ? null : actual.groupDatas,\n' + needle);
const harness = new Module(script, module);
harness.filename = script;
harness.paths = Module._nodeModulePaths(path.dirname(script));
process.argv.push('--json');
harness._compile(capture, script);
