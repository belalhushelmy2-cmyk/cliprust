#!/usr/bin/env python3
"""Fail packaging if a PE import is neither bundled nor a Windows 10 system DLL."""
import pathlib,struct,sys
SYSTEM=set('advapi32 bcrypt bcryptprimitives cfgmgr32 comctl32 comdlg32 combase crypt32 d3d11 d3d12 dcomp dbghelp dinput8 dnsapi dwrite dwmapi dxgi gdi32 imm32 iphlpapi kernel32 msvcrt ucrtbase ntdll ole32 oleaut32 opengl32 propsys psapi rpcrt4 secur32 setupapi shell32 shlwapi user32 userenv usp10 uxtheme version winhttp wininet winmm winspool wintrust ws2_32 wtsapi32 powrprof avrt shcore hid normaliz'.split())
def imports(path):
 b=path.read_bytes();u16=lambda o:struct.unpack_from('<H',b,o)[0];u32=lambda o:struct.unpack_from('<I',b,o)[0]
 pe=u32(0x3c)
 if b[pe:pe+4]!=b'PE\0\0':raise ValueError(f'Invalid PE: {path}')
 optional=pe+24;magic=u16(optional)
 if magic!=0x20b or u16(pe+4)!=0x8664:raise ValueError(f'Expected PE32+ x64: {path}')
 sections=optional+u16(pe+20)
 def offset(rva):
  for i in range(u16(pe+6)):
   s=sections+40*i;size=max(u32(s+8),u32(s+16));base=u32(s+12)
   if base<=rva<base+size:return u32(s+20)+rva-base
  raise ValueError(f'Invalid RVA in {path}')
 result=[]
 # Normal and delay-load imports both need to be deployable.
 for directory,stride,name_at in [(1,20,12),(13,32,4)]:
  rva=u32(optional+112+directory*8)
  if not rva:continue
  pos=offset(rva)
  while any(b[pos:pos+stride]):
   name=offset(u32(pos+name_at));end=b.index(0,name);result.append(b[name:end].decode('ascii').lower());pos+=stride
 return result
root=pathlib.Path(sys.argv[1]);files={p.name.lower():p for p in root.iterdir() if p.suffix.lower() in ('.exe','.dll')}
missing=[];report=[]
for name,path in sorted(files.items()):
 for dll in imports(path):
  bundled=dll in files
  system=dll.removesuffix('.dll') in SYSTEM or dll.startswith(('api-ms-win-','ext-ms-win-'))
  report.append(f'{name}: {dll} ({"bundled" if bundled else "Windows system" if system else "MISSING"})')
  if not bundled and not system:missing.append(dll)
text='\n'.join(report)+'\n'
if len(sys.argv)>2:pathlib.Path(sys.argv[2]).write_text(text)
else:print(text,end='')
if missing:raise SystemExit('Unbundled runtime DLLs: '+', '.join(sorted(set(missing))))
