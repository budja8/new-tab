import { GlRenderer } from './gl-renderer.js';
import { fitsTexture } from './glyph-atlas.js';
import { Renderer, gridFor } from './renderer.js';

const SOFTWARE = /swiftshader|llvmpipe|softpipe|software|basic render/i;

export function isSoftwareRenderer(name) {
  return SOFTWARE.test(name ?? '');
}

/** Decide el backend según lo que el navegador puede hacer. Sin ganancia con GPU por software. */
export function chooseBackend({ hasWebGL2, rendererName, maxTextureSize, pw, ph }) {
  if (!hasWebGL2) return { backend: '2d', reason: 'WebGL2 no disponible' };
  if (isSoftwareRenderer(rendererName)) return { backend: '2d', reason: `GPU por software (${rendererName})` };
  if (!fitsTexture(maxTextureSize, pw, ph)) return { backend: '2d', reason: 'el atlas no cabe en una textura' };
  return { backend: 'gl', reason: rendererName || 'WebGL2' };
}

/** Sondea WebGL2 en un canvas desechable: un contexto fallido bloquearía el canvas real. */
export function probeWebGL2({ cellW = 9, cellH = 16 } = {}) {
  const { pw, ph } = gridFor(window.innerWidth, window.innerHeight, window.devicePixelRatio || 1, cellW, cellH);
  const gl = document.createElement('canvas').getContext('webgl2');
  if (!gl) return { hasWebGL2: false, pw, ph };
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  const info = {
    hasWebGL2: true,
    rendererName: ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : '',
    maxTextureSize: gl.getParameter(gl.MAX_TEXTURE_SIZE),
    pw,
    ph,
  };
  gl.getExtension('WEBGL_lose_context')?.loseContext();
  return info;
}

const replaceCanvas = (canvas) => {
  const fresh = canvas.cloneNode(false);
  canvas.replaceWith(fresh);
  return fresh;
};

/**
 * Crea el renderer WebGL2 si se puede y, si no, el 2D. Devuelve también el canvas final: si el
 * constructor de WebGL falla después de crear el contexto, el canvas original queda bloqueado y
 * se reemplaza por uno nuevo.
 */
export function createRenderer(canvas, opts = {}, deps = {}) {
  const {
    probe = probeWebGL2,
    makeGl = (c, o) => new GlRenderer(c, o),
    make2d = (c, o) => new Renderer(c, o),
    replaceCanvas: replace = replaceCanvas,
    warn = console.warn,
  } = deps;

  let info;
  try {
    info = probe(opts);
  } catch (err) {
    warn('[newtab] El sondeo de WebGL2 falló; se usa el renderer 2D:', err.message);
    info = { hasWebGL2: false };
  }
  let { backend, reason } = chooseBackend(info);
  if (backend === 'gl') {
    try {
      return { renderer: makeGl(canvas, opts), backend, reason, canvas };
    } catch (err) {
      warn('[newtab] WebGL falló; se usa el renderer 2D:', err.message);
      backend = '2d';
      reason = `WebGL falló: ${err.message}`;
      canvas = replace(canvas);
    }
  }
  return { renderer: make2d(canvas, opts), backend, reason, canvas };
}
