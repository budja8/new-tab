// Holgura para el jitter de requestAnimationFrame: sin ella, 60 FPS en un monitor
// de 60 Hz se degradaría a 30 cuando un tick llega 1 ms antes de lo esperado.
const SLACK_MS = 2;

export function createLimiter(fps) {
  const interval = 1000 / fps;
  let next = 0;
  return {
    /** ¿Toca pintar un frame en este tick de rAF? */
    ready(nowMs) {
      if (next !== 0 && nowMs < next - SLACK_MS) return false;
      // Si nos atrasamos más de un frame, re-sincroniza en vez de hacer ráfaga.
      next = next === 0 || nowMs - next > interval ? nowMs + interval : next + interval;
      return true;
    },
    reset() {
      next = 0;
    },
  };
}
