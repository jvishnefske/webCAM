import { describe, test, expect } from 'vitest';
import * as fs from 'node:fs';
import * as path from 'node:path';

describe('WASM init', () => {
  test('main.ts passes explicit WASM path to init()', () => {
    const mainTs = fs.readFileSync(
      path.resolve(__dirname, 'main.ts'),
      'utf-8'
    );
    // init must be called with an explicit path to the .wasm file,
    // not rely on import.meta.url resolution which breaks when
    // the JS bundle is served from /dist/ but the wasm lives in /pkg/.
    expect(mainTs).toContain("init(");
    expect(mainTs).toContain("rustsim_bg.wasm");
    // Must NOT call bare init() without arguments
    expect(mainTs).not.toMatch(/\binit\(\)\s*\./);
  });

  test('built bundle imports rustsim.js from ../pkg/', () => {
    const distMain = path.resolve(__dirname, '..', 'dist', 'main.js');
    if (!fs.existsSync(distMain)) return; // skip if not built
    const bundle = fs.readFileSync(distMain, 'utf-8');
    expect(bundle).toContain('../pkg/rustsim.js');
  });

  test('WASM binary exists in pkg/', () => {
    const wasmPath = path.resolve(__dirname, '..', 'pkg', 'rustsim_bg.wasm');
    expect(fs.existsSync(wasmPath)).toBe(true);
  });
});
