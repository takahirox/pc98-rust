#!/usr/bin/env python3
"""Build the original MIT raw-port DMA/FDC boot probe, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/dma_autoinit.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = bytearray(1_261_568)
    image[:1024] = boot
    image[1024:2048] = bytes([0x39]) * 1024
    image[2048:3072] = bytes([0xc7]) * 1024
    # Distinct untouched media guards, including around the two write targets.
    image[3072:] = bytes((i * 17 + i // 1024 * 23) & 255 for i in range(3072, len(image)))
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE copied alongside the generated image',
        'assembly': 'nasm -f bin guest/dma_autoinit.asm -o autoinit.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/dma/autoinit.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'contract': 'Two auto-init READ DATA and two WRITE DATA commands, plus master-clear retained read/write with masked no-transfer commands before unmasking, plus controller-disabled no-transfer read/write and fresh commands after enabling; register readback/reload, buffer patterns, guards, result bytes and IRQ; bounded waits',
        'markers': ['DMA AUTO INIT PASS', 'DMA AUTO INIT FAIL', 'DMA MASTER CLEAR PASS', 'DMA MASTER CLEAR FAIL', 'DMA DISABLE PASS', 'DMA DISABLE FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; no proprietary software or firmware; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/dma-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('DMA fixture differs from fixtures/dma-manifest.json')
    destination = ROOT / 'fixtures/dma'
    destination.mkdir(exist_ok=True)
    (destination / 'autoinit.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'dma_autoinit.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('DMA fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
