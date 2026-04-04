/** Simple time-series plot renderer for plot blocks. */

import { theme } from '../theme.js';

const PLOT_PAD = 30;

export function drawPlot(
  canvas: HTMLCanvasElement,
  data: number[],
  label = 'Plot',
): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const rect = canvas.getBoundingClientRect();
  if (rect.width < 1) return;
  const dpr = window.devicePixelRatio || 1;
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;

  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const w = rect.width;
  const h = rect.height;

  // Background
  ctx.fillStyle = theme.colors.surface;
  ctx.fillRect(0, 0, w, h);

  if (data.length < 2) {
    ctx.fillStyle = theme.colors.textDim;
    ctx.font = '12px monospace';
    ctx.fillText('Waiting for data...', PLOT_PAD, h / 2);
    ctx.restore();
    return;
  }

  const plotW = w - PLOT_PAD * 2;
  const plotH = h - PLOT_PAD * 2;

  let min = data[0], max = data[0];
  for (const v of data) {
    if (v < min) min = v;
    if (v > max) max = v;
  }
  if (max === min) { max += 1; min -= 1; }

  // Axes
  ctx.strokeStyle = theme.colors.border;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(PLOT_PAD, PLOT_PAD);
  ctx.lineTo(PLOT_PAD, h - PLOT_PAD);
  ctx.lineTo(w - PLOT_PAD, h - PLOT_PAD);
  ctx.stroke();

  // Y-axis labels
  ctx.fillStyle = theme.colors.textDim;
  ctx.font = '10px monospace';
  ctx.textAlign = 'right';
  ctx.fillText(max.toFixed(2), PLOT_PAD - 4, PLOT_PAD + 4);
  ctx.fillText(min.toFixed(2), PLOT_PAD - 4, h - PLOT_PAD + 4);

  // Title
  ctx.textAlign = 'left';
  ctx.fillText(label, PLOT_PAD, PLOT_PAD - 8);

  // Data line
  ctx.strokeStyle = theme.colors.accent;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  for (let i = 0; i < data.length; i++) {
    const x = PLOT_PAD + (i / (data.length - 1)) * plotW;
    const y = PLOT_PAD + plotH - ((data[i] - min) / (max - min)) * plotH;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();

  ctx.restore();
}
