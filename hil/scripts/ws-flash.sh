#!/usr/bin/env bash
# ws-flash.sh — OTA firmware update via WebSocket CBOR protocol.
#
# Streams a .bin firmware image to the Pico over WebSocket using the
# FwBegin/FwChunk/FwFinish protocol (CBOR tags 20-22). Requires
# websocat and python3 with no additional packages.
#
# Usage:
#   ./scripts/ws-flash.sh firmware.bin [ws://host:port]
#
# Default WebSocket URL: ws://169.254.1.61:8080

set -euo pipefail

BIN_FILE="${1:?Usage: ws-flash.sh <firmware.bin> [ws://host:port]}"
WS_URL="${2:-ws://169.254.1.61:8080}"

if [ ! -f "$BIN_FILE" ]; then
    echo "Error: file not found: $BIN_FILE" >&2
    exit 1
fi

command -v websocat >/dev/null 2>&1 || { echo "Error: websocat not found. Install with: cargo install websocat" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "Error: python3 not found" >&2; exit 1; }

FILE_SIZE=$(stat -c%s "$BIN_FILE" 2>/dev/null || stat -f%z "$BIN_FILE")
echo "Firmware: $BIN_FILE ($FILE_SIZE bytes)"
echo "Target:   $WS_URL"

# Compute CRC32 and encode/decode CBOR using inline Python
# The Python script handles the full protocol flow
python3 -c "
import sys
import struct
import subprocess
import time

def crc32(data):
    '''IEEE 802.3 CRC32.'''
    import binascii
    return binascii.crc32(data) & 0xFFFFFFFF

def cbor_encode_uint(val):
    '''Encode a CBOR unsigned integer.'''
    if val <= 23:
        return bytes([val])
    elif val <= 0xFF:
        return bytes([0x18, val])
    elif val <= 0xFFFF:
        return bytes([0x19]) + struct.pack('>H', val)
    elif val <= 0xFFFFFFFF:
        return bytes([0x1A]) + struct.pack('>I', val)
    else:
        return bytes([0x1B]) + struct.pack('>Q', val)

def cbor_encode_map(n):
    '''Encode CBOR map header.'''
    if n <= 23:
        return bytes([0xA0 | n])
    return bytes([0xB8, n])

def cbor_encode_bytes(data):
    '''Encode CBOR byte string.'''
    hdr = len(data)
    if hdr <= 23:
        return bytes([0x40 | hdr]) + data
    elif hdr <= 0xFF:
        return bytes([0x58, hdr]) + data
    elif hdr <= 0xFFFF:
        return bytes([0x59]) + struct.pack('>H', hdr) + data
    else:
        return bytes([0x5A]) + struct.pack('>I', hdr) + data

def cbor_decode_map_tag(data):
    '''Decode CBOR map and return value of key 0 (the tag).'''
    pos = 0
    if data[pos] & 0xE0 == 0xA0:
        n = data[pos] & 0x1F
        pos += 1
    elif data[pos] == 0xB8:
        n = data[pos+1]
        pos += 2
    else:
        return None
    # key 0
    if data[pos] == 0:
        pos += 1
    else:
        return None
    # tag value
    if data[pos] <= 23:
        return data[pos]
    elif data[pos] == 0x18:
        return data[pos+1]
    elif data[pos] == 0x19:
        return struct.unpack('>H', data[pos+1:pos+3])[0]
    elif data[pos] == 0x1A:
        return struct.unpack('>I', data[pos+1:pos+5])[0]
    return None

def encode_fw_begin(total_size, crc):
    return cbor_encode_map(3) + cbor_encode_uint(0) + cbor_encode_uint(20) + \
           cbor_encode_uint(1) + cbor_encode_uint(total_size) + \
           cbor_encode_uint(2) + cbor_encode_uint(crc)

def encode_fw_chunk(offset, data):
    return cbor_encode_map(3) + cbor_encode_uint(0) + cbor_encode_uint(21) + \
           cbor_encode_uint(1) + cbor_encode_uint(offset) + \
           cbor_encode_uint(2) + cbor_encode_bytes(data)

def encode_fw_finish(crc):
    return cbor_encode_map(2) + cbor_encode_uint(0) + cbor_encode_uint(22) + \
           cbor_encode_uint(1) + cbor_encode_uint(crc)

# Read firmware
with open('$BIN_FILE', 'rb') as f:
    firmware = f.read()

file_crc = crc32(firmware)
total = len(firmware)
chunk_size = 1024

print(f'CRC32: 0x{file_crc:08X}')
print(f'Chunks: {(total + chunk_size - 1) // chunk_size}')

# Use websocat in binary mode
import socket
import websocket  # pip install websocket-client, or fall back to websocat

try:
    import websocket
    USE_WEBSOCKET_LIB = True
except ImportError:
    USE_WEBSOCKET_LIB = False

if not USE_WEBSOCKET_LIB:
    print('Error: python3 websocket-client not installed.', file=sys.stderr)
    print('Install with: pip3 install websocket-client', file=sys.stderr)
    sys.exit(1)

ws = websocket.create_connection('$WS_URL', timeout=30)
ws.settimeout(30)

try:
    # Step 1: FwBegin
    print('Sending FwBegin...')
    ws.send_binary(encode_fw_begin(total, file_crc))
    resp = ws.recv()
    if isinstance(resp, str):
        resp = resp.encode()
    tag = cbor_decode_map_tag(resp)
    if tag != 20:
        print(f'Error: expected FwReady (tag 20), got tag {tag}', file=sys.stderr)
        sys.exit(1)
    print('DFU partition erased, ready for chunks')

    # Step 2: FwChunk loop
    offset = 0
    while offset < total:
        end = min(offset + chunk_size, total)
        chunk = firmware[offset:end]
        ws.send_binary(encode_fw_chunk(offset, chunk))
        resp = ws.recv()
        if isinstance(resp, str):
            resp = resp.encode()
        tag = cbor_decode_map_tag(resp)
        if tag != 21:
            print(f'Error: expected FwChunkAck (tag 21), got tag {tag}', file=sys.stderr)
            sys.exit(1)
        offset = end
        pct = offset * 100 // total
        print(f'\rUploading: {offset}/{total} ({pct}%)', end='', flush=True)
    print()

    # Step 3: FwFinish
    print('Sending FwFinish...')
    ws.send_binary(encode_fw_finish(file_crc))
    resp = ws.recv()
    if isinstance(resp, str):
        resp = resp.encode()
    tag = cbor_decode_map_tag(resp)
    if tag != 22:
        print(f'Error: expected FwFinishAck (tag 22), got tag {tag}', file=sys.stderr)
        sys.exit(1)
    print('Firmware update complete! Device is rebooting...')
finally:
    ws.close()
" 2>&1
