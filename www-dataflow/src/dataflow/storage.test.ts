import { describe, test, expect, beforeEach, vi, afterEach } from 'vitest';
import {
  type SavedProject,
  serializeProject,
  listProjects,
  saveProject,
  loadProject,
  deleteProject,
  getActiveProjectName,
  setActiveProjectName,
  uniqueName,
  createAutoSave,
} from './storage.js';

// Mock localStorage
const store: Record<string, string> = {};
const localStorageMock = {
  getItem: (key: string) => store[key] ?? null,
  setItem: (key: string, value: string) => { store[key] = value; },
  removeItem: (key: string) => { delete store[key]; },
  clear: () => { for (const k in store) delete store[k]; },
} as Storage;

beforeEach(() => {
  localStorageMock.clear();
  vi.stubGlobal('localStorage', localStorageMock);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('serializeProject', () => {
  test('captures blocks with type and config, channels, positions, and viewport', () => {
    const snap = {
      blocks: [
        { id: 1, block_type: 'constant', name: 'C1', inputs: [], outputs: [], config: { value: 42 }, output_values: [] },
        { id: 2, block_type: 'gain', name: 'G1', inputs: [], outputs: [], config: { factor: 2 }, output_values: [] },
      ],
      channels: [
        { id: { 0: 10 }, from_block: { 0: 1 }, from_port: 0, to_block: { 0: 2 }, to_port: 0 },
      ],
      tick_count: 100,
      time: 1.0,
    };
    const positions = new Map<number, { x: number; y: number }>([[1, { x: 50, y: 100 }], [2, { x: 200, y: 100 }]]);
    const viewport = { panX: 10, panY: 20, scale: 1.5 };

    const result = serializeProject('My Project', snap, positions, viewport);

    expect(result.name).toBe('My Project');
    expect(result.lastModified).toBeTruthy();
    expect(result.graph.blocks).toEqual([
      { id: 1, blockType: 'constant', config: { value: 42 } },
      { id: 2, blockType: 'gain', config: { factor: 2 } },
    ]);
    expect(result.graph.channels).toEqual([
      { fromBlock: 1, fromPort: 0, toBlock: 2, toPort: 0 },
    ]);
    expect(result.positions).toEqual({ 1: { x: 50, y: 100 }, 2: { x: 200, y: 100 } });
    expect(result.viewport).toEqual({ panX: 10, panY: 20, scale: 1.5 });
  });

  test('handles empty graph', () => {
    const snap = { blocks: [], channels: [], tick_count: 0, time: 0 };
    const result = serializeProject('Empty', snap, new Map(), { panX: 0, panY: 0, scale: 1 });
    expect(result.graph.blocks).toEqual([]);
    expect(result.graph.channels).toEqual([]);
    expect(result.positions).toEqual({});
  });
});

describe('listProjects', () => {
  test('returns empty array when no projects saved', () => {
    expect(listProjects()).toEqual([]);
  });

  test('returns saved project names', () => {
    localStorage.setItem('webcam:projects', JSON.stringify(['A', 'B']));
    expect(listProjects()).toEqual(['A', 'B']);
  });

  test('returns empty array on corrupted data', () => {
    localStorage.setItem('webcam:projects', '{bad');
    expect(listProjects()).toEqual([]);
  });
});

describe('saveProject / loadProject', () => {
  const project: SavedProject = {
    name: 'Test',
    lastModified: new Date().toISOString(),
    graph: {
      blocks: [{ id: 1, blockType: 'constant', config: { value: 5 } }],
      channels: [],
    },
    positions: { 1: { x: 10, y: 20 } },
    viewport: { panX: 0, panY: 0, scale: 1 },
  };

  test('saveProject stores and lists the project', () => {
    saveProject(project);
    expect(listProjects()).toEqual(['Test']);
    const loaded = loadProject('Test');
    expect(loaded).not.toBeNull();
    expect(loaded!.graph.blocks).toEqual(project.graph.blocks);
  });

  test('saveProject updates existing project without duplicating name', () => {
    saveProject(project);
    saveProject({ ...project, lastModified: new Date().toISOString() });
    expect(listProjects()).toEqual(['Test']);
  });

  test('loadProject returns null for nonexistent project', () => {
    expect(loadProject('Nope')).toBeNull();
  });

  test('loadProject returns null for corrupted data', () => {
    localStorage.setItem('webcam:projects', JSON.stringify(['Bad']));
    localStorage.setItem('webcam:project:Bad', '{corrupt');
    expect(loadProject('Bad')).toBeNull();
  });
});

describe('deleteProject', () => {
  test('removes project from list and storage', () => {
    const project: SavedProject = {
      name: 'ToDelete',
      lastModified: new Date().toISOString(),
      graph: { blocks: [], channels: [] },
      positions: {},
      viewport: { panX: 0, panY: 0, scale: 1 },
    };
    saveProject(project);
    expect(listProjects()).toEqual(['ToDelete']);
    deleteProject('ToDelete');
    expect(listProjects()).toEqual([]);
    expect(loadProject('ToDelete')).toBeNull();
  });

  test('no-op for nonexistent project', () => {
    deleteProject('Ghost');
    expect(listProjects()).toEqual([]);
  });
});

describe('active project name', () => {
  test('returns null when nothing set', () => {
    expect(getActiveProjectName()).toBeNull();
  });

  test('set and get round-trips', () => {
    setActiveProjectName('MyProj');
    expect(getActiveProjectName()).toBe('MyProj');
  });
});

describe('uniqueName', () => {
  test('returns name as-is when no conflict', () => {
    expect(uniqueName('Foo', [])).toBe('Foo');
  });

  test('appends (2) on first conflict', () => {
    expect(uniqueName('Foo', ['Foo'])).toBe('Foo (2)');
  });

  test('increments past existing conflicts', () => {
    expect(uniqueName('Foo', ['Foo', 'Foo (2)', 'Foo (3)'])).toBe('Foo (4)');
  });
});

describe('createAutoSave', () => {
  test('debounces calls and invokes save callback after delay', () => {
    vi.useFakeTimers();
    const saveFn = vi.fn();
    const autoSave = createAutoSave(saveFn, 500);

    autoSave();
    autoSave();
    autoSave();
    expect(saveFn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(500);
    expect(saveFn).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });

  test('resets timer on each call', () => {
    vi.useFakeTimers();
    const saveFn = vi.fn();
    const autoSave = createAutoSave(saveFn, 500);

    autoSave();
    vi.advanceTimersByTime(300);
    autoSave(); // resets
    vi.advanceTimersByTime(300);
    expect(saveFn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(200);
    expect(saveFn).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });
});
