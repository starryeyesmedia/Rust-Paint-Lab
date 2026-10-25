#!/usr/bin/env python3
"""Linux/Xvfb integration test: inject real XTest mouse/key events into winit.

Uses only Python stdlib plus the platform's X11/XTest libraries. No mock app input
path, screenshots, user input recording, or additional Python packages.
"""
import ctypes as c
from pathlib import Path
import os, tempfile, hashlib, struct, json, zipfile, zlib, binascii
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
sync = api(x, 'XSync', I, D, I)
set_error_handler = api(x, 'XSetErrorHandler', c.c_void_p, c.c_void_p)
display = open_display(None)
if not display:
    raise SystemExit('Xvfb/X11 display is required')
root = root_window(display)
captured_screenshots = {}
x_error_count = 0
def handle_x_error(_display, _event):
    global x_error_count
    x_error_count += 1
    return 0
x_error_callback = c.CFUNCTYPE(I, D, c.c_void_p)(handle_x_error)
set_error_handler(c.cast(x_error_callback, c.c_void_p))

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

def brush_rect():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_BRUSH_RECT: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def send():
    flush(display)
    time.sleep(0.06)

def trace(action, *details):
    print('UI_TRACE:', action, *details, flush=True)

def move(px, py):
    trace('move',px,py)
    dx, dy, child = I(), I(), W()
    errors_before=x_error_count
    translated=translate(display, window, root, 0, 0, c.byref(dx), c.byref(dy), c.byref(child))
    sync(display,0)
    if x_error_count != errors_before:
        raise AssertionError(f'X11 window {int(window):#x} was destroyed before pointer move ({px},{py})')
    assert translated, f'XTranslateCoordinates rejected pointer move ({px},{py})'
    assert motion(display, -1, dx.value + px, dy.value + py, 0)
    send()

def click_button(number, pressed):
    trace('button',number,'down' if pressed else 'up')
    assert button(display, number, int(pressed), 0)
    send()

def key(name):
    trace('key',name)
    code = keycode(display, keysym(name.encode()))
    assert code and key_event(display, code, 1, 0) and key_event(display, code, 0, 0)
    send()

def status(text):
    try:
        return wait(lambda: text in title(window), text)
    except AssertionError as error:
        raise AssertionError(f'{error}; current window title: {title(window)!r}') from error

def path_prompt():
    return wait(lambda: 'TEST FILE PICKER' in title(window), 'test file picker is visible')

def new_document_prompt():
    return wait(lambda: 'TEST NEW DOCUMENT' in title(window), 'new document dialog is visible')


def ctrl(name):
    code=keycode(display,keysym(b'Control_L'));key_event(display,code,1,0);key(name);key_event(display,code,0,0);send()
def ctrl_shift(name):
    code=keycode(display,keysym(b'Shift_L'));key_event(display,code,1,0);ctrl(name);key_event(display,code,0,0);send()
def ctrl_alt(name):
    code=keycode(display,keysym(b'Alt_L'));key_event(display,code,1,0);ctrl(name);key_event(display,code,0,0);send()
def valid_project(path):
    if not path.exists(): return False
    return valid_project_bytes(path.read_bytes())
def valid_project_bytes(data):
    if len(data)<48 or data[:8]!=b'RPLALP01' or hashlib.sha256(data[:-32]).digest()!=data[-32:]: return False
    n=struct.unpack('<I',data[8:12])[0]
    return struct.unpack('<I',data[12+n:16+n])[0]
def latest_journal_checkpoint(path):
    if not path.exists(): return None
    data=path.read_bytes();offset=0;last_sequence=0;latest=None
    while offset<len(data):
        if len(data)-offset<56 or data[offset:offset+8]!=b'RPLJRN01': return None
        sequence,length=struct.unpack('<QQ',data[offset+8:offset+24])
        end=offset+24+length;record_end=end+32
        if sequence<=last_sequence or record_end>len(data): return None
        if hashlib.sha256(data[offset:end]).digest()!=data[end:record_end]: return None
        payload=data[offset+24:end]
        if valid_project_bytes(payload) is False: return None
        latest=payload;last_sequence=sequence;offset=record_end
    return latest
