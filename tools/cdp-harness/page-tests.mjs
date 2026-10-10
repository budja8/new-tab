// TASK 6: capturas de la página real, respaldo 2D, pérdida de contexto y fallo forzado del GlRenderer.
// Uso: node page-tests.mjs normal|nogpu|nogl|broken
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch, evaluate, openExt, sleep } from './lib2.mjs';

const dir = path.dirname(fileURLToPath(import.meta.url));
const shots = path.join(dir, 'shots-final');
fs.mkdirSync(shots, { recursive: true });
const kind = process.argv[2] || 'normal';
const FLAGS = { normal: [], nogpu: ['--disable-gpu'], nogl: ['--use-gl=disabled'], broken: [] };
const EXT = kind === 'broken' ? 'final-broken' : 'final';
const B = await launch(`C:/ntfinal/${kind[0]}`, FLAGS[kind]);
const { send, events } = B;
console.log('pid', B.pid, 'profile', B.profile, 'flags', FLAGS[kind]);
const out = { kind, flags: FLAGS[kind], ext: EXT, pages: {} };

const consoleOf = (from, s) => events.slice(from).filter((e) => e.sessionId === s && /exceptionThrown|consoleAPICalled|entryAdded/.test(e.method)).map((e) => {
  const p = e.params;
  if (e.method === 'Runtime.consoleAPICalled') return `${p.type}: ` + p.args.map((a) => a.value ?? a.description ?? a.type).join(' ');
  if (e.method === 'Runtime.exceptionThrown') return 'EXCEPTION: ' + (p.exceptionDetails.exception?.description || p.exceptionDetails.text);
  return `log.${p.entry.level}: ${p.entry.text} ${p.entry.url || ''}`;
});

// Analiza una captura en la propia página: % de píxeles distintos del fondo #0b0d10 y luminancia media.
async function shot(s, name) {
  const r = await send('Page.captureScreenshot', { format: 'png' }, s);
  const f = path.join(shots, `${kind}-${name}.png`);
  fs.writeFileSync(f, Buffer.from(r.data, 'base64'));
  const stats = await evaluate(send, s, `(async()=>{const b=await (await fetch('data:image/png;base64,${r.data}')).blob();const bm=await createImageBitmap(b);const c=new OffscreenCanvas(bm.width,bm.height);const g=c.getContext('2d');g.drawImage(bm,0,0);const d=g.getImageData(0,0,bm.width,bm.height).data;let n=0,l=0,red=0;for(let i=0;i<d.length;i+=4){const dd=Math.max(Math.abs(d[i]-11),Math.abs(d[i+1]-13),Math.abs(d[i+2]-16));if(dd>12)n++;l+=d[i]+d[i+1]+d[i+2];if(d[i]>120&&d[i]>d[i+2]*2)red++;}return {w:bm.width,h:bm.height,nonBgPct:+(n/(d.length/4)*100).toFixed(2),meanRGB:+(l/(d.length/4)/3).toFixed(2),reddishPct:+(red/(d.length/4)*100).toFixed(2)}})()`, true);
  console.log('  shot', path.basename(f), JSON.stringify(stats));
  return { file: path.basename(f), ...stats };
}

const canvasInfo = (s) => evaluate(send, s, `JSON.stringify((()=>{const c=document.getElementById('bg');const cs=getComputedStyle(c);const r=c.getBoundingClientRect();return {w:c.width,h:c.height,styleW:c.style.width,styleH:c.style.height,position:cs.position,top:cs.top,left:cs.left,display:cs.display,rect:[r.x,r.y,r.width,r.height],firstChild:document.body.firstElementChild===c,count:document.querySelectorAll('canvas').length}})())`).then(JSON.parse);

const LOSE = `(()=>{const g=document.getElementById('bg').getContext('webgl2'); if(!g) return 'no webgl2 context on #bg'; const e=g.getExtension('WEBGL_lose_context'); e.loseContext(); setTimeout(()=>e.restoreContext(),500); return 'lost'})()`;

