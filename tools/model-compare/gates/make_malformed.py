# SPDX-License-Identifier: MIT
# Crafted inputs for the #11 gate. Each must be rejected by both upstream and Rust (or accepted by both with equal output).
import io, os, struct, sys, zipfile, zlib
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
src = sys.argv[2]  # a real corpus file to mutate
data = open(src, 'rb').read()
open(f'{out}/truncated-half.xlsx', 'wb').write(data[: len(data) // 2])
open(f'{out}/truncated-tail22.xlsx', 'wb').write(data[:-22])
open(f'{out}/garbage.xlsx', 'wb').write(bytes((i * 131 + 7) % 251 for i in range(4096)))
open(f'{out}/empty.xlsx', 'wb').write(b'')
# Zip bomb: one honest 320 MiB entry of zeros (over the 300 MiB total and 200 MiB part limits).
b = io.BytesIO()
with zipfile.ZipFile(b, 'w', zipfile.ZIP_DEFLATED) as z:
    with z.open('xl/bomb.xml', 'w', force_zip64=True) as f:
        chunk = b'\0' * (1 << 20)
        for _ in range(320): f.write(chunk)
open(f'{out}/bomb-honest.xlsx', 'wb').write(b.getvalue())
# Lying bomb: same payload, but local and central headers claim 1 KiB uncompressed.
payload = zlib.compressobj(9, zlib.DEFLATED, -15)
comp = b''.join(payload.compress(b'\0' * (1 << 20)) for _ in range(320)) + payload.flush()
name = b'xl/bomb.xml'; crc = 0
for _ in range(320): crc = zlib.crc32(b'\0' * (1 << 20), crc)
lie = 1024
lh = struct.pack('<IHHHHHIIIHH', 0x04034b50, 20, 0, 8, 0, 0, crc, len(comp), lie, len(name), 0) + name
cd = struct.pack('<IHHHHHHIIIHHHHHII', 0x02014b50, 20, 20, 0, 8, 0, 0, crc, len(comp), lie, len(name), 0, 0, 0, 0, 0, 0) + name
body = lh + comp
eocd = struct.pack('<IHHHHIIH', 0x06054b50, 0, 0, 1, 1, len(cd), len(body), 0)
open(f'{out}/bomb-lying-size.xlsx', 'wb').write(body + cd + eocd)
# Leading-slash and mixed-case names, plus a directory entry: must be accepted, names normalised like upstream.
b = io.BytesIO()
with zipfile.ZipFile(b, 'w', zipfile.ZIP_DEFLATED) as z:
    z.writestr('/[Content_Types].xml', '<Types/>')
    z.writestr('XL/', '')
    z.writestr('XL/Workbook.xml', '<workbook/>')
    z.writestr('docProps/app.xml', 'x' * 100000)
open(f'{out}/names-slash-case-dir.xlsx', 'wb').write(b.getvalue())
