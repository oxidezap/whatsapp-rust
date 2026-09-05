#!/usr/bin/env python3
"""Re-derive and verify the primary MLOW fixtures using the pinned unwasm tool.

--check compares canonical CBOR bytes, independently of zstd's encoding
choices across versions. --from-derived verifies a previous run's complete
output hashes before reusing it; it never trusts unverified cached artifacts.
"""
import argparse,hashlib,importlib.util,json,subprocess,sys,os
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
DATA=ROOT/'wacore/src/voip/mlow/testdata'
PIN=Path(__file__).with_name('oracle.lock.json')
spec=importlib.util.spec_from_file_location('fixture_pack',Path(__file__).with_name('pack.py'))
pack=importlib.util.module_from_spec(spec);spec.loader.exec_module(pack)
LEAVES=['fe','signal_mode','pitch','lsf_quant','hp_postfilter','harm_postfilter','params','gennoise']
STREAMS=['wasm_derived_frames.json','wasm_derived_ref.raw','wasm_derived_vad.json','wasm_derived_120ms_frames.json','wasm_derived_120ms_ref.raw']

def sha(data):return hashlib.sha256(data).hexdigest()

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--oracle-repo',type=Path,required=True);p.add_argument('--out',type=Path,default=ROOT/'.derive-mlow/wasm');p.add_argument('--from-derived',type=Path);p.add_argument('--check',action='store_true');p.add_argument('--allow-tool-worktree',action='store_true');a=p.parse_args()
 tool=a.oracle_repo.resolve();pin=json.loads(PIN.read_text())
 revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=tool,text=True).strip()
 if not a.allow_tool_worktree:
  assert revision==pin['revision'],f'oracle revision differs: {revision}'
  subprocess.run(['git','diff','--quiet','HEAD','--','crates','scripts/mlow','specs','Cargo.lock','Cargo.toml'],cwd=tool,check=True)
 assert sha((tool/'specs/mlow.lock.json').read_bytes())==pin['derivation_lock_sha256'],'derivation lock drift'
 assert (tool/'specs/synth_mic.raw').read_bytes()==(DATA/'synth_mic.raw').read_bytes(),'synthetic inputs differ'
 sys.path.insert(0,str(tool/'scripts/mlow'))
 import verify
 out=(a.from_derived or a.out).resolve()
 if a.from_derived is None:
  # A checkout nested under this repo otherwise inherits its nightly-only
  # .cargo/config rustflags. Keep the oracle's stable build independent.
  build_env={**os.environ,'CARGO_ENCODED_RUSTFLAGS':''}
  subprocess.run(['cargo','+stable','build','--release','--locked','-p','oracle-cli'],cwd=tool,env=build_env,check=True)
  subprocess.run(['python3',str(tool/'scripts/mlow/verify.py'),'--out',str(out)],cwd=tool,check=True)
 lock=json.loads((tool/'specs/mlow.lock.json').read_text())
 for name,expected in lock['runs'].items():
  run=out/name;manifest=json.loads((run/'manifest.json').read_text())
  assert manifest['module']==expected['module'] and manifest['spec_sha256']==expected['spec_sha256'] and manifest['resolutions']==expected['resolutions'],f'cached manifest mismatch: {name}'
  assert verify.tree(manifest,run)==expected['tree_sha256'] and len(manifest['outputs'])==expected['outputs'],f'cached output mismatch: {name}'
 verify.assemble(out)
 stage=out/'packed';stage.mkdir(exist_ok=True);metadata={}
 for leaf in LEAVES:
  name='wasm_'+leaf+'.cbor.zst';source=out/'artifacts'/('wasm_'+leaf+'.json')
  record=pack.pack(source,stage/name)
  if a.check:
   decoded=subprocess.run(['zstd','-q','-d','-c',str(DATA/name)],check=True,stdout=subprocess.PIPE).stdout
   assert sha(decoded)==record['cbor_sha256'],f'wasm fixture drift: {name}'
  else:(DATA/name).write_bytes((stage/name).read_bytes())
  metadata[name]=record
 for name in STREAMS:
  payload=(out/'artifacts'/name).read_bytes()
  if a.check:assert (DATA/name).read_bytes()==payload,f'stream fixture drift: {name}'
  else:(DATA/name).write_bytes(payload)
  metadata[name]={'sha256':sha(payload),'bytes':len(payload)}
 if not a.check:
  (DATA/'wasm-fixtures.json').write_text(json.dumps({'derivation_lock_sha256':pin['derivation_lock_sha256'],'files':metadata},indent=2)+'\n')
 print('Wasm fixtures verified' if a.check else 'Wasm fixtures regenerated')
if __name__=='__main__':main()
