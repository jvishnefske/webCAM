import { describe, test, expect, beforeEach, vi, afterEach } from 'vitest';
import {
  type DataflowSheetData,
  serializeProject,
  serializeDataflowSheet,
  listProjects,
  saveProject,
  loadProject,
  deleteProject,
  getActiveProjectName,
  setActiveProjectName,
  uniqueName,
  createAutoSave,
  createProject,
  addSheet,
  removeSheet,
  migrateProject,
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

describe('serializeDataflowSheet', () => {
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

    const result = serializeDataflowSheet(snap, positions, viewport);

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
    const result = serializeDataflowSheet(snap, new Map(), { panX: 0, panY: 0, scale: 1 });
    expect(result.graph.blocks).toEqual([]);
    expect(result.graph.channels).toEqual([]);
    expect(result.positions).toEqual({});
  });
});

describe('serializeProject', () => {
  test('wraps snapshot into project with main sheet', () => {
    const snap = {
      blocks: [
        { id: 1, block_type: 'constant', name: 'C1', inputs: [], outputs: [], config: { value: 42 }, output_values: [] },
      ],
      channels: [],
      tick_count: 0,
      time: 0,
    };
    const positions = new Map<number, { x: number; y: number }>([[1, { x: 50, y: 100 }]]);
    const viewport = { panX: 0, panY: 0, scale: 1 };

    const result = serializeProject('My Project', snap, positions, viewport);

    expect(result.name).toBe('My Project');
    expect(result.lastModified).toBeTruthy();
    expect(result.activeSheet).toBe('main');
    expect(result.sheets['main']).toBeDefined();
    expect(result.sheets['main'].type).toBe('dataflow');
    const data = result.sheets['main'].data as DataflowSheetData;
    expect(data.graph.blocks).toEqual([
      { id: 1, blockType: 'constant', config: { value: 42 } },
    ]);
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
  test('saveProject stores and lists the project', () => {
    const project = createProject('Test');
    saveProject(project);
    expect(listProjects()).toEqual(['Test']);
    const loaded = loadProject('Test');
    expect(loaded).not.toBeNull();
    expect(loaded!.sheets['main']).toBeDefined();
  });

  test('saveProject updates existing project without duplicating name', () => {
    const project = createProject('Test');
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

  test('loadProject migrates old flat format', () => {
    const old = {
      name: 'Legacy',
      lastModified: '2026-01-01T00:00:00Z',
      graph: { blocks: [{ id: 1, blockType: 'constant', config: { value: 5 } }], channels: [] },
      positions: { 1: { x: 10, y: 20 } },
      viewport: { panX: 0, panY: 0, scale: 1 },
    };
    localStorage.setItem('webcam:projects', JSON.stringify(['Legacy']));
    localStorage.setItem('webcam:project:Legacy', JSON.stringify(old));
    const loaded = loadProject('Legacy');
    expect(loaded).not.toBeNull();
    expect(loaded!.sheets['main']).toBeDefined();
    expect(loaded!.activeSheet).toBe('main');
    const data = loaded!.sheets['main'].data as DataflowSheetData;
    expect(data.graph.blocks.length).toBe(1);
  });
});

describe('deleteProject', () => {
  test('removes project from list and storage', () => {
    const project = createProject('ToDelete');
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

describe('hierarchical sheet management', () => {
  test('createProject has main sheet', () => {
    const p = createProject('Test');
    expect(p.sheets['main']).toBeDefined();
    expect(p.sheets['main'].type).toBe('dataflow');
    expect(p.activeSheet).toBe('main');
  });

  test('addSheet creates new sheet', () => {
    const p = createProject('Test');
    addSheet(p, 'sub1', 'Sub Graph', 'dataflow');
    expect(p.sheets['sub1']).toBeDefined();
    expect(p.sheets['sub1'].label).toBe('Sub Graph');
  });

  test('addSheet with parent', () => {
    const p = createProject('Test');
    addSheet(p, 'bsp1', 'Pico BSP', 'bsp', 'main');
    expect(p.sheets['bsp1'].parentId).toBe('main');
  });

  test('addSheet creates empty bsp data for bsp type', () => {
    const p = createProject('Test');
    const sheet = addSheet(p, 'bsp1', 'Pico BSP', 'bsp');
    expect(sheet.type).toBe('bsp');
    expect((sheet.data as { mcuFamily: string }).mcuFamily).toBe('');
  });

  test('addSheet creates empty dataflow data for dataflow type', () => {
    const p = createProject('Test');
    const sheet = addSheet(p, 'sub1', 'Sub', 'dataflow');
    expect(sheet.type).toBe('dataflow');
    const data = sheet.data as DataflowSheetData;
    expect(data.graph.blocks).toEqual([]);
    expect(data.graph.channels).toEqual([]);
  });

  test('removeSheet cannot delete main', () => {
    const p = createProject('Test');
    expect(removeSheet(p, 'main')).toBe(false);
  });

  test('removeSheet deletes sub-sheet', () => {
    const p = createProject('Test');
    addSheet(p, 'sub1', 'Sub', 'dataflow');
    expect(removeSheet(p, 'sub1')).toBe(true);
    expect(p.sheets['sub1']).toBeUndefined();
  });

  test('removeSheet returns false for nonexistent sheet', () => {
    const p = createProject('Test');
    expect(removeSheet(p, 'nonexistent')).toBe(false);
  });

  test('removeSheet resets activeSheet to main if active sheet deleted', () => {
    const p = createProject('Test');
    addSheet(p, 'sub1', 'Sub', 'dataflow');
    p.activeSheet = 'sub1';
    removeSheet(p, 'sub1');
    expect(p.activeSheet).toBe('main');
  });

  test('migrateProject wraps old format', () => {
    const old = {
      name: 'Old',
      lastModified: '2026-01-01T00:00:00Z',
      graph: { blocks: [{ id: 1, blockType: 'constant', config: { value: 1 } }], channels: [] },
      positions: { 1: { x: 10, y: 20 } },
      viewport: { panX: 0, panY: 0, scale: 1 },
    };
    const p = migrateProject(old);
    expect(p.sheets['main']).toBeDefined();
    expect(p.sheets['main'].type).toBe('dataflow');
    expect((p.sheets['main'].data as DataflowSheetData).graph.blocks.length).toBe(1);
  });

  test('migrateProject passes through new format', () => {
    const p = createProject('New');
    const migrated = migrateProject(p as unknown as Record<string, unknown>);
    expect(migrated).toBe(p);
  });

  test('migrateProject handles missing fields', () => {
    const old = { name: 'Bare' };
    const p = migrateProject(old);
    expect(p.sheets['main']).toBeDefined();
    expect(p.activeSheet).toBe('main');
    const data = p.sheets['main'].data as DataflowSheetData;
    expect(data.graph.blocks).toEqual([]);
    expect(data.positions).toEqual({});
  });
});
