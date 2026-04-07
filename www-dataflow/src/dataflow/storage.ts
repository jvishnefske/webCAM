/** LocalStorage persistence for dataflow projects with hierarchical sheets. */

import type { GraphSnapshot, NodePosition } from './types.js';

// ── Types ────────────────────────────────────────────────────────────

export type SheetType = 'dataflow' | 'bsp';

export interface DataflowSheetData {
  graph: {
    blocks: Array<{ id: number; blockType: string; config: Record<string, unknown> }>;
    channels: Array<{ fromBlock: number; fromPort: number; toBlock: number; toPort: number }>;
  };
  positions: Record<number, { x: number; y: number }>;
  viewport: { panX: number; panY: number; scale: number };
}

export interface BspSheetData {
  mcuFamily: string;
  pinAssignments: Array<{ pin: string; function: string }>;
}

export interface Sheet {
  id: string;
  label: string;
  type: SheetType;
  parentId: string | null;
  data: DataflowSheetData | BspSheetData;
}

export interface Project {
  name: string;
  lastModified: string;
  activeSheet: string;
  sheets: Record<string, Sheet>;
}

// ── Helpers ──────────────────────────────────────────────────────────

/** Unwrap a value that may be a plain number or a newtype wrapper {0: number}. */
function unwrapId(v: number | { 0: number }): number {
  return typeof v === 'number' ? v : v[0];
}

const KEY_PROJECTS = 'webcam:projects';
const KEY_ACTIVE = 'webcam:active';
const projectKey = (name: string) => `webcam:project:${name}`;

// ── Serialization ────────────────────────────────────────────────────

/** Serialize a dataflow graph snapshot into sheet data. */
export function serializeDataflowSheet(
  snap: GraphSnapshot,
  positions: Map<number, NodePosition>,
  viewport: { panX: number; panY: number; scale: number },
): DataflowSheetData {
  return {
    graph: {
      blocks: snap.blocks.map(b => ({
        id: b.id,
        blockType: b.block_type,
        config: b.config,
      })),
      channels: snap.channels.map(c => ({
        fromBlock: unwrapId(c.from_block),
        fromPort: c.from_port,
        toBlock: unwrapId(c.to_block),
        toPort: c.to_port,
      })),
    },
    positions: Object.fromEntries(positions),
    viewport,
  };
}

/**
 * Legacy helper: serialize a full project in the old flat format.
 * Kept for backward compatibility with callers that haven't migrated yet.
 */
export function serializeProject(
  name: string,
  snap: GraphSnapshot,
  positions: Map<number, NodePosition>,
  viewport: { panX: number; panY: number; scale: number },
): Project {
  const data = serializeDataflowSheet(snap, positions, viewport);
  return {
    name,
    lastModified: new Date().toISOString(),
    activeSheet: 'main',
    sheets: {
      main: { id: 'main', label: 'Main', type: 'dataflow', parentId: null, data },
    },
  };
}

// ── Project CRUD ─────────────────────────────────────────────────────

/** Create a new project with one empty "main" dataflow sheet. */
export function createProject(name: string): Project {
  const data: DataflowSheetData = {
    graph: { blocks: [], channels: [] },
    positions: {},
    viewport: { panX: 0, panY: 0, scale: 1 },
  };
  return {
    name,
    lastModified: new Date().toISOString(),
    activeSheet: 'main',
    sheets: {
      main: { id: 'main', label: 'Main', type: 'dataflow', parentId: null, data },
    },
  };
}

/** Add a sheet to a project. Returns the new sheet. */
export function addSheet(
  project: Project,
  id: string,
  label: string,
  type: SheetType,
  parentId?: string,
): Sheet {
  const data: DataflowSheetData | BspSheetData = type === 'bsp'
    ? { mcuFamily: '', pinAssignments: [] }
    : { graph: { blocks: [], channels: [] }, positions: {}, viewport: { panX: 0, panY: 0, scale: 1 } };
  const sheet: Sheet = { id, label, type, parentId: parentId ?? null, data };
  project.sheets[id] = sheet;
  return sheet;
}

/** Remove a sheet from a project. Returns false if sheetId is "main". */
export function removeSheet(project: Project, sheetId: string): boolean {
  if (sheetId === 'main') return false;
  if (!(sheetId in project.sheets)) return false;
  delete project.sheets[sheetId];
  // If the removed sheet was active, fall back to main
  if (project.activeSheet === sheetId) {
    project.activeSheet = 'main';
  }
  return true;
}

// ── Migration ────────────────────────────────────────────────────────

/** Migrate old flat SavedProject format to hierarchical Project. */
export function migrateProject(raw: Record<string, unknown>): Project {
  if (raw.sheets) return raw as unknown as Project; // already new format

  // Old format: raw is SavedProject with .graph, .positions, .viewport
  const oldGraph = (raw.graph ?? { blocks: [], channels: [] }) as DataflowSheetData['graph'];
  const oldPositions = (raw.positions ?? {}) as Record<number, { x: number; y: number }>;
  const oldViewport = (raw.viewport ?? { panX: 0, panY: 0, scale: 1 }) as DataflowSheetData['viewport'];

  const data: DataflowSheetData = {
    graph: oldGraph,
    positions: oldPositions,
    viewport: oldViewport,
  };
  return {
    name: raw.name as string,
    lastModified: (raw.lastModified as string) ?? new Date().toISOString(),
    activeSheet: 'main',
    sheets: {
      main: { id: 'main', label: 'Main', type: 'dataflow', parentId: null, data },
    },
  };
}

// ── LocalStorage persistence ─────────────────────────────────────────

export function listProjects(): string[] {
  try {
    const raw = localStorage.getItem(KEY_PROJECTS);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export function saveProject(project: Project): void {
  const names = listProjects();
  if (!names.includes(project.name)) {
    names.push(project.name);
    localStorage.setItem(KEY_PROJECTS, JSON.stringify(names));
  }
  localStorage.setItem(projectKey(project.name), JSON.stringify(project));
}

export function loadProject(name: string): Project | null {
  try {
    const raw = localStorage.getItem(projectKey(name));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    return migrateProject(parsed);
  } catch {
    return null;
  }
}

export function deleteProject(name: string): void {
  const names = listProjects().filter(n => n !== name);
  localStorage.setItem(KEY_PROJECTS, JSON.stringify(names));
  localStorage.removeItem(projectKey(name));
}

export function getActiveProjectName(): string | null {
  return localStorage.getItem(KEY_ACTIVE);
}

export function setActiveProjectName(name: string): void {
  localStorage.setItem(KEY_ACTIVE, name);
}

export function uniqueName(base: string, existing: string[]): string {
  if (!existing.includes(base)) return base;
  let i = 2;
  while (existing.includes(`${base} (${i})`)) i++;
  return `${base} (${i})`;
}

export function createAutoSave(saveFn: () => void, delayMs = 1000): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return () => {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      saveFn();
    }, delayMs);
  };
}
