// Contract tests for the actual KWin script; these do not replace a KDE runtime test.
const vm = require('node:vm');
const fs = require('node:fs');
const assert = require('node:assert/strict');
const signal = () => ({connect() {}});
const d1 = {id: 1}, d2 = {id: 2}, screen = {name: 'second'};
const target = {normalWindow:true, resourceClass:'editor', desktops:[d2], minimized:true};
const launcher = {resourceClass:'cliprust', desktops:[d1], minimized:true, frameGeometry:{}};
const other = {normalWindow:true, resourceClass:'browser', desktops:[d2]};
let windows = [target, launcher, other];
let active = target;
let refuseFocus = false;
const workspace = {
    currentDesktop:d2, activeScreen:screen, desktops:[d1,d2], screens:[screen],
    windowList:() => windows, windowActivated:signal(), windowAdded:signal(),
    sendClientToScreen(w,s) {w.output=s;},
    clientArea:() => ({x:1920,y:30,width:1280,height:720}),
    get activeWindow() {return active;},
    set activeWindow(w) {if (!refuseFocus) active=w;}
};
const context = vm.createContext({workspace, KWin:{MaximizeArea:0}, callDBus(){}});
vm.runInContext(fs.readFileSync('packaging/kde/cliprust/contents/code/main.js','utf8'),context);
const handle = (action,id=0) => Array.from(context.handle(action,id));
const id=handle('prepare')[1];
assert.equal(handle('show')[0],true);
assert.equal(launcher.desktops[0],d2);
assert.equal(launcher.output,screen);
assert.equal(launcher.minimized,false);
assert.equal(launcher.frameGeometry.width,860);
assert.equal(launcher.frameGeometry.x,2130);
assert.equal(handle('restore',id)[0],true);
assert.equal(active,target);
assert.equal(handle('verify',id)[0],true);
active=other;
assert.equal(handle('verify',id)[0],false);
assert.equal(handle('restore',id+1)[0],false);
assert.equal(handle('show')[0],true); // Keep-open does not replace the target.
assert.equal(handle('restore',id)[0],true);
active=launcher;refuseFocus=true;
assert.equal(handle('restore',id)[0],false);
refuseFocus=false;
windows=[launcher,other];
assert.equal(handle('check',id)[0],false); // Closed destination is never replaced silently.
active=other;
const next=handle('prepare')[1];
assert.notEqual(next,id);
assert.equal(handle('restore',next)[0],true);
workspace.screens=[];
assert.equal(handle('show')[0],false);
console.log('KWin contract tests passed: placement, focus refusal, stale/closed targets, repeated paste, monitor removal');
