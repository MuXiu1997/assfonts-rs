#!/bin/sh
# Independent renderer, never linked into the release binary.
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
work="$root/.validation/oracle"
mkdir -p "$work"
archive="$work/libass-0.17.5.tar.gz"
if [ ! -f "$archive" ]; then
    curl -fL --retry 3 https://github.com/libass/libass/releases/download/0.17.5/libass-0.17.5.tar.gz -o "$archive"
fi
python3 - "$archive" <<'PY'
import hashlib, pathlib, sys
expected = 'caab4b993dd7be6187c55623b789ed75dddefea6e65938af134637c732fe094a'
if hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest() != expected:
    sys.exit('libass source checksum mismatch')
PY
if [ ! -d "$work/libass-0.17.5" ]; then
    tar -xzf "$archive" -C "$work"
fi
cd "$work/libass-0.17.5"
./configure --disable-shared --enable-static --disable-fontconfig --disable-coretext \
    --disable-require-system-font-provider --disable-asm > "$work/configure.log" 2>&1
make -j4 > "$work/make.log" 2>&1
iconv=''
if [ "$(uname -s)" = Darwin ]; then iconv='-liconv'; fi
# pkg-config output intentionally splits into compiler arguments.
cc -std=c11 -Wall -Wextra -Werror -O2 "$root/tests/render/verify_libass.c" \
    -I"$work/libass-0.17.5/libass" "$work/libass-0.17.5/libass/.libs/libass.a" \
    $(pkg-config --cflags --libs freetype2 harfbuzz fribidi) $iconv -lm \
    -o "$work/verify_libass"
pkg-config --modversion freetype2 harfbuzz fribidi > "$work/dependency-versions.txt"
