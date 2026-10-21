# Japanese replacement character generator

Upstream: [Shinonome font](https://github.com/code4fukui/shinonome-font), commit
`053b21e0a11ef5799c1ea3cefc374763add80617`.
The original `16/kanjic/font_src.bit` is retained, gzip-compressed with mtime 0,
as `shinonome16.bit.gz`. Its uncompressed SHA-256 is
`38ed485b3565b9c71147a459a9d0f8057714948ca6eaa2e6a3d74c95868d6467`.
The upstream LICENSE and AUTHORS are unmodified. The license explicitly permits
modification, conversion, embedding and redistribution, with a public-domain /
non-exercise-of-rights declaration. Do not infer MIT from the mirror's README.
The source acknowledges some XFree86 jiskanji16 symbol designs; preserve that
source notice along with the authors and license.

`python3 tools/build_font.py` converts the source's 6879 glyphs to
`jis0208.bin`: 94 rows × 94 cells × 32 bytes. Rows/cells run from JIS `21` to
`7E`. Unassigned cells are zero. For each glyph, each of sixteen raster lines
contains the left eight pixels followed by the right eight, MSB first. The
converted data SHA-256 is
`7ddc6bab059a22ae3e565a045b756dd78d6fe22a0e33c75130c316b62534cfbf`.
`--check` requires an identical conversion. No host text rasterizer or NEC ROM is
used. The binary is embedded into the same Rust core for native and Wasm builds.
