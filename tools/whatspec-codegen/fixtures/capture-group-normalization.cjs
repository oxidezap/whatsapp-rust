// Capture the already-qualified upstream harness with its actual group payloads.
// No upstream checkout is modified. Only output capture/representation changes.
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
const output = 'JSON.stringify(\n      { waVersion: evidence.waVersion, setHash: evidence.setHash, cases: outcomes },\n      null,\n      2\n    )';
assert.equal(source.split(output).length, 2, 'output anchor changed');
const capture = source
  .replace(needle, '          groupDatas: actual === null ? null : actual.groupDatas,\n' + needle)
  .replace(output, 'module.formatCapture({ waVersion: evidence.waVersion, setHash: evidence.setHash, cases: outcomes })');
const harness = new Module(script, module);
harness.filename = script;
harness.paths = Module._nodeModulePaths(path.dirname(script));
harness.formatCapture = ({ waVersion, setHash, cases }) => {
  // Share identical observed outcomes, without deriving any expected behavior
  // from the IR or dropping cases, payloads, reads or warning operations.
  const outcomes = [], indices = new Map();
  const rows = cases.map(({ family, kind, reason, hidden, ...outcome }) => {
    const key = JSON.stringify(outcome);
    if (!indices.has(key)) {
      indices.set(key, outcomes.length);
      outcomes.push(outcome);
    }
    return [family, kind, reason, hidden, indices.get(key)];
  });
  const metadata = { waVersion, setHash, inputColumns: ['family', 'kind', 'reason', 'hidden', 'outcome'], outcomes };
  const rendered = JSON.stringify(metadata, null, 2).slice(0, -2)
    + ',\n  "cases": [\n' + rows.map(row => '    ' + JSON.stringify(row)).join(',\n') + '\n  ]\n}';
  if (process.argv.includes('--check')) {
    assert.equal(rendered + '\n', fs.readFileSync(path.join(__dirname, 'group_normalization_oracle.json'), 'utf8'));
    return `${cases.length} complete normalization cases match the compact fixture`;
  }
  return rendered;
};
process.argv.push('--json');
harness._compile(capture, script);
