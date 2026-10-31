#!/usr/bin/env python3
"""Exercise image placement through the real Linux window and routed UI."""
import binascii
import ctypes as c
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time
import zlib

x11 = c.CDLL('libX11.so.6')
xtst = c.CDLL('libXtst.so.6')
D, W, I, U = c.c_void_p, c.c_ulong, c.c_int, c.c_uint

def api(lib, name, result, *args):
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = result, args
    return fn

open_display = api(x11, 'XOpenDisplay', D, c.c_char_p)
root_window = api(x11, 'XDefaultRootWindow', W, D)
query_tree = api(x11, 'XQueryTree', I, D, W, c.POINTER(W), c.POINTER(W), c.POINTER(c.POINTER(W)), c.POINTER(U))
fetch_name = api(x11, 'XFetchName', I, D, W, c.POINTER(c.c_void_p))
free = api(x11, 'XFree', I, c.c_void_p)
translate = api(x11, 'XTranslateCoordinates', I, D, W, W, I, I, c.POINTER(I), c.POINTER(I), c.POINTER(W))
focus = api(x11, 'XSetInputFocus', I, D, W, I, W)
resize = api(x11, 'XResizeWindow', I, D, W, U, U)
flush = api(x11, 'XFlush', I, D)
keysym = api(x11, 'XStringToKeysym', W, c.c_char_p)
keycode = api(x11, 'XKeysymToKeycode', U, D, W)
motion = api(xtst, 'XTestFakeMotionEvent', I, D, I, I, I, W)
button = api(xtst, 'XTestFakeButtonEvent', I, D, U, I, W)
key_event = api(xtst, 'XTestFakeKeyEvent', I, D, U, I, W)

display = open_display(None)
if not display:
    raise SystemExit('Xvfb/X11 display is required')
root = root_window(display)

def window_title(window):
    value = c.c_void_p()
    if not fetch_name(display, window, c.byref(value)) or not value:
        return ''
    try:
        return c.string_at(value).decode('utf-8', errors='replace')
    finally:
        free(value)

def find_window():
    parent, ancestor, count, children = W(), W(), U(), c.POINTER(W)()
    if not query_tree(display, root, c.byref(ancestor), c.byref(parent), c.byref(children), c.byref(count)):
        return 0
    try:
        return next((children[i] for i in range(count.value)
                     if 'RPL_UI_TEST_WINDOW' in window_title(children[i])), 0)
    finally:
        if children:
            free(children)

def key(name, control=False):
    ctrl = keycode(display, keysym(b'Control_L')) if control else 0
    code = keycode(display, keysym(name.encode()))
    if ctrl:
        key_event(display, ctrl, 1, 0)
    key_event(display, code, 1, 0)
    key_event(display, code, 0, 0)
    if ctrl:
        key_event(display, ctrl, 0, 0)
    flush(display)

def click(window, x, y):
    root_x, root_y, child = I(), I(), W()
    assert translate(display, window, root, 0, 0, c.byref(root_x), c.byref(root_y), c.byref(child))
    motion(display, -1, root_x.value + round(x), root_y.value + round(y), 0)
    button(display, 1, 1, 0)
    button(display, 1, 0, 0)
    flush(display)
    time.sleep(0.08)

def drag(window, start_x, start_y, end_x, end_y):
    root_x, root_y, child = I(), I(), W()
    assert translate(display, window, root, 0, 0, c.byref(root_x), c.byref(root_y), c.byref(child))
    origin_x, origin_y = root_x.value + round(start_x), root_y.value + round(start_y)
    motion(display, -1, origin_x, origin_y, 0)
    button(display, 1, 1, 0)
    for step in range(1, 9):
        fraction = step / 8
        motion(display, -1,
               origin_x + round((end_x - start_x) * fraction),
               origin_y + round((end_y - start_y) * fraction), 0)
        flush(display)
        time.sleep(0.015)
    button(display, 1, 0, 0)
    flush(display)
    time.sleep(0.08)

