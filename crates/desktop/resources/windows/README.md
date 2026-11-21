# Windows resources

`massorca.rc` is compiled into `massorca-desktop.exe` by `crates/desktop/build.rs`
when the target OS is Windows. GPUI loads icon resource id 1 as the window and
taskbar icon.

`massorca.ico` holds the rose at 16, 20, 24, 32, 40, 48, 64 and 256 px, rendered
from `crates/ui/assets/brand/massorca-logo.svg` with rsvg-convert 2.58.0 and
ImageMagick 6.9.12-98 (Ubuntu 24.04). From the repository root:

```sh
work=$(mktemp -d)
for s in 16 20 24 32 40 48 64 256; do
  rsvg-convert -w "$s" -h "$s" crates/ui/assets/brand/massorca-logo.svg -o "$work/$s.png"
done
convert $(for s in 16 20 24 32 40 48 64 256; do echo "$work/$s.png"; done) \
  -strip crates/desktop/resources/windows/massorca.ico
rm -rf "$work"
```

The output is byte-for-byte reproducible with those tool versions.
