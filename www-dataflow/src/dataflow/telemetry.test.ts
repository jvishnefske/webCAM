import { describe, it, expect, beforeEach } from 'vitest';
import { decode } from 'cbor-x';
import { TelemetryPublisher } from './telemetry';

describe('TelemetryPublisher', () => {
  let publisher: TelemetryPublisher;
  let sentMessages: Uint8Array[];
  let mockWs: { send: (data: Uint8Array) => void; readyState: number };

  beforeEach(() => {
    sentMessages = [];
    mockWs = {
      send: (data: Uint8Array) => sentMessages.push(data),
      readyState: 1,
    };
    publisher = new TelemetryPublisher();
    publisher.attach(mockWs as unknown as WebSocket);
    publisher.setEnabled(true);
  });

  it('encodes block-added as tag 50', () => {
    publisher.publish({ tag: 50, blockId: 7, blockType: 'constant', config: { value: 1.0 }, x: 100, y: 200 });
    expect(sentMessages).toHaveLength(1);
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(50);
    expect(msg[1]).toBe(7);
    expect(msg[2]).toBe('constant');
    expect(msg[4]).toBe(100);
    expect(msg[5]).toBe(200);
  });

  it('encodes block-removed as tag 51', () => {
    publisher.publish({ tag: 51, blockId: 3 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(51);
    expect(msg[1]).toBe(3);
  });

  it('encodes block-updated as tag 52', () => {
    publisher.publish({ tag: 52, blockId: 5, blockType: 'gain', config: { param1: 2.0 } });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(52);
    expect(msg[1]).toBe(5);
    expect(msg[2]).toBe('gain');
  });

  it('encodes connection-created as tag 53', () => {
    publisher.publish({ tag: 53, fromBlock: 1, fromPort: 0, toBlock: 2, toPort: 0, channelId: 10 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(53);
    expect(msg[1]).toBe(1);
    expect(msg[5]).toBe(10);
  });

  it('encodes connection-removed as tag 54', () => {
    publisher.publish({ tag: 54, channelId: 10 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(54);
    expect(msg[1]).toBe(10);
  });

  it('encodes graph-reset as tag 55', () => {
    publisher.publish({ tag: 55 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(55);
  });

  it('does not send when disabled', () => {
    publisher.setEnabled(false);
    publisher.publish({ tag: 50, blockId: 1, blockType: 'constant', config: {}, x: 0, y: 0 });
    expect(sentMessages).toHaveLength(0);
  });

  it('does not send when detached', () => {
    publisher.detach();
    publisher.publish({ tag: 51, blockId: 1 });
    expect(sentMessages).toHaveLength(0);
  });

  it('wraps in debug envelope (tag 56) when debug enabled', () => {
    publisher.setDebug(true);
    publisher.publish({ tag: 51, blockId: 9 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(56);
    expect(typeof msg[1]).toBe('number');
    expect(typeof msg[2]).toBe('number');
    const inner = msg[3] as Record<number, unknown>;
    expect(inner[0]).toBe(51);
    expect(inner[1]).toBe(9);
  });

  it('increments sequence number in debug mode', () => {
    publisher.setDebug(true);
    publisher.publish({ tag: 55 });
    publisher.publish({ tag: 55 });
    const msg1 = decode(sentMessages[0]) as Record<number, unknown>;
    const msg2 = decode(sentMessages[1]) as Record<number, unknown>;
    expect((msg2[1] as number) - (msg1[1] as number)).toBe(1);
  });
});
