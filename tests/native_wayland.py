# Explicit native acceptance test. Temporarily replaces and restores the clipboard.
# Requires wl-paste and a built debug Cliprust binary; uses a private temporary database.
import subprocess, json, os, tempfile, time, sqlite3, struct, zlib, shutil, pathlib
os.umask(0o077)
bin=str(pathlib.Path(__file__).resolve().parents[1] / 'target/debug/cliprust')
listed=subprocess.run(['wl-paste','--list-types'],capture_output=True)
types=listed.stdout.decode().splitlines() if listed.returncode==0 else []
original={m:list(subprocess.run(['wl-paste','--no-newline','--type',m],check=True,capture_output=True,timeout=4).stdout) for m in types}
assert len(original)<=32 and sum(map(len,original.values()))<=32*1024*1024
work=tempfile.mkdtemp(prefix='cliprust-native-')
db=work+'/history.db'
monitor=subprocess.Popen([bin,'--database',db,'monitor','--seconds','9'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
helpers=[]
def publish(formats):
    child=subprocess.Popen([bin,'serve-bundle'],start_new_session=True,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    child.stdin.write(json.dumps({'formats':{k:list(v) for k,v in formats.items()}}).encode());child.stdin.close()
    ready=child.stdout.readline().strip()
    assert ready==b'ready', 'Clipboard owner did not initialize: '+child.stderr.read().decode()
    helpers.append(child)
    return child

def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d)&0xffffffff)
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,2,8,6,0,0,0))+chunk(b'IDAT',zlib.compress((b'\0'+b'\xb5\x9a\xe5\xff'*2)*2))+chunk(b'IEND',b'')
fixtures=[{'text/plain':'cliprust native roundtrip مرحبا 🦀'.encode(),'text/html':b'<b>Cliprust native rich text</b>','text/rtf':b'{\\rtf1 Cliprust native rich text}'}, {'image/png':png}, {'text/uri-list':b'file:///tmp/cliprust-a.txt\r\nfile:///tmp/cliprust-b.txt\r\n'}]
try:
    time.sleep(.5)
    for formats in fixtures:
        publish(formats);time.sleep(.5)
        conn=sqlite3.connect(db)
        match=conn.execute('SELECT e.id FROM entries e JOIN formats f ON e.id=f.entry_id WHERE f.mime=? AND f.data=?',next(iter(formats.items()))).fetchone()
        assert match,'Offer was not captured'
        captured=dict(conn.execute('SELECT mime,data FROM formats WHERE entry_id=?',match))
        assert all(captured.get(k)==v for k,v in formats.items()), 'Expected bundle format changed: '+str({k:len(v) for k,v in captured.items()})
        conn.close()
        # Verify every offered format can be read back by a separate native client.
        for mime,value in formats.items():
            actual=subprocess.run(['wl-paste','--no-newline','--type',mime],check=True,capture_output=True,timeout=3).stdout
            assert actual==value,'Clipboard write mismatch'
    # A different image with identical MIME names must create a distinct entry.
    # PNG data differs through a valid tEXt chunk.
    second=png[:-12]+chunk(b'tEXt',b'Comment\0Second image')+png[-12:]
    publish({'image/png':second});time.sleep(.5)
    conn=sqlite3.connect(db);assert conn.execute("SELECT count(*) FROM entries WHERE kind='image'").fetchone()[0]==2;conn.close()
    print('PASS: Wayland native event capture and format roundtrip: text + HTML + RTF, PNG, multi-file URI list, consecutive image changes.')
finally:
    try:
        if original:
            publish({k:bytes(v) for k,v in original.items()})
            time.sleep(.2)
            for mime,data in original.items():
                restored=subprocess.run(['wl-paste','--no-newline','--type',mime],check=True,capture_output=True,timeout=3).stdout
                assert restored==bytes(data), 'Restored format differs: '+mime
        else:subprocess.run(['wl-copy','--clear'],check=False,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    finally:
        monitor.terminate();monitor.communicate(timeout=4)
        shutil.rmtree(work)
