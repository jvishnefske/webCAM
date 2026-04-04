/**
 * End-to-end test: HilClient DAG/PubSub HTTP flow.
 *
 * Simulates the full deploy → tick → read pubsub cycle with mocked fetch
 * matching the Pico2 dag_handler.rs HTTP API responses.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { HilClient } from './hil-client';

// Encode a CBOR text string
function cborStr(s: string): number[] {
  const bytes = new TextEncoder().encode(s);
  const out: number[] = [];
  if (bytes.length < 24) {
    out.push(0x60 | bytes.length);
  } else {
    out.push(0x78, bytes.length);
  }
  out.push(...bytes);
  return out;
}

// Build a CBOR-encoded DAG: [Const(42.0), Publish("test_topic", 0)]
function buildTestDag(): Uint8Array {
  const parts: number[] = [];
  // Array of 2 ops
  parts.push(0x82); // array(2)

  // Op 0: Const(42.0) → [0, 42.0]
  parts.push(0x82); // array(2)
  parts.push(0x00); // tag 0 = Const
  parts.push(0xfb); // f64
  const buf = new ArrayBuffer(8);
  new DataView(buf).setFloat64(0, 42.0, false);
  parts.push(...new Uint8Array(buf));

  // Op 1: Publish("test_topic", 0) → [11, "test_topic", 0]
  parts.push(0x83); // array(3)
  parts.push(0x0b); // tag 11 = Publish
  parts.push(...cborStr('test_topic'));
  parts.push(0x00); // src node 0

  return new Uint8Array(parts);
}

// Build a DAG: [Const(10), Publish("alpha", 0), Subscribe("alpha"), Publish("beta", 2)]
// This tests the round-trip: publish to alpha, subscribe reads it, publish to beta
function buildRoundTripDag(): Uint8Array {
  const parts: number[] = [];
  parts.push(0x84); // array(4)

  // Op 0: Const(10.0)
  parts.push(0x82, 0x00, 0xfb);
  const buf = new ArrayBuffer(8);
  new DataView(buf).setFloat64(0, 10.0, false);
  parts.push(...new Uint8Array(buf));

  // Op 1: Publish("alpha", 0)
  parts.push(0x83, 0x0b);
  parts.push(...cborStr('alpha'));
  parts.push(0x00);

  // Op 2: Subscribe("alpha")
  parts.push(0x82, 0x0a);
  parts.push(...cborStr('alpha'));

  // Op 3: Publish("beta", 2)
  parts.push(0x83, 0x0b);
  parts.push(...cborStr('beta'));
  parts.push(0x02);

  return new Uint8Array(parts);
}

describe('HilClient DAG/PubSub E2E', () => {
  let client: HilClient;
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    client = new HilClient();
    // Set URL directly so httpBase works (no actual WS connection)
    (client as any).url = 'ws://169.254.1.61:8080/ws';
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('httpBase derives from ws URL', () => {
    expect(client.httpBase).toBe('http://169.254.1.61:8080');
  });

  it('deployDag sends CBOR POST and parses response', async () => {
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true, nodes: 2 }),
    });

    const dag = buildTestDag();
    const result = await client.deployDag(dag);

    expect(fetchMock).toHaveBeenCalledWith(
      'http://169.254.1.61:8080/api/dag',
      expect.objectContaining({ method: 'POST', body: dag }),
    );
    expect(result).toEqual({ ok: true, nodes: 2 });
  });

  it('tick sends POST and returns ok', async () => {
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true }),
    });

    const result = await client.tick();
    expect(fetchMock).toHaveBeenCalledWith(
      'http://169.254.1.61:8080/api/tick',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(result).toEqual({ ok: true });
  });

  it('getPubsub returns topic snapshot', async () => {
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ test_topic: 42, other: 7.5 }),
    });

    const topics = await client.getPubsub();
    expect(topics.test_topic).toBe(42);
    expect(topics.other).toBe(7.5);
  });

  it('getStatus returns loaded/nodes/ticks', async () => {
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ loaded: true, nodes: 5, ticks: 10 }),
    });

    const status = await client.getStatus();
    expect(status.loaded).toBe(true);
    expect(status.nodes).toBe(5);
    expect(status.ticks).toBe(10);
  });

  it('getChannels returns inputs and outputs', async () => {
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ inputs: ['adc0', 'adc1'], outputs: ['pwm0'] }),
    });

    const ch = await client.getChannels();
    expect(ch.inputs).toEqual(['adc0', 'adc1']);
    expect(ch.outputs).toEqual(['pwm0']);
  });

  it('toggleDebug returns new debug state', async () => {
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ debug: true }),
    });

    const result = await client.toggleDebug();
    expect(result.debug).toBe(true);
  });

  it('full pubsub E2E: deploy → tick → read topics', async () => {
    // 1. Deploy
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true, nodes: 2 }),
    });
    const deployResult = await client.deployDag(buildTestDag());
    expect(deployResult.ok).toBe(true);
    expect(deployResult.nodes).toBe(2);

    // 2. Tick
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true }),
    });
    const tickResult = await client.tick();
    expect(tickResult.ok).toBe(true);

    // 3. Read pubsub — test_topic should have value 42
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ test_topic: 42 }),
    });
    const topics = await client.getPubsub();
    expect(topics.test_topic).toBe(42);
  });

  it('pubsub round-trip: publish → subscribe → republish', async () => {
    // Deploy round-trip DAG: Const(10) → Publish("alpha") → Subscribe("alpha") → Publish("beta")
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true, nodes: 4 }),
    });
    await client.deployDag(buildRoundTripDag());

    // Tick 1: Const publishes to alpha, Subscribe reads alpha (0 on first tick), publishes to beta
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true }),
    });
    await client.tick();

    // After tick 1: alpha=10, beta=0 (subscribe read before publish on same tick)
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ alpha: 10, beta: 0 }),
    });
    let topics = await client.getPubsub();
    expect(topics.alpha).toBe(10);
    expect(topics.beta).toBe(0);

    // Tick 2: Subscribe now reads alpha=10, publishes to beta=10
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true }),
    });
    await client.tick();

    fetchMock.mockResolvedValueOnce({
      json: async () => ({ alpha: 10, beta: 10 }),
    });
    topics = await client.getPubsub();
    expect(topics.alpha).toBe(10);
    expect(topics.beta).toBe(10);
  });

  it('tick without deployed DAG returns error', async () => {
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ error: 'no DAG loaded' }),
    });

    const result = await client.tick();
    expect(result.error).toBe('no DAG loaded');
  });

  it('deploy with debug mode shows _dbg topics', async () => {
    // Deploy
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true, nodes: 2 }),
    });
    await client.deployDag(buildTestDag());

    // Enable debug
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ debug: true }),
    });
    await client.toggleDebug();

    // Tick
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ ok: true }),
    });
    await client.tick();

    // Read pubsub — should include _dbg/0 and _dbg/1
    fetchMock.mockResolvedValueOnce({
      json: async () => ({ test_topic: 42, '_dbg/0': 42, '_dbg/1': 42 }),
    });
    const topics = await client.getPubsub();
    expect(topics['_dbg/0']).toBe(42);
    expect(topics['_dbg/1']).toBe(42);
    expect(topics.test_topic).toBe(42);
  });
});
