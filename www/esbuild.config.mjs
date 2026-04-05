import * as esbuild from 'esbuild';

const watch = process.argv.includes('--watch');

// Rewrite WASM module imports to correct paths relative to dist/
const wasmExternalPlugin = {
  name: 'wasm-external',
  setup(build) {
    build.onResolve({ filter: /rustcam\.js$/ }, () => ({
      path: '../pkg/rustcam.js',
      external: true,
    }));
    build.onResolve({ filter: /rustsim\.js$/ }, () => ({
      path: '../pkg/rustsim.js',
      external: true,
    }));
  },
};

/** @type {esbuild.BuildOptions} */
const shared = {
  bundle: true,
  format: 'esm',
  sourcemap: true,
  target: 'es2022',
  logLevel: 'info',
  plugins: [wasmExternalPlugin],
};

// Main app bundle
const appCtx = await esbuild.context({
  ...shared,
  entryPoints: ['src/main.ts'],
  outfile: 'dist/main.js',
});

// Web Worker bundle
const workerCtx = await esbuild.context({
  ...shared,
  entryPoints: ['src/worker.ts'],
  outfile: 'dist/worker.js',
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
