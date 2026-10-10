export const MODES = ['fire', 'rain', 'starfield', 'pipes', 'dna', 'synthwave', 'compile_sim', 'pacman_install', 'dashboard', 'pacman', 'image'];

export function normalizeSettings(raw) {
  return {
    mode: MODES.includes(raw?.mode) ? raw.mode : 'fire',
    kpiVisible: raw?.kpiVisible !== false,
  };
}

export async function loadSettings(area) {
  const got = await area.get('settings');
  return normalizeSettings(got.settings);
}

export function saveSettings(area, settings) {
  return area.set({ settings: normalizeSettings(settings) });
}
