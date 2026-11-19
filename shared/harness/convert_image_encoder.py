"""Convert the selected local encoder into MFLUX Q4 format, keeping source provenance."""
from pathlib import Path
import hashlib,json,os,time
from runtime import ROOT,heavy_lock

def main():
 os.environ['HF_HUB_OFFLINE']='1';os.environ['HF_HUB_DISABLE_TELEMETRY']='1'
 source=ROOT/'shared/models/image-local/source';out=ROOT/'shared/models/image-local/flux2-klein-4b-mflux-q4'
 if not (ROOT/'evidence/setup-20261102/image-encoder-download.json').is_file():raise RuntimeError('Verified encoder download is required first')
 with heavy_lock():
  import mlx.core as mx
  from mlx.utils import tree_flatten
  from mflux.models.flux2.weights.flux2_weight_definition import Flux2KleinWeightDefinition
  from mflux.models.common.weights.loading.weight_loader import WeightLoader
  from mflux.models.common.weights.loading.weight_applier import WeightApplier
  from mflux.models.common.weights.saving.model_saver import ModelSaver
  from mflux.models.flux2.model.flux2_text_encoder.qwen3_text_encoder import Qwen3TextEncoder
  from mflux.models.common.config.model_config import ModelConfig
  start=time.monotonic()
  component=next(c for c in Flux2KleinWeightDefinition.get_components() if c.name=='text_encoder')
  weights=WeightLoader.load_single_local(component,source)
  encoder=Qwen3TextEncoder(**ModelConfig.flux2_klein_4b().text_encoder_overrides)
  expected={k:tuple(v.shape) for k,v in tree_flatten(encoder.parameters()) if k.endswith(('.weight','.bias'))}
  supplied=dict(tree_flatten(weights.components['text_encoder']))
  missing=[k for k in expected if k not in supplied]
  mismatched=[k for k in expected if k in supplied and tuple(supplied[k].shape)!=expected[k]]
  if missing or mismatched:raise ValueError('Encoder mapping mismatch: '+json.dumps({'missing':missing,'shape_mismatch':mismatched}))
  bits=WeightApplier.apply_and_quantize_single(weights,encoder,component,4)
  del weights,supplied
  ModelSaver._save_weights(str(out),bits,encoder,'text_encoder')
  record={'source_repo':'ponpoke/flux2-klein-4b-local-text-encoder','source_revision':'633217e588e4c0bc76619052e05d3ce0e057cd83','quantization':bits,'mapped_parameter_tensors':len(expected),'all_parameter_shapes_matched':True,'seconds':round(time.monotonic()-start,2),'mlx_peak_bytes':mx.get_peak_memory(),'files':[]}
  for path in (out/'text_encoder').glob('*.safetensors'):
   h=hashlib.sha256()
   with path.open('rb') as f:
    for b in iter(lambda:f.read(8*1024*1024),b''):h.update(b)
   record['files'].append({'path':str(path),'sha256':h.hexdigest(),'bytes':path.stat().st_size})
  if bits!=4 or not record['files']:raise RuntimeError('Encoder conversion did not produce Q4 weight files')
  (ROOT/'evidence/setup-20261102/image-encoder-conversion.json').write_text(json.dumps(record,indent=2))
  print(json.dumps(record,indent=2))
if __name__=='__main__':main()
