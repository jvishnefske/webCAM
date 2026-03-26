import * as esbuild from 'esbuild';

const args = process.argv.slice(2);
const watch = args.includes('--watch');

const ctx = await esbuild.context({
  entryPoints: ['dag/dag-editor.ts'],
  outfile: 'dag/dag-editor.js',
  bundle: true,
  format: 'esm',
  target: 'es2022',
  minify: true,
  sourcemap: false,
  external: ['../pkg/rustcam.js'],
});

if (watch) {
  await ctx.watch();
  console.log('Watching...');
} else {
  await ctx.rebuild();
  console.log('Built dag/dag-editor.js');
  await ctx.dispose();
}