def native_state(path):
    data=path.read_bytes();n=struct.unpack('<I',data[8:12])[0]
    return json.loads(data[12:12+n])
def png_chunk(kind, data):
    body=kind+data
    return struct.pack('>I',len(data))+body+struct.pack('>I',binascii.crc32(body)&0xffffffff)

def capture_window(window, name):
    folder=os.environ.get('RPL_UI_SCREENSHOT_DIR')
    if not folder: return
    print(f'UI_SCREENSHOT_CAPTURE_BEGIN: {name}', flush=True)
    # Use the X11 utility for the image readback. Calling XGetImage through
    # ctypes against wgpu-owned Vulkan windows caused runner-only segmentation
    # faults on some X servers. xwd performs that native operation safely and
    # writes a small, well-defined header that this script converts to PNG.
    dump=subprocess.check_output(['xwd','-silent','-id',hex(int(window))])
    assert len(dump)>=100, 'xwd returned a truncated window image'
    header_be=struct.unpack('>25I',dump[:100])
    header_le=struct.unpack('<25I',dump[:100])
    header=header_be if 100<=header_be[0]<=4196 and header_be[1]==7 else header_le
    header_size,version,pixmap_format,depth,width,height,xoffset,byte_order,bitmap_unit,bitmap_bit_order,bitmap_pad,bits_per_pixel,bytes_per_line,visual_class,red_mask,green_mask,blue_mask,bits_per_rgb,colormap_entries,ncolors,window_width,window_height,window_x,window_y,window_border=header
    assert version==7 and header_size>=100 and width>0 and height>0, 'xwd returned invalid image metadata'
    assert pixmap_format==2 and bits_per_pixel in (16,24,32) and bytes_per_line>=width*((bits_per_pixel+7)//8), 'unsupported X11 screenshot pixel layout'
    assert width<=4096 and height<=4096 and bytes_per_line*height<=128*1024*1024, 'X11 screenshot exceeds the capture bounds'
    pixel_offset=header_size+ncolors*12
    pixel_data=dump[pixel_offset:pixel_offset+bytes_per_line*height]
    assert len(pixel_data)==bytes_per_line*height, 'xwd pixel buffer is truncated'
    masks=(red_mask,green_mask,blue_mask)
    def component(pixel, mask):
        if not mask: return 0
        shift=(mask & -mask).bit_length()-1
        maximum=mask >> shift
        return (((pixel & mask) >> shift)*255 + maximum//2)//maximum
    rows=bytearray()
    bytes_per_pixel=(bits_per_pixel+7)//8
    byte_order_name='little' if byte_order==0 else 'big'
    for y in range(height):
        rows.append(0)
        row=pixel_data[y*bytes_per_line:(y+1)*bytes_per_line]
        for x_pixel in range(width):
            start=x_pixel*bytes_per_pixel
            pixel=int.from_bytes(row[start:start+bytes_per_pixel],byte_order_name)
            rows.extend((component(pixel,masks[0]),component(pixel,masks[1]),component(pixel,masks[2]),255))
    raw=b'\x89PNG\r\n\x1a\n'+png_chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,6,0,0,0))+png_chunk(b'IDAT',zlib.compress(bytes(rows),6))+png_chunk(b'IEND',b'')
    # Guard against capturing a blank, obscured, or uninitialized window while
    # still allowing per-driver antialiasing and font differences.
    sampled=set()
    sample_stride=max(1,(width*height)//4096)
    for index in range(0,width*height,sample_stride):
        sample_y,sample_x=divmod(index,width)
        start=sample_y*bytes_per_line+sample_x*bytes_per_pixel
        pixel=int.from_bytes(pixel_data[start:start+bytes_per_pixel],byte_order_name)
        sampled.add(tuple(component(pixel,mask) for mask in masks))
    # The high-contrast palette intentionally reduces the number of colors.
    # Still reject a blank/uninitialized image, while not requiring the richer
    # palette diversity used by normal and dark-theme screenshots.
    minimum_colors = 2 if name == 'home-dark-high-contrast' else 18
    print(f'UI_SCREENSHOT_COLOR_COUNT: {name} {len(sampled)}', flush=True)
    assert len(sampled)>minimum_colors, f'{name} screenshot looks blank or was not fully rendered ({len(sampled)} sampled colors; expected >{minimum_colors})'
    destination=Path(folder);destination.mkdir(parents=True,exist_ok=True)
    (destination/f'{name}.png').write_bytes(raw)
    captured_screenshots[name] = (width, height)
    print(f'UI_SCREENSHOT_CAPTURED: {name} {width}x{height}', flush=True)

def resize_window(window, width, height):
    assert resize(display, window, width, height)
    sync(display, 0)
    time.sleep(0.35)
def decode_rgba_png(data):
    assert data[:8]==b'\x89PNG\r\n\x1a\n'
    offset=8;width=height=bit_depth=color_type=None;compressed=bytearray()
    while offset<len(data):
        length=struct.unpack('>I',data[offset:offset+4])[0]
        kind=data[offset+4:offset+8];payload=data[offset+8:offset+8+length]
        if kind==b'IHDR':
            width,height,bit_depth,color_type,compression,filter_method,interlace=struct.unpack('>IIBBBBB',payload)
            assert (bit_depth,color_type,compression,filter_method,interlace)==(8,6,0,0,0)
        elif kind==b'IDAT': compressed.extend(payload)
        elif kind==b'IEND': break
        offset+=12+length
    assert width and height and compressed
    row_bytes=width*4;raw=zlib.decompress(compressed)
    assert len(raw)==height*(row_bytes+1)
    pixels=bytearray(width*height*4);previous=bytearray(row_bytes);source=0
    def paeth(left,up,up_left):
        estimate=left+up-up_left
        distances=(abs(estimate-left),abs(estimate-up),abs(estimate-up_left))
        return (left,up,up_left)[distances.index(min(distances))]
    for y in range(height):
        filter_type=raw[source];source+=1
        row=bytearray(raw[source:source+row_bytes]);source+=row_bytes
        for i,value in enumerate(row):
            left=row[i-4] if i>=4 else 0;up=previous[i]
            up_left=previous[i-4] if i>=4 else 0
            if filter_type==1: predictor=left
            elif filter_type==2: predictor=up
            elif filter_type==3: predictor=(left+up)//2
            elif filter_type==4: predictor=paeth(left,up,up_left)
            else:
                assert filter_type==0
                predictor=0
            row[i]=(value+predictor)&255
        start=y*row_bytes;pixels[start:start+row_bytes]=row;previous=row
    return width,height,pixels
with tempfile.TemporaryDirectory(prefix='rpl-alpha-window-') as folder:
    directory=Path(folder)
    executable=Path(__file__).resolve().parent.parent/'target/debug/canvas-demo'
    env={**os.environ,'XDG_STATE_HOME':str(directory/'state'),'RPL_TEST_PATH_DIALOG':'1','RPL_TEST_DOCUMENT_NAME':'artwork'}
    # Load a persisted, colorful reference-board illustration. The magenta
    # sentinel at its first pixel must never leak into exported artwork.
    reference=directory/'reference.png'
    reference_width, reference_height = 480, 320
    reference_rows=bytearray()
    for y in range(reference_height):
        reference_rows.append(0)
        for x_pixel in range(reference_width):
            if x_pixel == 0 and y == 0:
                color=(255,0,255,255)
            elif (x_pixel-360)**2+(y-70)**2 < 38**2:
                color=(255,205,112,255)
            elif y > 230 or y > 180 and x_pixel < 130 + (y-180)*1.25:
                color=(58,91,79,255)
            elif y > 205 and x_pixel > 255 - (y-205)*1.35:
                color=(80,103,120,255)
            elif y > 270:
                color=(34,58,54,255)
            else:
                blend=y/reference_height
                color=(round(104-20*blend),round(148-28*blend),round(190-18*blend),255)
            reference_rows.extend(color)
    reference.write_bytes(b'\x89PNG\r\n\x1a\n'+png_chunk(b'IHDR',struct.pack('>IIBBBBB',reference_width,reference_height,8,6,0,0,0))+png_chunk(b'IDAT',zlib.compress(bytes(reference_rows),6))+png_chunk(b'IEND',b''))
    prefs=directory/'state/rust-paint-lab/workspace.json'
    prefs.parent.mkdir(parents=True)
    prefs.write_text(json.dumps({'version':1,'dark_theme':False,'brush_panel_width':280.0,'references':[{'path':str(reference),'position':[620.0,100.0],'size':[320.0,260.0],'opacity':0.7}]}))
    with (directory/'application.log').open('w') as log:
        process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            window=wait(find_window,'alpha workspace')
            focus(display,window,2,0);send()
            # Let font/layout requests settle before testing drawing ownership.
            time.sleep(0.5)
            resize_window(window,1280,800)
            capture_window(window,'home-light-normal')
            resize_window(window,640,480)
            capture_window(window,'home-light-narrow')
            resize_window(window,1280,800)
            ctrl('n');new_document_prompt();time.sleep(0.25)
            capture_window(window,'new-artwork-light-normal')
            resize_window(window,640,480)
            capture_window(window,'new-artwork-light-narrow')
            resize_window(window,1280,800)
            key('Return');time.sleep(0.5)
            capture_window(window,'canvas-workspace-light')
            capture_window(window,'reference-board-light')
            left, top, width, height, scale = wait(brush_rect, 'Brush control geometry')
            move(round((left + width / 2) * scale), round((top + height / 2) * scale))
            click_button(1, True)
            click_button(1, False)
            wait(lambda: 'UI_TEST_BRUSH_FLYOUT_OPENED' in (directory / 'application.log').read_text(),
                 'palette Brush control opens its flyout')
            time.sleep(0.25)
            capture_window(window,'brush-library-light-normal')
            resize_window(window,640,480)
            capture_window(window,'brush-library-light-narrow')
            resize_window(window,1280,800)
            key('Escape')
            time.sleep(0.3)
            assert process.poll() is None, 'Escape closed the application instead of dismissing Brush library'
            wait(lambda: 'UI_TEST_ESCAPE_DISMISSED: brush' in (directory / 'application.log').read_text(),
                 'Escape dismissed the Brush flyout')
            window=wait(find_window,'window after Escape dismisses Brush library')
            ctrl('b')
            wait(lambda: 'UI_TEST_BRUSH_FLYOUT_OPENED: shortcut' in (directory / 'application.log').read_text(),
                 'Ctrl+B opens Brush library for keyboard users')
            key('Escape')
            assert process.poll() is None, 'Escape closed the application instead of dismissing keyboard-opened Brush library'
            wait(lambda: (directory / 'application.log').read_text().count('UI_TEST_ESCAPE_DISMISSED: brush') >= 2,
                 'Escape dismisses the keyboard-opened Brush flyout')
            ctrl('l');time.sleep(0.3)
            window=wait(find_window,'window after Ctrl+L opens Layers')
            ctrl_shift('n');ctrl_shift('n');time.sleep(0.3)
            wait(lambda: 'UI_TEST_LAYERS_ADDED: 3' in (directory/'application.log').read_text(), 'Layers panel contains three real editable layers')
            capture_window(window,'layers-panel-populated-light')
            ctrl('l')
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
            assert struct.unpack('>II',data[16:24])==(1600,1000)
            png_width,png_height,pixels=decode_rgba_png(data)
            assert (png_width,png_height)==(1600,1000)
            assert not any(pixels[i:i+4]==b'\xff\x00\xff\xff' for i in range(0,len(pixels),4)), 'workspace reference pixels leaked into the exported artwork'
            assert json.loads(prefs.read_text())['references'][0]['path']==str(reference)
            key('Escape');assert process.wait(timeout=20)==0
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'reopened workspace');focus(display,window,2,0);send()
            ctrl('o');path_prompt();ctrl('Return')
            status('artwork.rpl')
            # Save and reload a custom versioned brush preset through the app UI.
            ctrl_shift('b');path_prompt();ctrl('Return')
            preset=directory/'artwork.rplbrush'
            wait(lambda: preset.exists(),'custom brush preset save')
            preset_data=json.loads(preset.read_text())
            assert preset_data['version']==1 and preset_data['brush']['kind']=='Round'
            ctrl_alt('b');path_prompt();ctrl('Return');ctrl('s')
            wait(lambda: native_state(project)['brush']==preset_data['brush'],'brush preset load and project save')
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
            ctrl_shift('o');path_prompt();ctrl('Return');status('*')
            imported=directory/'imported-artwork.rpl'
            recovery_dir=directory/'state/rust-paint-lab/recovery'
            snapshot=wait(lambda: next((p for p in sorted(recovery_dir.glob('recovery-*.rpl'),reverse=True) if valid_project(p)),None),'initial recovery checkpoint')
            # Recovery snapshots are submitted only while the current document is
            # dirty. Wait for this checkpoint before an explicit save makes it clean.
            ctrl_shift('m');time.sleep(0.4);capture_window(window,'recovery-manager-populated-light');ctrl_shift('m')
            ctrl('s')
            wait(lambda: bool(valid_project(imported)) and '*' not in title(window),'OpenRaster reopened and saved as editable native project')
            journal_path=Path(str(snapshot)+'.journal')
            assert latest_journal_checkpoint(journal_path)==snapshot.read_bytes(),'initial checkpoint has a complete synced journal record'
            before=snapshot.read_bytes()
            journal_before=journal_path.read_bytes()
            key('b');move(230,350);click_button(1,True);move(400,470);click_button(1,False);status('*')
            wait(lambda: valid_project(snapshot) and snapshot.read_bytes()!=before,'latest committed artwork checkpoint')
            recovered_bytes=snapshot.read_bytes()
            wait(lambda: journal_path.read_bytes()!=journal_before and latest_journal_checkpoint(journal_path)==recovered_bytes,'latest checkpoint is durably journaled before process termination')
            preserved_artwork=imported.read_bytes()
            process.kill();assert process.wait(timeout=10)!=0
            # Exercise recovery from the durable journal without its paired snapshot.
            snapshot.unlink()
            time.sleep(0.2)
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'workspace after forced crash');focus(display,window,2,0);send();time.sleep(0.5)
            ctrl_shift('r');status('*');ctrl('s')
            recovered=directory/'recovered-artwork.rpl'
            wait(lambda: recovered.exists() and recovered.read_bytes()==recovered_bytes and '*' not in title(window),'forced-crash journal recovery restores exact checkpoint bytes')
            assert imported.read_bytes()==preserved_artwork,'journal recovery leaves existing native artwork unchanged'


            key('Escape');assert process.wait(timeout=20)==0
            saved_preferences=json.loads(prefs.read_text())
            assert saved_preferences['dark_theme'] is False
            assert saved_preferences['references'][0]['path']==str(reference)
            assert 180.0<=saved_preferences['brush_panel_width']<=640.0
            # Reopen on Home to verify the real recent-project card uses its
            # bounded preview cache after an application restart.
            thumb_directory=directory/'state/rust-paint-lab/home-thumbnails'
            assert any(thumb_directory.glob('rpl-*.png')),'saved artwork generated a Home thumbnail cache entry'
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'Home after reopening saved artwork');focus(display,window,2,0);send();time.sleep(0.5)
            capture_window(window,'home-recent-artwork-light')
            key('Escape');assert process.wait(timeout=20)==0
            # Exercise the same application windows under dark mode and high
            # contrast. Seed Recovery with an independently validated .rpl file.
            preferences=json.loads(prefs.read_text())
            preferences['appearance_mode']='Dark';preferences['dark_theme']=True;preferences['high_contrast']=False
            prefs.write_text(json.dumps(preferences))
            recovery_dir.mkdir(parents=True,exist_ok=True)
            visual_checkpoint=recovery_dir/'recovery-visual-checkpoint.rpl'
            visual_checkpoint.write_bytes(recovered.read_bytes())
            assert valid_project(visual_checkpoint),'dark-theme recovery fixture is a valid native project'
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'dark Home after saved artwork restart');focus(display,window,2,0);send();time.sleep(0.5)
            resize_window(window,1280,800);capture_window(window,'home-dark-normal')
            resize_window(window,640,480);capture_window(window,'home-dark-narrow')
            resize_window(window,1280,800)
            ctrl('n');new_document_prompt();capture_window(window,'new-artwork-dark-normal');key('Return');time.sleep(0.4)
            capture_window(window,'reference-board-dark')
            ctrl('b');time.sleep(0.35);capture_window(window,'brush-library-dark')
            key('Escape');ctrl('l');time.sleep(0.35);capture_window(window,'layers-panel-populated-dark')
            ctrl_shift('m');time.sleep(0.35)
            assert any(p.name.startswith('recovery-') and p.suffix=='.rpl' and valid_project(p) for p in recovery_dir.iterdir()), 'dark recovery manager has a valid checkpoint entry'
            capture_window(window,'recovery-manager-populated-dark')
            ctrl_shift('m')
            key('Escape');assert process.wait(timeout=20)==0
            preferences=json.loads(prefs.read_text());preferences['high_contrast']=True;prefs.write_text(json.dumps(preferences))
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'high-contrast Home');focus(display,window,2,0);send();time.sleep(0.5)
            capture_window(window,'home-dark-high-contrast')
            key('Escape');assert process.wait(timeout=20)==0
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                required={'home-light-normal','home-light-narrow','home-recent-artwork-light','new-artwork-light-normal','new-artwork-light-narrow','reference-board-light','brush-library-light-normal','brush-library-light-narrow','layers-panel-populated-light','recovery-manager-populated-light','home-dark-normal','home-dark-narrow','new-artwork-dark-normal','reference-board-dark','brush-library-dark','layers-panel-populated-dark','recovery-manager-populated-dark','home-dark-high-contrast'}
                missing=required-set(captured_screenshots)
                assert not missing,f'screenshot suite missed required states: {sorted(missing)}'
                for name,(width,height) in captured_screenshots.items():
                    if name.endswith('narrow'):
                        assert width<=700 and height<=520,f'{name} was not captured at a narrow size: {width}x{height}'
                    if name.endswith('normal'):
                        assert width>=900 and height>=650,f'{name} was not captured at a normal size: {width}x{height}'
            print('ALPHA_WORKFLOW_SMOKE: PASSED (real mouse raster paint, Home thumbnail after restart, persistent theme/reference board, reference image excluded from artwork export, save checksum, undo/redo, previous-file backup, PNG export, reopen, brush preset save/load, rectangle selection/fill/undo, zoom/fit, layered OpenRaster export/import, journal-durable process-kill recovery without paired snapshot, source artwork preservation, clean close)')
        finally:
            if process.poll() is None: process.terminate();process.wait(timeout=10)
            close_display(display)
            # Keep runner diagnostics bounded and leave room for Python's
            # traceback. The full app log remains in the uploaded diagnostics
            # artifact when the workflow reaches its always-run upload step.
            lines=(directory/'application.log').read_text().splitlines()
            relevant=[line for line in lines if line.startswith(('UI_TEST_','TEST_PATH_DIALOG:','Canvas adapter:'))]
            print('\n'.join(relevant[-60:]))
