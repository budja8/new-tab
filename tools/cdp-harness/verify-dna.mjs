// TASK 6: verificación píxel a píxel del Renderer 2D real vs GlRenderer real (copia en verify2/).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch, attach, evaluate, setBounds, sleep } from './lib2.mjs';

const dir = path.dirname(fileURLToPath(import.meta.url));
const B = await launch('C:/ntdna/v');
const { send, events } = B;
console.log('pid', B.pid, 'profile', B.profile);
const out = {};
try {
  const gi = await send('SystemInfo.getInfo');
  fs.writeFileSync(path.join(dir, 'gpuinfo-dna.json'), JSON.stringify(gi, null, 1));
  out.gpu = { featureStatus: gi.gpu.featureStatus, devices: gi.gpu.devices.map((d) => [d.vendorString, d.deviceString, d.driverVersion]), glRenderer: gi.gpu.auxAttributes?.glRenderer };
  console.log('gpu', JSON.stringify(out.gpu));
  const id = (await send('Extensions.loadUnpacked', { path: path.join(dir, 'verify4') })).id;
  const t = (await send('Target.createTarget', { url: `chrome-extension://${id}/verify.html` })).targetId;
  const s = await attach(send, t);
  out.bounds = await setBounds(send, t);
  if (process.env.DSF) await send('Emulation.setDeviceMetricsOverride', { width: 0, height: 0, deviceScaleFactor: +process.env.DSF, mobile: false }, s);
  const before = events.length;
  await send('Page.reload', {}, s);
  await sleep(2000);
  for (let k = 0; k < 40 && !(await evaluate(send, s, '!!window.__verify')); k++) await sleep(500);
  const res = await evaluate(send, s, 'window.__verify', true);
  if (res === undefined) out.diag = await evaluate(send, s, 'JSON.stringify({u:location.href,rs:document.readyState,v:typeof window.__verify})');
  out.verify = res;
  out.events = events.slice(before).filter((e) => e.sessionId === s && /exceptionThrown|consoleAPICalled|entryAdded/.test(e.method)).map((e) => JSON.stringify(e.params).slice(0, 600));
  await send('Target.closeTarget', { targetId: t });
} catch (e) { console.log('ERR', e.stack); out.fatal = String(e.stack); }
fs.writeFileSync(path.join(dir, process.env.DSF ? 'verify-dna-dsf' + process.env.DSF + '.json' : 'verify-dna.json'), JSON.stringify(out, null, 1));
console.log('profile removed', await B.close());
process.exit(0);
