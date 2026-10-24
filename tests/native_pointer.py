"""Real Wayland virtual pointer for isolated compositor acceptance tests only."""
import os,pathlib,subprocess

_SOURCE = r"""
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <wayland-client.h>
#include "pointer-client.h"
static struct zwlr_virtual_pointer_manager_v1 *manager;
static void global(void *data, struct wl_registry *r, uint32_t id, const char *name, uint32_t version) {
    if (!strcmp(name, "zwlr_virtual_pointer_manager_v1"))
        manager = wl_registry_bind(r, id, &zwlr_virtual_pointer_manager_v1_interface, 1);
}
static void removed(void *data, struct wl_registry *r, uint32_t id) {}
static const struct wl_registry_listener listener = {global, removed};
static uint32_t now(void) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint32_t)(t.tv_sec * 1000 + t.tv_nsec / 1000000);
}
int main(void) {
    struct wl_display *d = wl_display_connect(NULL);
    if (!d) return 2;
    struct wl_registry *r = wl_display_get_registry(d);
    wl_registry_add_listener(r, &listener, NULL);
    if (wl_display_roundtrip(d) < 0 || !manager) return 3;
    struct zwlr_virtual_pointer_v1 *p = zwlr_virtual_pointer_manager_v1_create_virtual_pointer(manager, NULL);
    if (wl_display_roundtrip(d) < 0) return 4;
    puts("ready"); fflush(stdout);
    char command[32]; int x, y;
    while (scanf("%31s %d %d", command, &x, &y) == 3) {
        if (!strcmp(command, "absolute"))
            zwlr_virtual_pointer_v1_motion_absolute(p, now(), x, y, 2560, 800);
        else if (!strcmp(command, "relative"))
            zwlr_virtual_pointer_v1_motion(p, now(), wl_fixed_from_int(x), wl_fixed_from_int(y));
        else if (!strcmp(command, "button"))
            zwlr_virtual_pointer_v1_button(p, now(), 272, x ? WL_POINTER_BUTTON_STATE_PRESSED : WL_POINTER_BUTTON_STATE_RELEASED);
        else return 5;
        zwlr_virtual_pointer_v1_frame(p);
        if (wl_display_roundtrip(d) < 0) return 6;
        puts("ok"); fflush(stdout);
    }
    zwlr_virtual_pointer_v1_destroy(p);
    wl_display_disconnect(d);
    return 0;
}
"""

class Pointer:
    def __init__(self, directory, env, log):
        root=pathlib.Path(directory)/'pointer-helper';root.mkdir()
        cargo=pathlib.Path(os.environ.get('CARGO_HOME',pathlib.Path.home()/'.cargo'))
        protocol=next((cargo/'registry/src').glob('*/wayland-protocols-wlr-*/wlr-protocols/unstable/wlr-virtual-pointer-unstable-v1.xml'))
        header=root/'pointer-client.h';source=root/'pointer-protocol.c';main=root/'main.c';binary=root/'pointer'
        subprocess.run(['wayland-scanner','client-header',str(protocol),str(header)],check=True)
        subprocess.run(['wayland-scanner','private-code',str(protocol),str(source)],check=True)
        main.write_text(_SOURCE)
        flags=subprocess.check_output(['pkg-config','--cflags','--libs','wayland-client'],text=True).split()
        subprocess.run(['cc',str(main),str(source),'-o',str(binary),*flags],check=True)
        self.process=subprocess.Popen([str(binary)],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True)
        assert self.process.stdout.readline().strip()=='ready','Wayland virtual pointer unavailable'
    def command(self, kind, x, y=0):
        self.process.stdin.write(f'{kind} {int(x)} {int(y)}\n');self.process.stdin.flush()
        assert self.process.stdout.readline().strip()=='ok','Wayland pointer injection failed'
