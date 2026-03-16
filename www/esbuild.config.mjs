import * as esbuild from 'esbuild';

const watch = process.argv.includes('--watch');

/** @type {esbuild.BuildOptions} */
const shared = {
  bundle: true,
  format: 'esm',
  sourcemap: true,
  target: 'es2022',
  logLevel: 'info',
};

// Main app bundle
const appCtx = await esbuild.context({
  ...shared,
  entryPoints: ['src/main.ts'],
  outfile: 'dist/main.js',
  // WASM is loaded at runtime, treat as external
  external: ['../pkg/rustcam.js'],
});

// Web Worker bundle
const workerCtx = await esbuild.context({
  ...shared,
  entryPoints: ['src/worker.ts'],
  outfile: 'dist/worker.js',
  external: ['../pkg/rustcam.js'],
});

if (watch) {
  await appCtx.watch();
  await workerCtx.watch();
  console.log('Watching for changes...');
} else {
  await appCtx.rebuild();
  await workerCtx.rebuild();
  await appCtx.dispose();
  await workerCtx.dispose();
}
