"""Natural-language image request through the real coordinator and FLUX worker."""
import json
import shutil
from pathlib import Path
from PIL import Image
from rust_agent import orchestrate
from runtime import ROOT

def main():
 directory=ROOT/'evidence/setup-20261102/media-routing-run'
 result=orchestrate('coordinator','Create an image of a small red robot reading a book beside a blue lamp, detailed studio photograph, clean composition.',directory/'workspace',directory,10,1800,[],False,1)
 images=[c for c in result.get('child_results',[]) if c['recipient']=='image-studio' and c['status']=='completed']
 valid=False
 if images:
  output=images[0]['result'];path=ROOT/output['output']
  with Image.open(path) as image:image.verify()
  destination=ROOT/'agents/image-studio/outputs/local-smoke.png';destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,destination)
  data=json.loads(Path(output['evidence']).read_text());data['path']=str(destination);data['source_routed_output']=str(path);data['visual_inspection']='pending'
  destination.with_suffix('.evidence.json').write_text(json.dumps(data,indent=2));valid=True
 passed=bool(valid and result.get('status')=='model_finished')
 evidence={'passed':passed,'status':result.get('status'),'real_image_file_valid':valid,'visual_quality':'requires separate inspection','model_driven_media_routing_verified':passed}
 (directory.parent/'media-routing-acceptance.json').write_text(json.dumps(evidence,indent=2))
 print(json.dumps(evidence));return 0 if passed else 1

if __name__=='__main__':
 raise SystemExit(main())
