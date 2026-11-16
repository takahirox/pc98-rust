# Pinned ADV98 compatibility executable

`ADV98.EXE` is the unmodified, independently authored MIT guest already used by
the DOS compatibility suite. It is 75,776 bytes with SHA-256
`904f476516bf009b6d25d0a2738ddf649af05bcafac72cbb703c73d7159e5954`.
The accompanying [LICENSE](LICENSE) is copied unchanged from the pinned
[engine source](https://github.com/yarufu/pc98/tree/11a9f1c59c6fd8169439d477d5276ee7c0f27566).
The fixture preparation script verifies that license against the checksum-pinned
source archive, and verifies the executable before constructing disks.

The executable was extracted from the previously verified
[v0.1 release archive](https://github.com/yarufu/pc98/releases/tag/v0.1),
86,752 bytes with SHA-256
`9aa590f53b331ec04fa1a2352f7ed6190407756d1f552c9c0bf76f9a05eb2d3d`.
The release URL is mutable: upstream replaced its archive and executable on
2026-11-15. Keeping the existing executable here preserves the exact guest and
disk hashes without accepting different release bytes or weakening verification.
The original release artwork, music and optional TSRs are excluded.

`tools/prepare_dos_apps.py` combines this executable with a pinned MIT scenario
and original replacement art. Generated disks retain their existing complete
hashes in [the audited manifest](../dos-apps-manifest.json). Preserve this
directory, the fetched source archives and the generated FreeDOS corresponding
sources when redistributing the fixtures. See [DOS compatibility](../../docs/dos-compatibility.md).
