import crypto from 'node:crypto';
import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';import assert from 'node:assert/strict';import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),source=fs.readFileSync(path.join(root,'reports/local-agent-models.html'),'utf8');
const ids=[...source.matchAll(/\bid="([^"]+)"/g)].map(m=>m[1]);assert.equal(new Set(ids).size,ids.length);
for(const m of source.matchAll(/href="#([^"]+)"/g))assert(ids.includes(m[1]));
assert(!/<(?:script|img|source|link)[^>]+(?:src|href)="https?:/i.test(source));assert.equal((source.match(/<svg\b/g)||[]).length,4);
class Element{constructor(text='',dataset={}){this.textContent=text;this.dataset=dataset;this.attrs={};this.hidden=false;this.value='';this.children=[];this.style={};this.classList={toggle:()=>{}}}setAttribute(k,v){this.attrs[k]=v}removeAttribute(k){delete this.attrs[k]}replaceChildren(){this.children=[]}append(...v){this.children.push(...v)}focus(){this.focused=true}}
const elements=Object.fromEntries(ids.map(id=>['#'+id,new Element()]));elements['#report-data'].textContent=source.match(/<script id="report-data" type="application\/json">([\s\S]*?)<\/script>/)[1];
const rows=[...source.matchAll(/<tr data-category="([^"]+)">([\s\S]*?)<\/tr>/g)].map(m=>new Element(m[2].replace(/<[^>]+>/g,' '),{category:m[1]}));
const filters=['all','passed','open'].map(filter=>new Element('',{filter})),nodes=[...source.matchAll(/data-component="([^"]+)"/g)].map(m=>new Element('',{component:m[1]}));
const circles=Array.from({length:5},()=>new Element()),details=[new Element()],nav=['outcome','architecture','lifecycle','checks','gates'].map(id=>Object.assign(new Element(),{hash:'#'+id})),sections=nav.map(n=>({id:n.hash.slice(1)}));
const html={dataset:{},scrollHeight:1000},events={},storage=new Map();let prints=0;
const document={documentElement:html,activeElement:{tagName:'BODY'},querySelector:s=>elements[s]||new Element(),querySelectorAll:s=>({'#results tbody tr':rows,'[data-filter]':filters,'[data-component]':nodes,'#stage-dots circle':circles,'details':details,'.hud nav a':nav,'main>section':sections}[s]||[]),createElement:()=>new Element(),addEventListener:(k,f)=>events[k]=f};
const context={document,window:{print:()=>prints++,addEventListener:(k,f)=>events[k]=f},localStorage:{getItem:k=>storage.get(k),setItem:(k,v)=>storage.set(k,v)},IntersectionObserver:class{constructor(f){this.f=f}observe(target){this.f([{target,isIntersecting:true}])}},CSS:{supports:()=>false},scrollY:100,innerHeight:500};
vm.runInNewContext(fs.readFileSync(path.join(root,'shared/bin/report.js'),'utf8'),context);
for(const f of filters){f.onclick();assert.equal(rows.filter(r=>!r.hidden).length,f.dataset.filter==='all'?rows.length:rows.filter(r=>r.dataset.category===f.dataset.filter).length)}
filters[0].onclick();elements['#search'].value='Wan video';elements['#search'].oninput();assert.equal(rows.filter(r=>!r.hidden).length,1);
elements['#theme'].onchange({target:{value:'dark'}});assert.equal(html.dataset.theme,'dark');assert.equal(storage.get('agent-theme'),'dark');elements['#theme'].onchange({target:{value:'light'}});assert.equal(html.dataset.theme,'light');
elements['#density'].onclick();assert.equal(html.dataset.density,'compact');
elements['#next'].onclick();assert.equal(elements['#step-status'].textContent,'Stage 2 of 5');elements['#prev'].onclick();assert.equal(elements['#step-status'].textContent,'Stage 1 of 5');
nodes[0].onclick();assert.equal(elements['#inspector'].children.length,2);nodes[1].onkeydown({key:'Enter',preventDefault(){}});assert.equal(elements['#inspector'].children[0].textContent,'Job supervisor');
events.keydown({key:'/',preventDefault(){}});assert(elements['#search'].focused);events.beforeprint();assert(details[0].open);elements['#print'].onclick();assert.equal(prints,1);
for(const s of ['prefers-reduced-motion','@media print','@container','light-dark','--bg-canvas',':focus-visible'])assert(source.includes(s));
const receipt={passed:true,verified_utc:new Date().toISOString(),report_sha256:crypto.createHash('sha256').update(source).digest('hex'),scope:'Static structure plus Node DOM-contract tests; not a rendered-browser visual acceptance',checks:['Unique IDs and valid anchors','Four inline SVG diagrams','No external assets','Passed/open filtering and text search','Persisted dark/light controls','Density toggle','Lifecycle stepper','Keyboard node inspector and search shortcut','Print expansion','Responsive/reduced-motion/focus styles'],rendered_ui_verification:'Separate visual inspection required'};
fs.writeFileSync(path.join(root,'evidence/setup-20261102/report-static-checks.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));
