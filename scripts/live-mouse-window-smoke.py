#!/usr/bin/env python3
"""Linux/Xvfb integration test: inject real XTest mouse/key events into winit.

Uses only Python stdlib plus the platform's X11/XTest libraries. No mock app input
path, screenshots, user input recording, or additional Python packages.
"""
import ctypes as c
from pathlib import Path
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
        return next((children[i] for i in range(count.value) if 'LIVE mouse' in title(children[i])), 0)
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

log_path = Path('/tmp/rpl-live-window.log')
with log_path.open('w') as log:
    process = subprocess.Popen(['target/debug/canvas-demo'], stdout=log, stderr=subprocess.STDOUT)
    try:
        window = wait(find_window, 'live drawing window')
        focus(display, window, 2, 0)
        send()
        move(200, 220)
        click_button(1, True)
        status('DRAWING')
        move(400, 320)
        click_button(1, False)
        status('1 completed')
        click_button(3, True)
        move(450, 340)
        click_button(3, False)
        status('1 completed')
        click_button(1, True)
        status('DRAWING')
        move(2, 2)  # Outside paper: cancel.
        status('READY')
        move(200, 220)
        click_button(1, False)
        status('1 completed')
        click_button(1, True)
        status('DRAWING')
        focus(display, root, 2, 0)
        send()
        status('READY')
        click_button(1, False)
        focus(display, window, 2, 0)
        send()
        move(220, 240)
        click_button(1, True)
        status('DRAWING')
        key('p')
        status('REPLAY')
        click_button(1, False)
        key('l')
        status('1 completed')
        key('c')
        status('0 completed')
        resize(display, window, 832, 532)
        send()
        move(116, 116)
        click_button(1, True)
        move(316, 216)
        click_button(1, False)
        status('1 completed')
        key('Escape')
        assert process.wait(timeout=15) == 0
        print('LIVE_WINDOW_SMOKE: PASSED (OS mouse events, right-button exclusion, boundary/focus cancellation, modes, clear, resize, close)')
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        close_display(display)
        print(log_path.read_text())
