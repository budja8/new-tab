import { launch, attach, evaluate, sleep } from './lib.mjs';
const B = await launch('C:/ntdash/v');
const { send, events } = B;
try {
  const id = (await send('Extensions.loadUnpacked', { path: 'C:/Users/Daniel Rodriguez/Documents/Tests/owns/new-tab/extension' })).id;
  console.log('ext', id);
  const t = (await send('Target.createTarget', { url: `chrome-extension://${id}/newtab.html` })).targetId;
  const s = await attach(send, t);
  await sleep(2500);
  await evaluate(send, s, `chrome.storage.local.set({settings:{mode:'dashboard',kpiVisible:true}}).then(()=>1)`, true);
  const before = events.length;
  await send('Page.reload', {}, s); await sleep(4000);
  const expr = `(async()=>{
    const o={};
    const T=async(k,f)=>{try{o[k]=await f()}catch(e){o[k]='ERR '+e.name+': '+e.message}};
    o.typeofSystem=typeof chrome.system;
    o.keys=chrome.system?Object.keys(chrome.system):null;
    o.manifestPerms=chrome.runtime.getManifest().permissions;
    await T('permsAll',()=>chrome.permissions.getAll());
    await T('cpu',async()=>{const r=await chrome.system.cpu.getInfo();return {n:r.numOfProcessors,arch:r.archName,model:r.modelName,p0:r.processors[0]}});
    await T('mem',()=>chrome.system.memory.getInfo());
    await T('storage',()=>chrome.system.storage.getInfo());
    try{const u=await chrome.system.storage.getInfo();o.cap={};for(const x of u){try{o.cap[x.id+'|'+x.type+'|'+x.name]=await chrome.system.storage.getAvailableCapacity(x.id)}catch(e){o.cap[x.id]='ERR '+e.message}}}catch(e){}
    try{const m=await import('./src/sysinfo.js');const si=m.createSysinfo();await si.sample();await new Promise(r=>setTimeout(r,1100));await si.sample();o.data=Array.from(si.data());o.text=si.text()}catch(e){o.sysinfoErr=String(e)}
    return JSON.stringify(o,null,1)})()`;
  console.log(await evaluate(send, s, expr, true));
  console.log('EVENTS', events.slice(before).filter(e=>e.sessionId===s&&/exceptionThrown|consoleAPICalled|entryAdded/.test(e.method)).map(e=>JSON.stringify(e.params).slice(0,400)).join('\n'));
  console.log('DOM', await evaluate(send, s, `document.body.innerText.slice(0,500)`));
} catch (e) { console.log('ERR', e.stack); }
await B.close(); process.exit(0);
