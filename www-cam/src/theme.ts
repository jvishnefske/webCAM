/** Shared design tokens — single source of truth for all colors, fonts, and spacing. */

export const theme = {
  colors: {
    bg: '#0f1117',
    surface: '#1a1d27',
    border: '#2a2d3a',
    accent: '#4f8cff',
    accentDim: '#2d5299',
    text: '#e0e0e8',
    textDim: '#8888a0',
    danger: '#ff5555',
    success: '#55ff88',
    warning: '#ff9800',
    white: '#ffffff',

    // Port type colors (dataflow)
    portFloat: '#4f8cff',
    portBytes: '#ff9800',
    portText: '#55ff88',
    portSeries: '#ff55aa',
    portAny: '#aaaaaa',

    // Wire colors
    wire: '#4f8cff66',
    wireActive: '#4f8cff',

    // Constraint status colors
    cstFullyConstrained: '#4caf50',
    cstOverConstrained: '#f44336',
    cstUnderConstrained: '#ff9800',
    cstPickHighlight: '#ffeb3b',

    // Simulation colors
    simMaterialRemoval: 'rgba(255,80,80,0.35)',
    simRapid: 'rgba(255,255,100,0.25)',
    simCutting: 'rgba(79,140,255,0.6)',
    simToolShadow: 'rgba(0,0,0,0.3)',
    simToolCutting: 'rgba(255,80,80,0.7)',
    simToolIdle: 'rgba(100,200,100,0.5)',
    simToolOutlineCutting: '#ff5555',
    simToolOutlineIdle: '#55ff88',
    simToolCenter: '#ffffff',

    // CAM preview colors
    camLaser: 'rgba(255, 60, 40, 0.85)',
    camZDefault: '#4f8cff',

    // Hover state
    btnSecondaryHover: '#3a3d4a',

    // Drop zone hover
    dropZoneHoverBg: 'rgba(79,140,255,0.06)',
  },

  fonts: {
    sans: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    mono: "'Courier New', monospace",
  },

  spacing: {
    sidebarWidth: 320,
    headerHeight: 49,
    pad: 16,
    portRadius: 6,
    portSpacing: 20,
    portOffsetY: 30,
    nodeWidth: 140,
    nodeBaseHeight: 40,
  },
} as const;

/** Return a port color from the theme based on port kind. */
export function portColor(kind: string): string {
  switch (kind) {
    case 'Float': return theme.colors.portFloat;
    case 'Bytes': return theme.colors.portBytes;
    case 'Text': return theme.colors.portText;
    case 'Series': return theme.colors.portSeries;
    default: return theme.colors.portAny;
  }
}
