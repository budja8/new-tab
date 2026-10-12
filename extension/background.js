import { createKpiStore } from './src/store.js';

const store = createKpiStore(chrome.storage.local);

// Los content scripts mandan lotes de conteos; solo se aceptan los de esta extensión.
chrome.runtime.onMessage.addListener((message, sender) => {
  if (sender.id !== chrome.runtime.id) return false;
  if (message && message.type === 'keys') {
    store.record(message.batch).catch((err) => console.error('No se pudo guardar el lote de teclas', err));
  }
  return false;
});
