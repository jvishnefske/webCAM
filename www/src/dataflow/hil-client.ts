/**
 * HIL (Hardware-in-the-Loop) client: CBOR-over-WebSocket protocol
 * for I2C bus management, pin configuration, and graph deployment.
 *
 * CBOR maps use integer key 0 for the message tag.
 * Tags match hil-firmware-support/src/ws_dispatch.rs exactly.
 */

import { encode, decode } from 'cbor-x';

// ── Types ──────────────────────────────────────────────────────────

export interface BusEntry {
  busIdx: number;
  devices: DeviceEntry[];
}

export interface DeviceEntry {
  addr: number;
  name: string;
}

export interface PinEntry {
  name: string;
  direction: 'I' | 'O';
}

// ── CBOR tags (matching ws_dispatch.rs handle_request) ─────────────
// Requests (client → MCU)
const TAG_I2C_READ       = 1;   // {0:1, 1:bus, 2:addr, 3:reg, 4:len}
const TAG_I2C_WRITE      = 2;   // {0:2, 1:bus, 2:addr, 3:h'data'}
const TAG_LIST_BUSES     = 3;   // {0:3}
const TAG_ADD_DEVICE     = 30;  // {0:30, 1:bus, 2:addr, 3:"name", 4:h'registers'}
const TAG_REMOVE_DEVICE  = 31;  // {0:31, 1:bus, 2:addr}
const TAG_SET_REGISTERS  = 32;  // {0:32, 1:bus, 2:addr, 3:offset, 4:h'data'}
const TAG_SET_BUS_COUNT  = 33;  // {0:33, 1:count}
const TAG_CLEAR_ALL      = 34;  // {0:34}
const TAG_GET_CONFIG     = 35;  // {0:35}
const TAG_DEPLOY_GRAPH   = 40;  // {0:40, 1:json, 2:target, 3:dt} (new)
const TAG_GET_PIN_CONFIG = 43;  // {0:43} (new)

// Responses (MCU → client) — response tag mirrors request tag
// TAG_I2C_READ (1) and TAG_I2C_WRITE (2) responses use the same tags
const TAG_RESP_BUS_LIST  = 3;   // {0:3, 1:[{0:busIdx, 1:[{0:addr, 1:"name"}, ...]}, ...]}
const TAG_RESP_CONFIG    = 35;  // {0:35, 1:[...]}  same structure as bus list
const TAG_RESP_PIN_CFG   = 42;  // {0:42, 1:[...]}
const TAG_RESP_DEPLOY    = 41;  // {0:41}
const TAG_RESP_ERROR     = 255; // {0:255, 1:"message"}

// ── Client ─────────────────────────────────────────────────────────

export class HilClient {
  private ws: WebSocket | null = null;
  private url = '';
  private reconnectDelay = 500;
  private maxReconnectDelay = 30_000;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private shouldReconnect = false;

  // Callbacks
  onConnect: (() => void) | null = null;
  onDisconnect: (() => void) | null = null;
  onBusList: ((buses: BusEntry[]) => void) | null = null;
  onPinConfig: ((pins: PinEntry[]) => void) | null = null;
  onError: ((msg: string) => void) | null = null;
  onDeployAck: (() => void) | null = null;

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  connect(url: string): void {
    this.url = url;
    this.shouldReconnect = true;
    this.reconnectDelay = 500;
    this.openSocket();
  }

