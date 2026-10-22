#!/usr/bin/env python3
"""Create a source snapshot and checksums for files actually present, never placeholders."""
import argparse, hashlib, pathlib, subprocess, tarfile, re
parser=argparse.ArgumentParser();parser.add_argument('--source',action='store_true');args=parser.parse_args()
repo=pathlib.Path(__file__).resolve().parents[1]
version=re.search(r'^version = "([^"]+)"',(repo/'Cargo.toml').read_text(),re.M)[1]
output=repo/'dist/releases'/version;output.mkdir(parents=True,exist_ok=True)
revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()
(output/'SOURCE-REVISION.txt').write_text(revision+'\n')
if args.source:
    # Working source plus new packaging files, as selected by Git. No build products or clipboard databases.
    files=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=repo).decode().split('\0')
    with tarfile.open(output/f'cliprust-{version}-source.tar.gz','w:gz') as archive:
        for name in sorted(set(filter(None,files))):
            path=repo/name
            if path.is_file():archive.add(path,arcname=f'cliprust-{version}/'+name,recursive=False)
with (output/'SHA256SUMS').open('w') as manifest:
    for path in sorted(output.iterdir()):
        if path.is_file() and path.name!='SHA256SUMS':
            digest=hashlib.sha256()
            with path.open('rb') as source:
                for chunk in iter(lambda:source.read(1024*1024),b''):digest.update(chunk)
            manifest.write(digest.hexdigest()+'  '+path.name+'\n')
print(output)
