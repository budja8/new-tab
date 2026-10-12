// Fidelidad GL vs 2D de las animaciones nuevas (copia de la extensión + verify.html en vext/).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch, attach, evaluate, setBounds, sleep } from './lib2.mjs';

const dir = path.dirname(fileURLToPath(import.meta.url));
const B = await launch('C:/ntmeas/V-');
const { send, events } = B;
console.log('pid', B.pid, 'profile', B.profile);
const out = {};
try {
  const gi = await send('SystemInfo.getInfo');
  out.gpu = { devices: gi.gpu.devices.map((d) => [d.vendorString, d.deviceString, d.driverVersion]), glRenderer: gi.gpu.auxAttributes?.glRenderer };
  const id = (await send('Extensions.loadUnpacked', { path: path.join(dir, 'vext') })).id;
  const t = (await send('Target.createTarget', { url: `chrome-extension://${id}/verify.html` })).targetId;
  const s = await attach(send, t);
  out.bounds = await setBounds(send, t);
  const before = events.length;
  await send('Page.reload', {}, s);
  await sleep(2000);
  for (let k = 0; k < 40 && !(await evaluate(send, s, '!!window.__verify')); k++) await sleep(500);
  const t0 = Date.now();
  out.verify = await evaluate(send, s, 'window.__verify', true);
  out.seconds = (Date.now() - t0) / 1000;
  out.events = events.slice(before).filter((e) => e.sessionId === s && /exceptionThrown|consoleAPICalled|entryAdded/.test(e.method)).map((e) => JSON.stringify(e.params).slice(0, 600));
  await send('Target.closeTarget', { targetId: t });
} catch (e) { console.log('ERR', e.stack); out.fatal = String(e.stack); }
fs.writeFileSync(path.join(dir, 'verify-anim.json'), JSON.stringify(out, null, 1));
for (const r of out.verify?.results || []) console.log(r.mode, r.grid, 'cp', r.checkpoints, 'max', r.max, 'over2', r.over2, 'inv', JSON.stringify(r.afterInvalidate), 'syn', r.synthetic && `${r.synthetic.combos}/${r.synthetic.max}/${r.synthetic.over2}`, JSON.stringify(r.synthetic?.required), r.nonAscii?.join(' '));
if (out.verify?.fatal) console.log(out.verify.fatal);
console.log('profile removed', await B.close());
process.exit(0);
