/** LocalStorage persistence for dataflow projects. */

import type { GraphSnapshot, NodePosition } from './types.js';

export interface SavedProject {
  name: string;
  lastModified: string;
  graph: {
    blocks: Array<{ id: number; blockType: string; config: Record<string, unknown> }>;
    channels: Array<{ fromBlock: number; fromPort: number; toBlock: number; toPort: number }>;
  };
  positions: Record<number, { x: number; y: number }>;
  viewport: { panX: number; panY: number; scale: number };
}

const KEY_PROJECTS = 'webcam:projects';
const KEY_ACTIVE = 'webcam:active';
const projectKey = (name: string) => `webcam:project:${name}`;

export function serializeProject(
  name: string,
  snap: GraphSnapshot,
  positions: Map<number, NodePosition>,
  viewport: { panX: number; panY: number; scale: number },
): SavedProject {
  return {
    name,
    lastModified: new Date().toISOString(),
    graph: {
      blocks: snap.blocks.map(b => ({
        id: b.id,
        blockType: b.block_type,
        config: b.config,
      })),
      channels: snap.channels.map(c => ({
        fromBlock: c.from_block[0],
        fromPort: c.from_port,
        toBlock: c.to_block[0],
        toPort: c.to_port,
      })),
    },
    positions: Object.fromEntries(positions),
    viewport,
  };
}

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

export function saveProject(project: SavedProject): void {
  const names = listProjects();
  if (!names.includes(project.name)) {
    names.push(project.name);
    localStorage.setItem(KEY_PROJECTS, JSON.stringify(names));
  }
  localStorage.setItem(projectKey(project.name), JSON.stringify(project));
}

export function loadProject(name: string): SavedProject | null {
  try {
    const raw = localStorage.getItem(projectKey(name));
    if (!raw) return null;
    return JSON.parse(raw) as SavedProject;
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
