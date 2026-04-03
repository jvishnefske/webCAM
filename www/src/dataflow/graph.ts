/** Dataflow graph manager: wraps WASM API, manages tick loop. */

import {
  dataflow_new, dataflow_destroy, dataflow_add_block, dataflow_remove_block,
  dataflow_update_block, dataflow_connect, dataflow_disconnect, dataflow_advance,
  dataflow_run, dataflow_set_speed, dataflow_snapshot, dataflow_block_types,
} from '../../pkg/rustcam.js';
import type { GraphSnapshot, BlockTypeInfo, NodePosition } from './types.js';
import type { TelemetryPublisher } from './telemetry.js';
import type { SavedProject } from './storage.js';

export class DataflowManager {
  graphId: number;
  running = false;
  private rafId: number | null = null;
  private lastTime: number | null = null;

  /** UI positions for each block, keyed by block id. */
  positions = new Map<number, NodePosition>();

  /** Optional telemetry publisher for CRUD events. */
  telemetry: TelemetryPublisher | null = null;

  /** Callback invoked after each tick with the latest snapshot. */
  onTick: ((snap: GraphSnapshot) => void) | null = null;

  constructor(dt = 0.01) {
    this.graphId = dataflow_new(dt);
  }

  destroy(): void {
    this.stop();
    dataflow_destroy(this.graphId);
  }

  addBlock(blockType: string, config: Record<string, unknown>, x = 100, y = 100): number {
    const id = dataflow_add_block(this.graphId, blockType, JSON.stringify(config));
    this.positions.set(id, { x, y });
    this.telemetry?.publish({ tag: 50, blockId: id, blockType, config, x, y });
    return id;
  }

  updateBlock(blockId: number, blockType: string, config: Record<string, unknown>): void {
    dataflow_update_block(this.graphId, blockId, blockType, JSON.stringify(config));
    this.telemetry?.publish({ tag: 52, blockId, blockType, config });
  }

  removeBlock(blockId: number): void {
    dataflow_remove_block(this.graphId, blockId);
    this.positions.delete(blockId);
    this.telemetry?.publish({ tag: 51, blockId });
  }

  connect(fromBlock: number, fromPort: number, toBlock: number, toPort: number): number {
    const id = dataflow_connect(this.graphId, fromBlock, fromPort, toBlock, toPort);
    this.telemetry?.publish({ tag: 53, fromBlock, fromPort, toBlock, toPort, channelId: id });
    return id;
  }

  disconnect(channelId: number): void {
    dataflow_disconnect(this.graphId, channelId);
    this.telemetry?.publish({ tag: 54, channelId });
  }

  setSpeed(speed: number): void {
    dataflow_set_speed(this.graphId, speed);
  }

  snapshot(): GraphSnapshot {
    return JSON.parse(dataflow_snapshot(this.graphId));
  }

  /** Run N ticks instantly (non-realtime batch). */
  runBatch(steps: number, dt: number): GraphSnapshot {
    const json = dataflow_run(this.graphId, steps, dt);
    return JSON.parse(json);
  }

  /** Start the realtime tick loop. */
  start(): void {
    if (this.running) return;
    this.running = true;
    this.lastTime = null;
    this.tick();
  }

  /** Stop the realtime tick loop. */
  stop(): void {
    this.running = false;
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    this.lastTime = null;
  }

  private tick = (): void => {
    if (!this.running) return;
    const now = performance.now() / 1000;
    if (this.lastTime !== null) {
      const elapsed = Math.min(now - this.lastTime, 0.1); // cap at 100ms
      const json = dataflow_advance(this.graphId, elapsed);
      const snap: GraphSnapshot = JSON.parse(json);
      this.onTick?.(snap);
    }
    this.lastTime = now;
    this.rafId = requestAnimationFrame(this.tick);
  };

  /** Replay a saved project into this (empty) graph. Returns old→new block ID map. */
  restoreProject(project: SavedProject): Map<number, number> {
    const idMap = new Map<number, number>();
    for (const block of project.graph.blocks) {
      const newId = this.addBlock(block.blockType, block.config);
      idMap.set(block.id, newId);
      const pos = project.positions[block.id];
      if (pos) this.positions.set(newId, { x: pos.x, y: pos.y });
    }
    for (const ch of project.graph.channels) {
      const from = idMap.get(ch.fromBlock);
      const to = idMap.get(ch.toBlock);
      if (from !== undefined && to !== undefined) {
        this.connect(from, ch.fromPort, to, ch.toPort);
      }
    }
    return idMap;
  }

  static blockTypes(): BlockTypeInfo[] {
    return JSON.parse(dataflow_block_types());
  }
}
