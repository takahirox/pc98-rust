#!/usr/bin/env python3
"""Assemble the original MIT input and media regression disk outside web/."""
from build_media import ROOT, assemble


if __name__ == '__main__':
    boot = assemble('guest/input_probe.asm')
    assert len(boot) == 1024
    disk = ROOT / 'fixtures/input_probe/input.hdm'
    disk.parent.mkdir(exist_ok=True)
    disk.write_bytes(boot.ljust(1261568, b'\0'))
