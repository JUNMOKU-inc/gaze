#!/usr/bin/env bash
# Generate fixture images for E2E CLI tests
set -euo pipefail

FIXTURE_DIR="$(cd "$(dirname "$0")" && pwd)/fixtures"
mkdir -p "$FIXTURE_DIR"

# Generate PNGs using python3 (small + large using row duplication for speed)
python3 << 'PYEOF'
import struct, zlib, os

FIXTURE_DIR = os.environ.get("FIXTURE_DIR_OVERRIDE", os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures"))

def create_png(width, height, r, g, b):
    # Build one row, duplicate for all rows
    row = b'\x00' + bytes([r, g, b, 255]) * width
    raw = row * height
    compressed = zlib.compress(raw, 1)  # fast compression
    def chunk(ctype, data):
        c = ctype + data
        return struct.pack('>I', len(data)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    ihdr = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', ihdr) + chunk(b'IDAT', compressed) + chunk(b'IEND', b'')

with open(f'{FIXTURE_DIR}/e2e_input.png', 'wb') as f:
    f.write(create_png(100, 50, 255, 0, 0))

with open(f'{FIXTURE_DIR}/e2e_large_input.png', 'wb') as f:
    f.write(create_png(2000, 1500, 0, 128, 255))

print("PNGs generated")
PYEOF

# Generate JPEG from PNG using sips (macOS built-in, no Pillow needed)
sips -s format jpeg "$FIXTURE_DIR/e2e_input.png" --out "$FIXTURE_DIR/e2e_input.jpg" >/dev/null 2>&1 || true

# Generate invalid image file
printf '\x00\x01\x02\x03NOTANIMAGE' > "$FIXTURE_DIR/not_image.bin"

echo "Fixtures generated in $FIXTURE_DIR"
ls -la "$FIXTURE_DIR"