def wait(predicate, process, description, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise AssertionError(f'canvas exited early ({process.returncode}): {description}')
        result = predicate()
        if result:
            return result
        time.sleep(0.04)
    raise AssertionError(f'timed out waiting for {description}')

def png_chunk(kind, data):
    block = kind + data
    return struct.pack('>I', len(data)) + block + struct.pack('>I', binascii.crc32(block) & 0xffffffff)

def write_fixture(path):
    width, height = 96, 72
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            if x < width // 2 and y < height // 2:
                color = (244, 42, 190, 255)
            elif x >= width // 2 and y < height // 2:
                color = (58, 204, 248, 255)
            elif x < width // 2:
                color = (255, 199, 66, 255)
            else:
                color = (100, 50, 190, 210)
            rows.extend(color)
    raw = (b'\x89PNG\r\n\x1a\n'
           + png_chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
           + png_chunk(b'IDAT', zlib.compress(bytes(rows), 6))
           + png_chunk(b'IEND', b''))
    path.write_bytes(raw)

def capture(window, directory, name, log_path):
    request = directory / 'capture.request'
    capture_file = directory / f'{name}.rplcap'
    request.write_text(name)
    key('F12')
    deadline = time.monotonic() + 20
    while not capture_file.exists() and time.monotonic() < deadline:
        time.sleep(0.04)
    assert capture_file.exists(), 'wgpu preview screenshot was not returned'
    raw = capture_file.read_bytes()
    assert raw[:8] == b'RPLCAP01'
    width, height = struct.unpack('<II', raw[8:16])
    pixels = raw[16:]
    assert len(pixels) == width * height * 4
    bounds = marker(log_path, 'UI_TEST_IMAGE_PREVIEW_BOUNDS')
    assert bounds is not None, 'the canvas did not report rendered image-preview bounds'
    x, y, preview_width, preview_height, scale = bounds
    left = max(0, min(width, round(x * scale)))
    top = max(0, min(height, round(y * scale)))
    right = max(0, min(width, round((x + preview_width) * scale)))
    bottom = max(0, min(height, round((y + preview_height) * scale)))
    assert right - left >= 16 and bottom - top >= 16, \
        f'placement preview is too small to inspect: {bounds}'
    regions = {'pink': 0, 'cyan': 0, 'yellow': 0}
    for py in range(top + 2, bottom - 2):
        for px in range(left + 2, right - 2):
            offset = (py * width + px) * 4
            red, green, blue = pixels[offset:offset + 3]
            if red > 190 and blue > 120 and green < 105:
                regions['pink'] += 1
            if blue > 175 and green > 120 and red < 120:
                regions['cyan'] += 1
            if red > 190 and green > 125 and blue < 120:
                regions['yellow'] += 1
    assert all(count >= 16 for count in regions.values()), \
        f'preview does not show the imported image regions: {regions}'
    png_rows = b''.join(b'\0' + pixels[y * width * 4:(y + 1) * width * 4]
                        for y in range(height))
    screenshot = directory / f'{name}.png'
    screenshot.write_bytes(
        b'\x89PNG\r\n\x1a\n'
        + png_chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
        + png_chunk(b'IDAT', zlib.compress(png_rows, 6))
        + png_chunk(b'IEND', b''))
    assert len(set(pixels[0::4])) > 12, 'placement preview capture is visually empty'
    return screenshot

def marker(log_path, name):
    prefix = name + ': '
    lines = [line.partition(prefix)[2] for line in log_path.read_text().splitlines()
             if line.startswith(prefix)]
    return tuple(map(float, lines[-1].split())) if lines else None

def start_case(root_dir, source, cancel):
    name = 'cancel' if cancel else 'commit'
    work = root_dir / name
    work.mkdir()
    state = work / 'state'
    screens = work / 'screens'
    screens.mkdir()
    log_path = work / 'application.log'
    env = {
        **os.environ,
        'XDG_STATE_HOME': str(state),
        'RPL_TEST_PATH_DIALOG': '1',
        'RPL_TEST_IMAGE_PLACEMENT_PATH': str(source),
        'RPL_UI_SCREENSHOT_DIR': str(screens),
    }
    with log_path.open('w') as log:
        process = subprocess.Popen([str(Path('target/debug/canvas-demo').resolve())],
                                   cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            window = wait(find_window, process, 'visible placement window')
            focus(display, window, 2, 0)
            resize(display, window, 1280, 800)
            flush(display)
            wait(lambda: 'UI_TEST_IMAGE_PLACEMENT_PREVIEW:' in log_path.read_text(),
                 process, 'decoded image preview')
            button_name = 'UI_TEST_IMAGE_CANCEL_BUTTON_RECT' if cancel else 'UI_TEST_IMAGE_PLACE_BUTTON_RECT'
            bounds = wait(lambda: marker(log_path, button_name), process, 'placement action bounds')
            x, y, width, height, scale = bounds
            assert width >= 40 and height >= 18, f'placement action is too small: {bounds}'
            if cancel:
                screenshot = capture(window, screens, 'cancelled-image-preview', log_path)
                print(f'PLACE_IMAGE_CANCEL_SCREENSHOT: {screenshot}', flush=True)
            else:
                screenshot = capture(window, screens, 'placed-image-preview', log_path)
                print(f'PLACE_IMAGE_SCREENSHOT: {screenshot}', flush=True)
                initial = marker(log_path, 'UI_TEST_IMAGE_PLACEMENT_PREVIEW')
                assert initial is not None and len(initial) == 6
                scale_rect = wait(lambda: marker(log_path, 'UI_TEST_IMAGE_SCALE_SLIDER_RECT'),
                                  process, 'interactive image scale control')
                # The response rect includes the numeric value and label. Drag
                # the visible handle along the track rather than clicking the
                # value or label portion of that wider response rectangle.
                drag(window, (scale_rect[0] + scale_rect[2] * 0.35) * scale_rect[4],
                     (scale_rect[1] + scale_rect[3] / 2) * scale_rect[4],
                     (scale_rect[0] + scale_rect[2] * 0.18) * scale_rect[4],
                     (scale_rect[1] + scale_rect[3] / 2) * scale_rect[4])
                scaled = wait(lambda: (lambda value: value
                                       if value and abs(value[2] - initial[2]) > 0.1
                                       else None)(marker(log_path,
                                           'UI_TEST_IMAGE_PLACEMENT_PREVIEW')),
                              process, 'scale control changes the pending image transform')
                rotate_rect = wait(lambda: marker(log_path, 'UI_TEST_IMAGE_ROTATE_SLIDER_RECT'),
                                   process, 'interactive image rotation control')
                drag(window, (rotate_rect[0] + rotate_rect[2] * 0.24) * rotate_rect[4],
                     (rotate_rect[1] + rotate_rect[3] / 2) * rotate_rect[4],
                     (rotate_rect[0] + rotate_rect[2] * 0.36) * rotate_rect[4],
                     (rotate_rect[1] + rotate_rect[3] / 2) * rotate_rect[4])
                rotated = wait(lambda: (lambda value: value
                                        if value and abs(value[3]) > 1.0
                                        else None)(marker(log_path,
                                            'UI_TEST_IMAGE_PLACEMENT_PREVIEW')),
                               process, 'rotation control changes the pending image transform')
                preview_rect = wait(lambda: marker(log_path,
                                                   'UI_TEST_IMAGE_PREVIEW_BOUNDS'),
                                    process, 'transformed preview bounds')
                start_x = preview_rect[0] + preview_rect[2] / 2
                start_y = preview_rect[1] + preview_rect[3] / 2
                drag(window, start_x * preview_rect[4], start_y * preview_rect[4],
                     (start_x + 60) * preview_rect[4], (start_y + 40) * preview_rect[4])
                moved = wait(lambda: (lambda value: value
                                      if value and abs(value[4] - rotated[4])
                                      + abs(value[5] - rotated[5]) > 0.1
                                      else None)(marker(log_path,
                                          'UI_TEST_IMAGE_PLACEMENT_PREVIEW')),
                             process, 'dragging moves the pending image without painting')
                assert moved[2] == scaled[2] and moved[3] == rotated[3], \
                    'moving the image should preserve its scale and rotation'
                screenshot = capture(window, screens, 'placed-image-transformed-preview', log_path)
                print(f'PLACE_IMAGE_TRANSFORM_SCREENSHOT: {screenshot}', flush=True)
                bounds = wait(lambda: marker(log_path, button_name),
                              process, 'transformed placement action bounds')
                x, y, width, height, scale = bounds
            click(window, (x + width / 2) * scale, (y + height / 2) * scale)
            if cancel:
                wait(lambda: 'UI_TEST_IMAGE_PLACEMENT_CANCELLED: 1 0 ' in log_path.read_text(),
                     process, 'cancel leaves artwork unchanged')
                assert not (work / 'artwork.rpl').exists(), 'cancel unexpectedly saved changed artwork'
                print('PLACE_IMAGE_CANCEL: PASS', flush=True)
            else:
                wait(lambda: 'UI_TEST_IMAGE_PLACEMENT_COMMITTED:' in log_path.read_text(),
                     process, 'editable layer commit')
                key('s', control=True)
                project = work / 'artwork.rpl'
                wait(lambda: project.exists(), process, 'native project save')
                time.sleep(0.2)
                data = project.read_bytes()
                assert data[:8] == b'RPLALP01' and hashlib.sha256(data[:-32]).digest() == data[-32:]
                metadata_length = struct.unpack('<I', data[8:12])[0]
                state_doc = json.loads(data[12:12 + metadata_length])
                assert len(state_doc['layers']) == 2 and state_doc['layers'][-1]['name'] == 'test-pattern'
                tile_count_offset = 12 + metadata_length
                tile_count = struct.unpack('<I', data[tile_count_offset:tile_count_offset + 4])[0]
                assert tile_count > 0, 'placed layer contains no raster tiles'
                print(f'PLACE_IMAGE_COMMIT: PASS layers={len(state_doc["layers"])} tiles={tile_count}', flush=True)
                placed_bytes = project.read_bytes()
                key('z', control=True)
                key('s', control=True)
                wait(lambda: len(json.loads(project.read_bytes()[12:12 +
                     struct.unpack('<I', project.read_bytes()[8:12])[0]])['layers']) == 1,
                     process, 'Undo removes the placed layer in the real application')
                key('y', control=True)
                key('s', control=True)
                wait(lambda: project.read_bytes() == placed_bytes,
                     process, 'Redo restores the placed image exactly')
            key('Escape')
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                key('Escape')
                process.wait(timeout=10)
            assert process.returncode == 0
        except Exception:
            print(f'PLACE_IMAGE_APP_LOG: {log_path}', flush=True)
            if log_path.exists():
                print(log_path.read_text()[-12000:], flush=True)
            raise
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
    return screens

with tempfile.TemporaryDirectory(prefix='bitchpixel-place-image-') as temp:
    root_dir = Path(temp)
    source = root_dir / 'test-pattern.png'
    write_fixture(source)
    start_case(root_dir, source, cancel=True)
    screens = start_case(root_dir, source, cancel=False)
    output_dir = Path(os.environ.get('RPL_UI_SCREENSHOT_DIR', '/tmp/rpl-place-image-screens'))
    output_dir.mkdir(parents=True, exist_ok=True)
    for screenshot in screens.glob('*.png'):
        (output_dir / screenshot.name).write_bytes(screenshot.read_bytes())
print('PLACE_IMAGE_WORKFLOW_SMOKE: PASSED', flush=True)
