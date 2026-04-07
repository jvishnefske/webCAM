/**
 * E2E test: verify block connections work in the browser.
 *
 * Starts native-server, opens Playwright, adds blocks via sidebar clicks,
 * drags to connect, captures [wire-trace] console output.
 */
import { test, expect } from '@playwright/test';

const BASE_URL = 'http://localhost:3000';

test.describe('Block Editor Connections', () => {
  test('add constant and gain blocks, verify ports exist', async ({ page }) => {
    // Capture all console messages
    const logs: string[] = [];
    page.on('console', msg => logs.push(msg.text()));

    await page.goto(BASE_URL);
    // Wait for WASM to load
    await page.waitForSelector('.df-workspace', { timeout: 10000 });
    await page.waitForTimeout(1000); // WASM init

    // Click "Constant" in sidebar palette to add a block
    await page.click('text=Constant');
    await page.waitForTimeout(300);

    // Click "Gain" in sidebar palette
    await page.click('text=Gain');
    await page.waitForTimeout(300);

    // Check blocks appeared in workspace
    const nodes = await page.locator('.df-node').count();
    expect(nodes).toBeGreaterThanOrEqual(2);

    // Check ports exist on the blocks
    const outputPorts = await page.locator('.df-port[data-side="output"]').count();
    const inputPorts = await page.locator('.df-port[data-side="input"]').count();

    console.log(`Output ports: ${outputPorts}, Input ports: ${inputPorts}`);
    expect(outputPorts).toBeGreaterThanOrEqual(2); // Constant has 1 out, Gain has 1 out
    expect(inputPorts).toBeGreaterThanOrEqual(1); // Gain has 1 input
  });

  test('drag from output to input creates connection', async ({ page }) => {
    const wireTraces: string[] = [];
    page.on('console', msg => {
      const text = msg.text();
      if (text.includes('wire-trace')) wireTraces.push(text);
    });

    await page.goto(BASE_URL);
    await page.waitForSelector('.df-workspace', { timeout: 10000 });
    await page.waitForTimeout(1000);

    // Add Constant block
    await page.click('text=Constant');
    await page.waitForTimeout(300);

    // Add Gain block
    await page.click('text=Gain');
    await page.waitForTimeout(300);

    // Get the output port of the first block and input port of the second
    const outputPort = page.locator('.df-port[data-side="output"]').first();
    const inputPort = page.locator('.df-port[data-side="input"]').first();

    const outBox = await outputPort.boundingBox();
    const inBox = await inputPort.boundingBox();

    if (outBox && inBox) {
      console.log(`Output port: ${JSON.stringify(outBox)}`);
      console.log(`Input port: ${JSON.stringify(inBox)}`);

      // Check what elementFromPoint returns at the input port center BEFORE drag
      const preDragHit = await page.evaluate(({x, y}) => {
        const el = document.elementFromPoint(x, y) as HTMLElement;
        return { tag: el?.tagName, cls: el?.className, side: el?.dataset?.side };
      }, { x: inBox.x + inBox.width / 2, y: inBox.y + inBox.height / 2 });
      console.log('Pre-drag hit at input port:', JSON.stringify(preDragHit));

      // Drag from output to input center
      const ox = outBox.x + outBox.width / 2;
      const oy = outBox.y + outBox.height / 2;
      const ix = inBox.x + inBox.width / 2;
      const iy = inBox.y + inBox.height / 2;

      await page.mouse.move(ox, oy);
      await page.mouse.down();
      // Move in steps to simulate real drag
      for (let t = 0; t <= 1; t += 0.1) {
        await page.mouse.move(ox + (ix - ox) * t, oy + (iy - oy) * t);
      }
      // Final move to exact target center
      await page.mouse.move(ix, iy);
      await page.waitForTimeout(50);

      // Check what's under cursor RIGHT before release
      const preReleaseHit = await page.evaluate(({x, y}) => {
        const el = document.elementFromPoint(x, y) as HTMLElement;
        return { tag: el?.tagName, cls: el?.className, side: el?.dataset?.side, idx: el?.dataset?.index };
      }, { x: ix, y: iy });
      console.log('Pre-release hit at target:', JSON.stringify(preReleaseHit));

      await page.mouse.up();
      await page.waitForTimeout(500);
    }

    // Print all wire traces for debugging
    console.log('Wire traces captured:', wireTraces.length);
    for (const trace of wireTraces) {
      console.log('TRACE:', trace);
    }
    // Also dump all console messages
    const allLogs = await page.evaluate(() => {
      // Check what elementFromPoint returns at various positions
      const ports = document.querySelectorAll('.df-port');
      const portInfo: any[] = [];
      ports.forEach(p => {
        const rect = (p as HTMLElement).getBoundingClientRect();
        const el = document.elementFromPoint(rect.x + rect.width/2, rect.y + rect.height/2);
        portInfo.push({
          side: (p as HTMLElement).dataset.side,
          index: (p as HTMLElement).dataset.index,
          rect: { x: rect.x, y: rect.y, w: rect.width, h: rect.height },
          elementFromPointClass: el?.className,
          elementFromPointTag: el?.tagName,
          isSame: el === p,
          zIndex: getComputedStyle(p as HTMLElement).zIndex,
          pointerEvents: getComputedStyle(p as HTMLElement).pointerEvents,
        });
      });
      // Also check what's at the first input port center
      const firstInput = document.querySelector('.df-port[data-side="input"]') as HTMLElement;
      if (firstInput) {
        const r = firstInput.getBoundingClientRect();
        const cx = r.x + r.width/2;
        const cy = r.y + r.height/2;
        const hitEl = document.elementFromPoint(cx, cy) as HTMLElement;
        portInfo.push({
          _label: 'FIRST_INPUT_HIT_TEST',
          hitClass: hitEl?.className,
          hitTag: hitEl?.tagName,
          hitText: hitEl?.textContent?.slice(0, 50),
          hitParent: hitEl?.parentElement?.className,
          portRect: { x: r.x, y: r.y, w: r.width, h: r.height },
        });
      }
      return portInfo;
    });
    console.log('Port hit-test analysis:', JSON.stringify(allLogs, null, 2));

    // Check if any edges were created
    const edges = await page.locator('.df-edge:not(.dragging)').count();
    console.log(`Edges after connect attempt: ${edges}`);

    // If there were wire traces, check the result
    if (wireTraces.length > 0) {
      const lastTrace = wireTraces[wireTraces.length - 1];
      console.log('Last trace:', lastTrace);
      // The trace should show either success or the specific error
    }
  });

  test('snapshot shows correct port counts per block type', async ({ page }) => {
    // This test directly checks the WASM API via page.evaluate
    await page.goto(BASE_URL);
    await page.waitForSelector('.df-workspace', { timeout: 10000 });
    await page.waitForTimeout(1500);

    // Use the WASM API directly to add blocks and inspect
    const result = await page.evaluate(async () => {
      // Access the WASM functions
      const w = (window as any);
      // The rustsim module should be loaded
      const mod = await import('/pkg/rustsim.js');

      const graphId = mod.dataflow_new(0.01);

      // Add blocks with known configs
      const constId = mod.dataflow_add_block(graphId, 'constant', '{"value":42.0}');
      const gainId = mod.dataflow_add_block(graphId, 'gain', '{"op":"Gain","param1":2.0,"param2":0.0}');
      const addId = mod.dataflow_add_block(graphId, 'add', '{}');
      const adcId = mod.dataflow_add_block(graphId, 'adc_source', '{"channel":0,"resolution_bits":12}');

      // Get snapshot
      const snap = JSON.parse(mod.dataflow_snapshot(graphId));

      // Try connecting constant output 0 → gain input 0
      let connectResult: string;
      try {
        mod.dataflow_connect(graphId, constId, 0, gainId, 0);
        connectResult = 'success';
      } catch (e) {
        connectResult = 'error: ' + String(e);
      }

      mod.dataflow_destroy(graphId);

      return {
        blocks: snap.blocks.map((b: any) => ({
          id: b.id,
          type: b.block_type,
          inputs: b.inputs.length,
          outputs: b.outputs.length,
          inputNames: b.inputs.map((p: any) => p.name),
          outputNames: b.outputs.map((p: any) => p.name),
        })),
        connectResult,
      };
    });

    console.log('Block port counts:', JSON.stringify(result.blocks, null, 2));
    console.log('Connect result:', result.connectResult);

    // Verify port counts
    const constBlock = result.blocks.find((b: any) => b.type === 'constant');
    const gainBlock = result.blocks.find((b: any) => b.type === 'gain');
    const addBlock = result.blocks.find((b: any) => b.type === 'add');
    const adcBlock = result.blocks.find((b: any) => b.type === 'adc_source');

    expect(constBlock.inputs).toBe(0);
    expect(constBlock.outputs).toBe(1);
    expect(gainBlock.inputs).toBe(1);
    expect(gainBlock.outputs).toBe(1);
    expect(addBlock.inputs).toBe(2);
    expect(addBlock.outputs).toBe(1);
    expect(adcBlock.inputs).toBe(0);
    expect(adcBlock.outputs).toBe(1);

    // Connection should succeed
    expect(result.connectResult).toBe('success');
  });
});
