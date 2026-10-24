#!/usr/bin/env python3
"""Linux/Xvfb integration test: inject real XTest mouse/key events into winit.

Uses only Python stdlib plus the platform's X11/XTest libraries. No mock app input
path, screenshots, user input recording, or additional Python packages.
"""
import ctypes as c
from pathlib import Path
import os, tempfile, hashlib, struct, json, zipfile
import subprocess
import time

x = c.CDLL('libX11.so.6')
t = c.CDLL('libXtst.so.6')
D, W, I, U = c.c_void_p, c.c_ulong, c.c_int, c.c_uint

def api(lib, name, result, *args):
    function = getattr(lib, name)
    function.restype, function.argtypes = result, args
    return function

open_display = api(x, 'XOpenDisplay', D, c.c_char_p)
root_window = api(x, 'XDefaultRootWindow', W, D)
query = api(x, 'XQueryTree', I, D, W, c.POINTER(W), c.POINTER(W), c.POINTER(c.POINTER(W)), c.POINTER(U))
fetch_name = api(x, 'XFetchName', I, D, W, c.POINTER(c.c_void_p))
free = api(x, 'XFree', I, c.c_void_p)
flush = api(x, 'XFlush', I, D)
translate = api(x, 'XTranslateCoordinates', I, D, W, W, I, I, c.POINTER(I), c.POINTER(I), c.POINTER(W))
focus = api(x, 'XSetInputFocus', I, D, W, I, W)
resize = api(x, 'XResizeWindow', I, D, W, U, U)
keysym = api(x, 'XStringToKeysym', W, c.c_char_p)
keycode = api(x, 'XKeysymToKeycode', U, D, W)
motion = api(t, 'XTestFakeMotionEvent', I, D, I, I, I, W)
button = api(t, 'XTestFakeButtonEvent', I, D, U, I, W)
key_event = api(t, 'XTestFakeKeyEvent', I, D, U, I, W)
close_display = api(x, 'XCloseDisplay', I, D)
display = open_display(None)
if not display:
    raise SystemExit('Xvfb/X11 display is required')
root = root_window(display)

def title(window):
    pointer = c.c_void_p()
    if not fetch_name(display, window, c.byref(pointer)) or not pointer:
        return ''
    try:
        return c.string_at(pointer).decode('utf-8', errors='replace')
    finally:
        free(pointer)

def find_window():
    parent, ancestor, count, children = W(), W(), U(), c.POINTER(W)()
    if not query(display, root, c.byref(ancestor), c.byref(parent), c.byref(children), c.byref(count)):
        return 0
    try:
        return next((children[i] for i in range(count.value) if 'Rust Paint Lab Alpha' in title(children[i])), 0)
    finally:
        if children:
            free(children)

def wait(predicate, description):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise AssertionError(f'Canvas exited early: {process.returncode}; {description}')
        value = predicate()
        if value:
            return value
        time.sleep(0.03)
    raise AssertionError(f'Timeout: {description}')

def send():
    flush(display)
    time.sleep(0.06)

def move(px, py):
    dx, dy, child = I(), I(), W()
    assert translate(display, window, root, 0, 0, c.byref(dx), c.byref(dy), c.byref(child))
    assert motion(display, -1, dx.value + px, dy.value + py, 0)
    send()

def click_button(number, pressed):
    assert button(display, number, int(pressed), 0)
    send()

def key(name):
    code = keycode(display, keysym(name.encode()))
    assert code and key_event(display, code, 1, 0) and key_event(display, code, 0, 0)
    send()

def status(text):
    return wait(lambda: text in title(window), text)


def ctrl(name):
    code=keycode(display,keysym(b'Control_L'));key_event(display,code,1,0);key(name);key_event(display,code,0,0);send()
def ctrl_shift(name):
    code=keycode(display,keysym(b'Shift_L'));key_event(display,code,1,0);ctrl(name);key_event(display,code,0,0);send()
def valid_project(path):
    if not path.exists(): return False
    data=path.read_bytes()
    if len(data)<48 or data[:8]!=b'RPLALP01' or hashlib.sha256(data[:-32]).digest()!=data[-32:]: return False
    n=struct.unpack('<I',data[8:12])[0]
    return struct.unpack('<I',data[12+n:16+n])[0]
