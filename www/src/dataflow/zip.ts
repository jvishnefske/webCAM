/**
 * Minimal ZIP file creator (STORE method, no compression).
 * Pure TypeScript, no external dependencies.
 */

const encoder = new TextEncoder();

function crc32(data: Uint8Array): number {
  const table = new Uint32Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let j = 0; j < 8; j++) {
      c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
    }
    table[i] = c;
  }
  let crc = 0xFFFFFFFF;
  for (let i = 0; i < data.length; i++) {
    crc = table[(crc ^ data[i]) & 0xFF] ^ (crc >>> 8);
  }
  return (crc ^ 0xFFFFFFFF) >>> 0;
}

/** Create a ZIP file from an array of [path, content] pairs. Returns a Blob. */
export function createZip(files: Array<[string, string]>): Blob {
  const encoded = files.map(([path, content]) => ({
    nameBytes: encoder.encode(path),
    dataBytes: encoder.encode(content),
  }));

  // Calculate total size:
  // Each file: 30 + nameLen + dataLen (local header + data)
  // Each central dir entry: 46 + nameLen
  // End of central dir: 22
  let localSize = 0;
  let centralSize = 0;
  for (const { nameBytes, dataBytes } of encoded) {
    localSize += 30 + nameBytes.length + dataBytes.length;
    centralSize += 46 + nameBytes.length;
  }
  const totalSize = localSize + centralSize + 22;

  const buf = new ArrayBuffer(totalSize);
  const view = new DataView(buf);
  const bytes = new Uint8Array(buf);

  let offset = 0;
  const offsets: number[] = [];

  // Write local file headers + data
  for (const { nameBytes, dataBytes } of encoded) {
    offsets.push(offset);
    const crc = crc32(dataBytes);
    const size = dataBytes.length;
    const nameLen = nameBytes.length;

    view.setUint32(offset, 0x04034b50, true);       // signature
    view.setUint16(offset + 4, 20, true);            // version needed
    view.setUint16(offset + 6, 0, true);             // flags
    view.setUint16(offset + 8, 0, true);             // compression: STORE
    view.setUint16(offset + 10, 0, true);            // mod time
    view.setUint16(offset + 12, 0, true);            // mod date
    view.setUint32(offset + 14, crc, true);          // crc-32
    view.setUint32(offset + 18, size, true);         // compressed size
    view.setUint32(offset + 22, size, true);         // uncompressed size
    view.setUint16(offset + 26, nameLen, true);      // filename length
    view.setUint16(offset + 28, 0, true);            // extra field length
    offset += 30;

    bytes.set(nameBytes, offset);
    offset += nameLen;

    bytes.set(dataBytes, offset);
    offset += size;
  }

  // Write central directory
  const cdOffset = offset;
  for (let i = 0; i < encoded.length; i++) {
    const { nameBytes, dataBytes } = encoded[i];
    const crc = crc32(dataBytes);
    const size = dataBytes.length;
    const nameLen = nameBytes.length;

    view.setUint32(offset, 0x02014b50, true);        // signature
    view.setUint16(offset + 4, 20, true);             // version made by
    view.setUint16(offset + 6, 20, true);             // version needed
    view.setUint16(offset + 8, 0, true);              // flags
    view.setUint16(offset + 10, 0, true);             // compression: STORE
    view.setUint16(offset + 12, 0, true);             // mod time
    view.setUint16(offset + 14, 0, true);             // mod date
    view.setUint32(offset + 16, crc, true);           // crc-32
    view.setUint32(offset + 20, size, true);          // compressed size
    view.setUint32(offset + 24, size, true);          // uncompressed size
    view.setUint16(offset + 28, nameLen, true);       // filename length
    view.setUint16(offset + 30, 0, true);             // extra field length
    view.setUint16(offset + 32, 0, true);             // comment length
    view.setUint16(offset + 34, 0, true);             // disk number start
    view.setUint16(offset + 36, 0, true);             // internal file attributes
    view.setUint32(offset + 38, 0, true);             // external file attributes
    view.setUint32(offset + 42, offsets[i], true);    // local header offset
    offset += 46;

    bytes.set(nameBytes, offset);
    offset += nameLen;
  }

  // Write end of central directory
  const cdSize = offset - cdOffset;
  const count = encoded.length;

  view.setUint32(offset, 0x06054b50, true);           // signature
  view.setUint16(offset + 4, 0, true);                // disk number
  view.setUint16(offset + 6, 0, true);                // CD disk number
  view.setUint16(offset + 8, count, true);            // entries on this disk
  view.setUint16(offset + 10, count, true);           // total entries
  view.setUint32(offset + 12, cdSize, true);          // CD size
  view.setUint32(offset + 16, cdOffset, true);        // CD offset
  view.setUint16(offset + 20, 0, true);               // comment length

  return new Blob([buf], { type: 'application/zip' });
}
