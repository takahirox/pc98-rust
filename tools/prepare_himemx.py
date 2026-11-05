#!/usr/bin/env python3
"""Pinned, unmodified PC-98 HimemX guest and accompanying Standard Version.

Artistic License clauses 1 and 4(a)/(b): preserve notices and publish the complete
release plus matching tagged source with the desktop. Never extract archive paths.
"""
import hashlib
import io
import tarfile
import urllib.request
import zipfile

from prepare_freedos import DEST

COMMIT = '5dad3f18835c0d028bb7c9e967c548aaf152b15e'
ASSETS = {
    'himemx-release.zip': (
        'https://github.com/lpproj/himemx.nec/releases/download/v3.34-nec98-20160913/himemx334_nec98_20160913.zip',
        '4a8d1321ff91ac43d8996f6940953cdddb5ac3be90e5ad3912aee43c338390f7'),
    'himemx-source.tar.gz': (
        f'https://codeload.github.com/lpproj/himemx.nec/tar.gz/{COMMIT}',
        '151e6cb71e2a177c80cb722385886c5f9e1a65bc1bd2608765c541f587969683'),
}
LIMIT = 1024 * 1024


def fetch(name, url, checksum):
    path = DEST / name
    if path.exists():
        with path.open('rb') as stream:
            data = stream.read(LIMIT + 1)
    else:
        with urllib.request.urlopen(url, timeout=60) as stream:
            data = stream.read(LIMIT + 1)
    if len(data) > LIMIT or hashlib.sha256(data).hexdigest() != checksum:
        raise ValueError(f'HimemX checksum or size mismatch: {name}')
    path.write_bytes(data)
    return data


def prepare():
    DEST.mkdir(parents=True, exist_ok=True)
    blobs = {n: fetch(n, *pin) for n, pin in ASSETS.items()}
    with zipfile.ZipFile(io.BytesIO(blobs['himemx-release.zip'])) as archive:
        assert archive.namelist() == ['nec98/himemx.exe', 'HimemX98.txt', 'README.txt', 'LICENSE.txt']
        assert all(m.file_size < LIMIT for m in archive.infolist())
        binary = archive.read('nec98/himemx.exe')
        assert len(binary) == 5859
        with tarfile.open(fileobj=io.BytesIO(blobs['himemx-source.tar.gz'])) as source:
            # Tag source includes the same three exact notices as the release.
            for name in ['HimemX98.txt', 'README.txt', 'LICENSE.txt']:
                data = archive.read(name)
                member = source.getmember(f'himemx.nec-{COMMIT}/{name}')
                assert member.isfile() and member.size < LIMIT
                assert source.extractfile(member).read() == data
                (DEST / ('HimemX-' + name)).write_bytes(data)
            assert source.getmember(f'himemx.nec-{COMMIT}/source/himemx.asm').isfile()
            assert source.getmember(f'himemx.nec-{COMMIT}/Makefile.wc').isfile()
    return binary
