/** Publishes dataflow graph CRUD events as CBOR over WebSocket. */

import { encode } from 'cbor-x';

export const TAG_BLOCK_ADDED       = 50;
export const TAG_BLOCK_REMOVED     = 51;
export const TAG_BLOCK_UPDATED     = 52;
export const TAG_CONNECTION_CREATED = 53;
export const TAG_CONNECTION_REMOVED = 54;
export const TAG_GRAPH_RESET       = 55;
export const TAG_DEBUG_WRAPPER     = 56;

export type TelemetryEvent =
  | { tag: 50; blockId: number; blockType: string; config: unknown; x: number; y: number }
  | { tag: 51; blockId: number }
  | { tag: 52; blockId: number; blockType: string; config: unknown }
  | { tag: 53; fromBlock: number; fromPort: number; toBlock: number; toPort: number; channelId: number }
  | { tag: 54; channelId: number }
  | { tag: 55 };

export class TelemetryPublisher {
  private ws: WebSocket | null = null;
  private enabled = false;
  private debug = false;
  private seq = 0;

  attach(ws: WebSocket): void {
    this.ws = ws;
  }

  detach(): void {
    this.ws = null;
  }

  setEnabled(on: boolean): void {
    this.enabled = on;
  }

  setDebug(on: boolean): void {
    this.debug = on;
    if (on) this.seq = 0;
  }

  publish(event: TelemetryEvent): void {
    if (!this.enabled || !this.ws || this.ws.readyState !== WebSocket.OPEN) return;

    const payload = this.encodeEvent(event);

    if (this.debug) {
      const wrapped: Record<number, unknown> = {
        0: TAG_DEBUG_WRAPPER,
        1: this.seq++,
        2: performance.now(),
        3: payload,
      };
      this.ws.send(encode(wrapped));
    } else {
      this.ws.send(encode(payload));
    }
  }

  private encodeEvent(event: TelemetryEvent): Record<number, unknown> {
    switch (event.tag) {
      case 50:
        return { 0: 50, 1: event.blockId, 2: event.blockType, 3: event.config, 4: event.x, 5: event.y };
      case 51:
        return { 0: 51, 1: event.blockId };
      case 52:
        return { 0: 52, 1: event.blockId, 2: event.blockType, 3: event.config };
      case 53:
        return { 0: 53, 1: event.fromBlock, 2: event.fromPort, 3: event.toBlock, 4: event.toPort, 5: event.channelId };
      case 54:
        return { 0: 54, 1: event.channelId };
      case 55:
        return { 0: 55 };
    }
  }
}
