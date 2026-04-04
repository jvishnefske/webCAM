import * as esbuild from 'esbuild';

const watch = process.argv.includes('--watch');

const wasmExternalPlugin = {
  name: 'wasm-external',
  setup(build) {
    build.onResolve({ filter: /rustsim\.js$/ }, () => ({
      path: '../pkg/rustsim.js',
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

if (watch) {
  await appCtx.watch();
  console.log('Watching for changes...');
} else {
  await appCtx.rebuild();
  await appCtx.dispose();
}