// Imagen empaquetada [rampa 0..9, r, g, b] de 160x60, degradado de 24 bits, guardada en IndexedDB como lo hace la página.
const STORE_IMG = `new Promise((res,rej)=>{const w=160,h=60,data=new Uint8Array(w*h*4);for(let y=0;y<h;y++)for(let x=0;x<w;x++){const o=(y*w+x)*4;data[o]=((x+y)>>3)%10;data[o+1]=x*255/w|0;data[o+2]=y*255/h|0;data[o+3]=((x+y)*3)&255;}
const q=indexedDB.open('newtab-ascii',1);q.onupgradeneeded=()=>q.result.createObjectStore('image');q.onerror=()=>rej(String(q.error));q.onsuccess=()=>{const db=q.result;const tx=db.transaction('image','readwrite');tx.objectStore('image').put({w,h,data},'current');tx.oncomplete=()=>{db.close();res('stored '+w+'x'+h)};tx.onerror=()=>rej(String(tx.error));};})`;

try {
  const gi = await send('SystemInfo.getInfo');
  out.gpu = { featureStatus: gi.gpu.featureStatus, glRenderer: gi.gpu.auxAttributes?.glRenderer, devices: gi.gpu.devices.map((d) => d.deviceString) };
  console.log('gpu', JSON.stringify(out.gpu));
  const id = (await send('Extensions.loadUnpacked', { path: path.join(dir, EXT) })).id;
  const url = `chrome-extension://${id}/newtab.html`;
  const modes = kind === 'normal' ? ['fire', 'rain', 'starfield', 'image'] : ['fire'];
  for (const mode of modes) {
    const page = {};
    const from = events.length;
    let o;
    if (mode === 'image') {
      o = await openExt(send, url, 'fire');
      page.store = await evaluate(send, o.s, STORE_IMG, true);
      await evaluate(send, o.s, `chrome.storage.local.set({settings:{mode:'image',kpiVisible:true}}).then(()=>'ok')`, true);
      const f2 = events.length;
      await send('Page.reload', {}, o.s);
      await sleep(2500);
      page.consoleAfterReload = consoleOf(f2, o.s);
      page.modeValue = await evaluate(send, o.s, `document.querySelector('#mode').value + ' | msg=' + document.getElementById('message').textContent`);
    } else o = await openExt(send, url, mode);
    const { s, targetId, info } = o;
    page.info = info;
    page.canvas = await canvasInfo(s);
    page.shots = [await shot(s, `${mode}-3s`)];
    if (mode !== 'image') { await sleep(9000); page.shots.push(await shot(s, `${mode}-12s`)); }
    if (mode === 'starfield') for (let k = 0; k < 4; k++) { await sleep(5000); page.shots.push(await shot(s, `${mode}-${17 + 5 * k}s`)); }
    if (kind === 'normal' && (mode === 'fire' || mode === 'image')) {
      page.lose = await evaluate(send, s, LOSE);
      await sleep(200);
      page.shots.push(await shot(s, `${mode}-lost-200ms`));
      await sleep(1800);
      page.shots.push(await shot(s, `${mode}-restored-2s`));
      await sleep(2000);
      page.shots.push(await shot(s, `${mode}-restored-4s`));
      page.canvasAfter = await canvasInfo(s);
    }
    page.console = consoleOf(from, s);
    out.pages[mode] = page;
    console.log(mode, JSON.stringify(info), '\n  console:', JSON.stringify(page.console), page.store || '', page.modeValue || '', page.lose || '');
    await send('Target.closeTarget', { targetId });
    await sleep(800);
  }
} catch (e) { console.log('ERR', e.stack); out.fatal = String(e.stack); }
fs.writeFileSync(path.join(dir, `page-tests-${kind}.json`), JSON.stringify(out, null, 1));
console.log('profile removed', await B.close());
process.exit(0);