with tempfile.TemporaryDirectory(prefix='rpl-alpha-window-') as folder:
    directory=Path(folder)
    executable=Path(__file__).resolve().parent.parent/'target/debug/canvas-demo'
    env={**os.environ,'XDG_STATE_HOME':str(directory/'state')}
    with (directory/'application.log').open('w') as log:
        process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            window=wait(find_window,'alpha workspace')
            focus(display,window,2,0);send()
            # Let font/layout requests settle before testing drawing ownership.
            time.sleep(0.5)
            move(180,250);click_button(1,True);move(400,400);click_button(1,False)
            status('*')
            ctrl('s');project=directory/'artwork.rpl'
            wait(lambda: valid_project(project) and '*' not in title(window),'valid saved raster project')
            initial=project.read_bytes()
            recent=directory/'state/rust-paint-lab/recent.json'
            assert json.loads(recent.read_text())['paths']==[str(project.resolve())]
            ctrl('z');ctrl('s')
            wait(lambda: valid_project(project)==0 and project.read_bytes()!=initial,'undo saved with no tiles')
            ctrl('y');ctrl('s')
            wait(lambda: bool(valid_project(project)) and '*' not in title(window),'redo saved raster tiles')
            assert (directory/'artwork.rpl.bak').exists()
            ctrl('e');png=directory/'artwork.png'
            wait(lambda: png.exists(),'PNG export')
            data=png.read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n'
            assert struct.unpack('>II',data[16:24])==(800,500)
            key('Escape');assert process.wait(timeout=20)==0
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'reopened workspace');focus(display,window,2,0);send()
            ctrl('o');time.sleep(0.3);key('Return')
            status('artwork.rpl')
            # Exercise actual selection-tool ownership and transactional fill.
            time.sleep(0.5)
            saved=project.read_bytes()
            key('m');move(200,260);click_button(1,True);move(360,370);click_button(1,False)
            assert '*' not in title(window), 'selection alone changed artwork'
            ctrl('f');status('*');ctrl('s')
            wait(lambda: project.read_bytes()!=saved and '*' not in title(window),'selected fill saved')
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==saved,'undo selected fill restores exact native artwork')
            ctrl('d');key('b')
            # Navigation changes view state, never authoritative artwork.
            move(300,300);click_button(4,True);click_button(4,False);time.sleep(0.2)
            key('0');ctrl('s')
            wait(lambda: project.read_bytes()==saved,'zoom and fit preserve artwork')
            ctrl_shift('e');ora=directory/'artwork.ora'
            wait(lambda: ora.exists(),'layered OpenRaster export')
            with zipfile.ZipFile(ora) as archive:
                assert archive.namelist()[0]=='mimetype' and archive.read('mimetype')==b'image/openraster'
                assert archive.testzip() is None and b'<layer' in archive.read('stack.xml')
            ctrl_shift('o');time.sleep(0.3);key('Return');status('*');ctrl('s')
            imported=directory/'imported-artwork.rpl'
            wait(lambda: bool(valid_project(imported)) and '*' not in title(window),'OpenRaster reopened and saved as editable native project')
            recovery_dir=directory/'state/rust-paint-lab/recovery'
            snapshot=wait(lambda: next((p for p in sorted(recovery_dir.glob('recovery-*.rpl'),reverse=True) if valid_project(p)),None),'initial recovery checkpoint')
            before=snapshot.read_bytes()
            key('b');move(230,350);click_button(1,True);move(400,470);click_button(1,False);status('*')
            wait(lambda: valid_project(snapshot) and snapshot.read_bytes()!=before,'latest committed artwork checkpoint')
            recovered_bytes=snapshot.read_bytes()
            process.kill();assert process.wait(timeout=10)!=0
            time.sleep(0.2)
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'workspace after forced crash');focus(display,window,2,0);send();time.sleep(0.5)
            ctrl_shift('r');status('*');ctrl('s')
            recovered=directory/'recovered-artwork.rpl'
            wait(lambda: recovered.exists() and recovered.read_bytes()==recovered_bytes and '*' not in title(window),'forced-crash recovery restores exact checkpoint bytes')


            key('Escape');assert process.wait(timeout=20)==0
            print('ALPHA_WORKFLOW_SMOKE: PASSED (real mouse raster paint, save checksum, undo/redo, previous-file backup, PNG export, reopen, rectangle selection/fill/undo, zoom/fit, layered OpenRaster export/import, forced-crash recovery, clean close)')
        finally:
            if process.poll() is None: process.terminate();process.wait(timeout=10)
            close_display(display)
            print((directory/'application.log').read_text())
