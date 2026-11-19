/** Deterministic modality selection before an agent role is chosen. */
import { fileURLToPath } from 'node:url';
function closeWord(word,target,limit=2){
  if(!word||Math.abs(word.length-target.length)>limit||/(?:ed|ing)$/.test(word))return false;
  let previous=Array.from({length:target.length+1},(_,i)=>i);
  for(let i=1;i<=word.length;i++){
    const current=[i];let rowMin=i;
    for(let j=1;j<=target.length;j++){
      const value=Math.min(current[j-1]+1,previous[j]+1,previous[j-1]+(word[i-1]===target[j-1]?0:1));
      current[j]=value;rowMin=Math.min(rowMin,value);
    }
    if(rowMin>limit)return false;
    previous=current;
  }
  return previous[target.length]<=limit;
}

export function classifyRequest(text) {
  const normalized=String(text).toLowerCase().replace(/\s+/g,' ').trim();
  const generationVerb=/\b(generate|create|make|render|draw|design|produce|paint|illustrate|compose|synthesize)\b/;
  const leadingWords=normalized.match(/[a-z]+/g)?.slice(0,4)??[];
  const typoGeneration=leadingWords.some(word=>['generate','create','render','design','produce','illustrate'].some(target=>closeWord(word,target)));
  if(/\b(edit|modify|transform|restyle|change)\b/.test(normalized)&&/\b(video|clip|footage)\b/.test(normalized))return 'video-edit';
  if(/\b(video|animation|movie|clip|footage)\b/.test(normalized)&&generationVerb.test(normalized))return 'video';
  if(/\b(song|music|instrumental|soundtrack|beat)\b/.test(normalized)&&generationVerb.test(normalized))return 'music';
  if(/\b(sound effect|soundscape|ambience|ambient audio|foley)\b/.test(normalized)&&generationVerb.test(normalized))return 'effects';
  if(/\b(voiceover|voice over|narration|spoken audio|speech)\b/.test(normalized)&&generationVerb.test(normalized))return 'speech';
  const imageNoun=/\b(image|images|picture|pictures|photo|photos|photograph|photographs|illustration|illustrations|artwork|portrait|poster|wallpaper|logo)\b/;
  if(imageNoun.test(normalized)&&(generationVerb.test(normalized)||typoGeneration))return 'image';
  const visualScene=/\b(?:hyper[ -]?realistic|photo[ -]?realistic|cinematic|visually detailed|ultra[ -]?detailed|studio lighting|camera angle|depth of field|8k|4k)\b/;
  if(generationVerb.test(normalized)&&/\bscene\b/.test(normalized)&&visualScene.test(normalized))return 'image';
  if(/\b(cve-\d{4}-\d+|pentest|penetration test|exploit|vulnerability audit|attack surface|privilege escalation|reverse shell|sbom)\b/.test(normalized))return 'cyber';
  return null;
}

export function executionPlan(text, role, runDirectory,referenceImage=null,plannedModality=null) {
  const modality=plannedModality??(referenceImage?'image':classifyRequest(text));
  if(modality==='image')return {
    modality,
    label:referenceImage?'creative-director → image-edit':'creative-director → image-studio',
    args:['image',text,'--output',`${runDirectory}/generated.png`,'--preset','quality',...(referenceImage?['--reference-image',referenceImage]:[])],
    referenceImage,
  };
  if(['video','video-edit','music','effects','speech'].includes(modality))return {
    modality,label:`creative-director → ${modality}`,
    executable:process.execPath,
    args:[fileURLToPath(new URL('../private-inference/media-client.ts',import.meta.url)),modality,text,'--output',`${runDirectory}/generated.${modality==='video'||modality==='video-edit'?'mp4':'wav'}`],
    referenceImage:null,
  };
  return {modality:null,label:role,args:null,referenceImage:null};
}