  disconnect(): void {
    this.shouldReconnect = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  // ── Request methods ────────────────────────────────────────────

  listBuses(): void {
    this.send({ 0: TAG_LIST_BUSES });
  }

  /** Add a simulated device. `registers` is initial register data (bytes). */
  addDevice(bus: number, addr: number, name: string, registers: Uint8Array | number): void {
    // If registers is a count, create a zero-filled buffer
    const regData = typeof registers === 'number'
      ? new Uint8Array(registers)
      : registers;
    this.send({ 0: TAG_ADD_DEVICE, 1: bus, 2: addr, 3: name, 4: regData });
  }

  removeDevice(bus: number, addr: number): void {
    this.send({ 0: TAG_REMOVE_DEVICE, 1: bus, 2: addr });
  }

  setRegisters(bus: number, addr: number, offset: number, data: Uint8Array): void {
    this.send({ 0: TAG_SET_REGISTERS, 1: bus, 2: addr, 3: offset, 4: data });
  }

  setBusCount(count: number): void {
    this.send({ 0: TAG_SET_BUS_COUNT, 1: count });
  }

  clearAll(): void {
    this.send({ 0: TAG_CLEAR_ALL });
  }

  getConfig(): void {
    this.send({ 0: TAG_GET_CONFIG });
  }

  getPinConfig(): void {
    this.send({ 0: TAG_GET_PIN_CONFIG });
  }

  /** Read I2C register(s) from a device on a bus. */
  i2cRead(bus: number, addr: number, reg: number, len: number): void {
    this.send({ 0: TAG_I2C_READ, 1: bus, 2: addr, 3: reg, 4: len });
  }

  /** Write raw bytes to an I2C device on a bus. */
  i2cWrite(bus: number, addr: number, data: Uint8Array): void {
    this.send({ 0: TAG_I2C_WRITE, 1: bus, 2: addr, 3: data });
  }

  deploy(snapshotJson: string, target: string, dt: number): void {
    this.send({ 0: TAG_DEPLOY_GRAPH, 1: snapshotJson, 2: target, 3: dt });
  }

  // ── Internals ──────────────────────────────────────────────────

  private openSocket(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    const ws = new WebSocket(this.url);
    ws.binaryType = 'arraybuffer';
    this.ws = ws;

    ws.addEventListener('open', () => {
      this.reconnectDelay = 500;
      this.onConnect?.();
      // Request initial state
      this.listBuses();
      this.getPinConfig();
    });

    ws.addEventListener('close', () => {
      this.ws = null;
      this.onDisconnect?.();
      this.scheduleReconnect();
    });

    ws.addEventListener('error', () => {
      // close event will follow
    });

    ws.addEventListener('message', (ev) => {
      this.handleMessage(ev);
    });
  }

  private scheduleReconnect(): void {
    if (!this.shouldReconnect) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.openSocket();
    }, this.reconnectDelay);
    this.reconnectDelay = Math.min(this.reconnectDelay * 2, this.maxReconnectDelay);
  }

  private send(msg: Record<number, unknown>): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(encode(msg));
  }

  private handleMessage(ev: MessageEvent): void {
    let msg: Record<number, unknown>;
    try {
      const data = ev.data instanceof ArrayBuffer ? new Uint8Array(ev.data) : ev.data;
      msg = decode(data) as Record<number, unknown>;
    } catch {
      this.onError?.('Failed to decode CBOR response');
      return;
    }

    const tag = msg[0] as number;
    switch (tag) {
      case TAG_RESP_BUS_LIST:
        this.onBusList?.(parseBusList(msg));
        break;
      case TAG_RESP_PIN_CFG:
        this.onPinConfig?.(parsePinConfig(msg));
        break;
      case TAG_RESP_CONFIG:
        // Config response has same bus-list structure
        this.onBusList?.(parseBusList(msg));
        break;
      case TAG_RESP_DEPLOY:
        this.onDeployAck?.();
        break;
      case TAG_RESP_ERROR:
        this.onError?.(String(msg[1] ?? 'Unknown error'));
        break;
      // Tag-only acks (30-34) — re-request bus list to refresh UI
      case TAG_ADD_DEVICE:
      case TAG_REMOVE_DEVICE:
      case TAG_SET_REGISTERS:
      case TAG_SET_BUS_COUNT:
      case TAG_CLEAR_ALL:
        this.listBuses();
        break;
    }
  }
}

// ── Response parsers ───────────────────────────────────────────────
// Wire format: {0:3, 1:[{0:busIdx, 1:[{0:addr, 1:"name"}, ...]}, ...]}
// All keys are integers, matching minicbor encoding in ws_dispatch.rs.

function parseBusList(msg: Record<number, unknown>): BusEntry[] {
  const raw = msg[1];
  if (!Array.isArray(raw)) return [];
  return raw.map((b: Record<number, unknown>, i: number) => ({
    busIdx: (b[0] as number) ?? i,
    devices: Array.isArray(b[1])
      ? (b[1] as Array<Record<number, unknown>>).map(d => ({
          addr: d[0] as number,
          name: String(d[1] ?? ''),
        }))
      : [],
  }));
}

function parsePinConfig(msg: Record<number, unknown>): PinEntry[] {
  const raw = msg[1];
  if (!Array.isArray(raw)) return [];
  return raw.map((p: Record<number, unknown>) => ({
    name: String(p[0] ?? ''),
    direction: p[1] === 'O' ? 'O' as const : 'I' as const,
  }));
}
