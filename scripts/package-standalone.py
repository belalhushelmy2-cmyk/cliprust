#!/usr/bin/env python3
"""Create a FUSE-free single-file launcher containing the complete runtime."""
import pathlib,sys,tarfile
root=pathlib.Path(sys.argv[1]);output=pathlib.Path(sys.argv[2])
header='#!/bin/sh\nset -eu\nself=$(readlink -f "$0")\nwork=$(mktemp -d "${TMPDIR:-/tmp}/cliprust-runtime.XXXXXXXX")\ntrap \'rm -rf "$work"\' EXIT\ntrap \'exit 130\' INT\ntrap \'exit 143\' TERM\nexport CLIPRUST_LAUNCHER="$self"\ntail -n +__LINE__ "$self" | tar -xz -C "$work"\n"$work/AppRun" "$@" <&0 &\nchild=$!\ntrap \'kill "$child" 2>/dev/null || :; wait "$child" 2>/dev/null || :; exit 143\' TERM INT\nwait "$child"\nexit $?\n'
header=header.replace('__LINE__',str(header.count('\n')+1))
with output.open('wb') as dest:
 dest.write(header.encode())
 with tarfile.open(fileobj=dest,mode='w:gz') as archive:
  for p in sorted(root.iterdir()): archive.add(p,arcname=p.name)
output.chmod(0o755)
