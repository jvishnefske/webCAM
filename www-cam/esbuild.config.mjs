import * as esbuild from 'esbuild';

const watch = process.argv.includes('--watch');

const wasmExternalPlugin = {
  name: 'wasm-external',
  setup(build) {
    build.onResolve({ filter: /rustcam\.js$/ }, () => ({
      path: '../pkg/rustcam.js',
      external: true,
    }));
  },
};

const shared = {
  bundle: true,
  format: 'esm',
  sourcemap: true,
  target: 'es2022',
  logLevel: 'info',
  plugins: [wasmExternalPlugin],
};

const appCtx = await esbuild.context({
  ...shared,
  entryPoints: ['src/main.ts'],
  outfile: 'dist/main.js',
});

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
