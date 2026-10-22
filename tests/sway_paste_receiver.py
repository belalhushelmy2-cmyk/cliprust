"""Native GTK test destination. Clipboard data is rendered and saved for assertions."""
import gi,sys,pathlib,os,json
from gi.repository import GLib
gi.require_version('Gtk','3.0')
from gi.repository import Gtk,Gdk
root=pathlib.Path(sys.argv[1]);GLib.set_prgname('cliprust-paste-receiver')
window=Gtk.Window(title='Cliprust paste acceptance target');window.set_default_size(500,300)
box=Gtk.Box(orientation=Gtk.Orientation.VERTICAL);window.add(box)
text=Gtk.TextView();box.pack_start(text,True,True,0)
picture=Gtk.Image();box.pack_start(picture,True,True,0)
buffer=text.get_buffer()
def changed(buffer):
 root.joinpath('text.txt').write_text(buffer.get_text(buffer.get_start_iter(),buffer.get_end_iter(),True))
buffer.connect('changed',changed)
clipboard=Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
def press(widget,event):
 if event.keyval not in [Gdk.KEY_v,Gdk.KEY_V] or not event.state&Gdk.ModifierType.CONTROL_MASK:return False
 mode=(root/'mode').read_text().strip()
 if mode=='image':
  def received(clipboard,pixbuf,data):
   if pixbuf:
    picture.set_from_pixbuf(pixbuf);pixbuf.savev(str(root/'received.png'),'png',[],[])
    (root/'pixels.json').write_text(json.dumps([pixbuf.get_width(),pixbuf.get_height(),list(pixbuf.get_pixels())]))
  clipboard.request_image(received,None);return True
 if mode=='files':
  def received(clipboard,data,user_data):
   if data.get_data() is not None:(root/'files.txt').write_bytes(data.get_data())
  clipboard.request_contents(Gdk.Atom.intern('text/uri-list',False),received,None);return True
 if os.environ.get('CLIPRUST_SWAY_INPUT')=='wtype':
  widget.emit('paste-clipboard');return True # Explicit mapping for the legacy backend probe only.
 return False # GTK's stock paste binding inserts the clipboard content.
text.connect('key-press-event',press)
window.connect('destroy',Gtk.main_quit);window.show_all();text.grab_focus();Gtk.main()
