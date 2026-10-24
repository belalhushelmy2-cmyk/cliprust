// Enabled explicitly in KWin Scripts. No shortcuts are registered or replaced.
var service = "org.cliprust.KWin";
var path = "/org/cliprust/KWin";
var polling = false;
var previous = null;
var generation = 0;
var desktop = workspace.currentDesktop;
var output = workspace.activeScreen;
function isCliprust(w) {
    return w && (String(w.resourceClass) === "cliprust" || String(w.resourceClass) === "org.cliprust.Cliprust");
}
function exists(w) { return w && workspace.windowList().indexOf(w) >= 0; }
function remember(w) {
    if (w && !isCliprust(w) && w.normalWindow && w !== previous) {
        previous = w;
        generation++;
    }
}
remember(workspace.activeWindow);
// Track the last external window, but don't change the chosen target during paste.
var prepared = false;
workspace.windowActivated.connect(function(w) { if (!prepared) remember(w); });
function handle(action, target) {
    if (action === "prepare") {
        if (!isCliprust(workspace.activeWindow)) remember(workspace.activeWindow);
        desktop = workspace.currentDesktop;
        output = workspace.activeScreen;
        prepared = true;
        return [true, generation, ""];
    }
    if (action === "show") {
        var windows = workspace.windowList().filter(isCliprust);
        if (windows.length !== 1) return [false, generation, "Cliprust surface is not ready or multiple Cliprust windows exist"];
        var w = windows[0];
        if (workspace.desktops.indexOf(desktop) < 0 || workspace.screens.indexOf(output) < 0)
            return [false, generation, "Invoking desktop or monitor was removed; summon Cliprust again"];
        w.desktops = [desktop];
        w.minimized = false;
        w.keepAbove = false;
        workspace.sendClientToScreen(w, output);
        var area = workspace.clientArea(KWin.MaximizeArea, output, desktop);
        var geometry = w.frameGeometry;
        geometry.width = Math.min(geometry.width, area.width);
        geometry.height = Math.min(geometry.height, area.height);
        geometry.x = Math.max(area.x, Math.min(geometry.x, area.x + area.width - geometry.width));
        geometry.y = Math.max(area.y, Math.min(geometry.y, area.y + area.height - geometry.height));
        w.frameGeometry = geometry;
        workspace.currentDesktop = desktop;
        workspace.activeWindow = w;
        return [workspace.activeWindow === w, generation, "Compositor refused launcher focus"];
    }
    if (!prepared || target !== generation || !exists(previous))
        return [false, generation, "Previous application is unavailable; summon Cliprust from the destination app"];
    if (action === "check") return [true, generation, ""];
    if (action === "restore") {
        previous.minimized = false;
        if (previous.desktops.length) workspace.currentDesktop = previous.desktops[0];
        workspace.activeWindow = previous;
        return [workspace.activeWindow === previous, generation, "Compositor refused target focus"];
    }
    if (action === "verify") return [workspace.activeWindow === previous, generation, "Paste target lost focus"];
    return [false, generation, "Unknown integration command"];
}
function poll() {
    callDBus(service, path, service, "NextRequest", function(id, action, target, deadline) {
        if (typeof action !== "string") { polling = false; return; }
        if (action === "idle") { poll(); return; }
        var result;
        try { result = Date.now() > Number(deadline) ? [false, generation, "Expired request"] : handle(action, Number(target)); }
        catch (error) { result = [false, generation, String(error)]; }
        callDBus(service, path, service, "Report", id, result[0], String(result[1]), result[2], poll);
    });
}
function start(w) {
    if (isCliprust(w) && !polling) { polling = true; poll(); }
}
workspace.windowAdded.connect(start);
workspace.windowList().forEach(start);
