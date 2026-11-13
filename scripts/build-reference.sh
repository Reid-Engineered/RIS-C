#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
mkdir -p reference
archive=reference/ngspice-47.tar.gz
if [ ! -f "$archive" ]; then
  curl -fL https://downloads.sourceforge.net/project/ngspice/ng-spice-rework/47/ngspice-47.tar.gz -o "$archive"
fi
python3 - "$archive" <<'PY'
import hashlib, sys
expected = '894e649651f1838a14095e5a5439e7d3aa63e87ede14d283173fda4fcdef675f'
with open(sys.argv[1], 'rb') as stream:
    actual = hashlib.file_digest(stream, 'sha256').hexdigest()
if actual != expected:
    raise SystemExit(f'ngspice archive checksum mismatch: {actual}')
PY
if [ ! -d reference/ngspice-47 ]; then
  tar -xzf "$archive" -C reference
fi
mkdir -p reference/build
cd reference/build
../ngspice-47/configure --without-x --enable-xspice --disable-cider --disable-openmp --with-readline=no
make -j"${SPICE_BUILD_JOBS:-4}"
printf '\nReference executable: %s/src/ngspice\n' "$PWD"
