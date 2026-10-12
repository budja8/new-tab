import test from 'node:test';
import assert from 'node:assert/strict';
import { MODES, loadSettings, normalizeSettings, saveSettings } from '../extension/src/settings.js';

test('los valores por defecto son fuego con el panel visible', () => {
  assert.deepEqual(normalizeSettings(undefined), { mode: 'fire', kpiVisible: true });
  assert.deepEqual(normalizeSettings(null), { mode: 'fire', kpiVisible: true });
});

test('acepta cada modo válido', () => {
  for (const mode of MODES) assert.equal(normalizeSettings({ mode }).mode, mode);
});

test('un modo inválido cae a fuego', () => {
  for (const mode of ['nethack', '', 7, null, {}, '__proto__']) {
    assert.equal(normalizeSettings({ mode }).mode, 'fire');
  }
});

test('kpiVisible solo es falso si es exactamente false', () => {
  assert.equal(normalizeSettings({ kpiVisible: false }).kpiVisible, false);
  for (const v of [0, '', null, undefined, 'no']) {
    assert.equal(normalizeSettings({ kpiVisible: v }).kpiVisible, true);
  }
});

test('loadSettings y saveSettings usan la clave "settings" del área', async () => {
  const data = {};
  const area = {
    get: async (k) => (Object.hasOwn(data, k) ? { [k]: data[k] } : {}),
    set: async (o) => Object.assign(data, o),
  };
  assert.deepEqual(await loadSettings(area), { mode: 'fire', kpiVisible: true });
  await saveSettings(area, { mode: 'rain', kpiVisible: false, basura: 1 });
  assert.deepEqual(data.settings, { mode: 'rain', kpiVisible: false });
  assert.deepEqual(await loadSettings(area), { mode: 'rain', kpiVisible: false });
});

test('pipes es un modo válido', () => {
  assert.ok(MODES.includes('pipes'));
  assert.equal(normalizeSettings({ mode: 'pipes' }).mode, 'pipes');
});

test('synthwave es un modo válido', () => {
  assert.ok(MODES.includes('synthwave'));
  assert.equal(normalizeSettings({ mode: 'synthwave' }).mode, 'synthwave');
});

test('dna es un modo válido', () => {
  assert.ok(MODES.includes('dna'));
  assert.equal(normalizeSettings({ mode: 'dna' }).mode, 'dna');
});

test('compile_sim es un modo válido', () => {
  assert.ok(MODES.includes('compile_sim'));
  assert.equal(normalizeSettings({ mode: 'compile_sim' }).mode, 'compile_sim');
});

test('dashboard es un modo válido, antes de image', () => {
  assert.ok(MODES.includes('dashboard'));
  assert.ok(MODES.indexOf('pacman_install') < MODES.indexOf('dashboard'));
  assert.ok(MODES.indexOf('dashboard') < MODES.indexOf('image'));
  assert.equal(normalizeSettings({ mode: 'dashboard' }).mode, 'dashboard');
});

test('pacman_install es un modo válido', () => {
  assert.ok(MODES.includes('pacman_install'));
  assert.ok(MODES.indexOf('pacman_install') < MODES.indexOf('image'));
  assert.equal(normalizeSettings({ mode: 'pacman_install' }).mode, 'pacman_install');
});

test('mario es un modo válido, después de pacman y antes de image', () => {
  assert.ok(MODES.includes('mario'));
  assert.ok(MODES.indexOf('pacman') < MODES.indexOf('mario'));
  assert.ok(MODES.indexOf('mario') < MODES.indexOf('image'));
  assert.equal(normalizeSettings({ mode: 'mario' }).mode, 'mario');
});

test('pacman es un modo válido, antes de image', () => {
  assert.ok(MODES.includes('pacman'));
  assert.ok(MODES.indexOf('pacman') < MODES.indexOf('image'));
  assert.equal(normalizeSettings({ mode: 'pacman' }).mode, 'pacman');
});
