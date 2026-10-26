#!/usr/bin/env python3
"""Linux/Xvfb integration test: inject real XTest mouse/key events into winit.

Uses only Python stdlib plus the platform's X11/XTest libraries. No mock app input
path, screenshots, user input recording, or additional Python packages.
"""
import ctypes as c
from pathlib import Path
import os, tempfile, hashlib, struct, json, zipfile, zlib, binascii, shutil
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
        return next((children[i] for i in range(count.value) if 'BitchPixel' in title(children[i])), 0)
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

def brush_preview_rect():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_BRUSH_PREVIEW_BOUNDS: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def brush_preset_preview_count():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_BRUSH_PRESET_PREVIEWS: ')]
    return int(lines[-1].partition(': ')[2]) if lines else 0

def color_palette_marker(name):
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(name + ': ')]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].partition(': ')[2].split()))

def stable_marker(name):
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(name + ': ')]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return lines[-1].partition(': ')[2]

def event_marker(name):
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(name + ': ')]
    return lines[-1].partition(': ')[2] if lines else None

def assert_slogan_contrast(theme):
    prefix=f'UI_TEST_HOME_SLOGAN_CONTRAST: {theme} '
    line=next((line for line in reversed((directory/'application.log').read_text().splitlines())
               if line.startswith(prefix)),None)
    assert line is not None,f'{theme} Home did not report the approved slogan contrast'
    contrast=float(line.partition(prefix)[2])
    assert contrast>=4.5,f'{theme} Home slogan contrast is below 4.5:1 ({contrast:.2f})'

def workspace_brush_tab_rect():
    marker=stable_marker('UI_TEST_WORKSPACE_BRUSH_TAB_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_front_row_rect():
    marker=stable_marker('UI_TEST_LAYER_FRONT_ROW_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_front_row_id():
    marker=stable_marker('UI_TEST_LAYER_FRONT_ROW_RECT')
    return int(marker.split()[0]) if marker else None

def layer_actions_open_count():
    return sum(line.startswith('UI_TEST_LAYER_ACTIONS_OPENED: ')
               for line in (directory/'application.log').read_text().splitlines())

def layer_menu_action_rect(action):
    lines=(directory/'application.log').read_text().splitlines()
    start=max((i for i,line in enumerate(lines)
               if line.startswith('UI_TEST_LAYER_ACTIONS_OPENED: ')),default=-1)
    prefix=f'UI_TEST_LAYER_{action}_MENU_RECT: '
    markers=[line.partition(': ')[2] for line in lines[start+1:]
             if line.startswith(prefix)]
    if len(markers)<2 or markers[-1]!=markers[-2]:
        return None
    return tuple(map(float,markers[-1].split()))

def layer_duplicate_menu_rect():
    return layer_menu_action_rect('DUPLICATE')

def layer_more_button_rect():
    marker=stable_marker('UI_TEST_LAYER_MORE_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_context_duplicate_count():
    marker=event_marker('UI_TEST_LAYER_CONTEXT_DUPLICATED')
    return int(marker.split()[0]) if marker else None

def history_layer_count():
    marker=event_marker('UI_TEST_HISTORY_LAYER_COUNT')
    return int(marker) if marker else None

def layer_context_delete_count():
    marker=event_marker('UI_TEST_LAYER_CONTEXT_DELETED')
    return int(marker.split()[0]) if marker else None

def recovery_preview_target():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_RECOVERY_PREVIEW_TARGET: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def recovery_preview_image_rect():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_RECOVERY_PREVIEW_IMAGE_RECT: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def home_recent_card_rect():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_HOME_RECENT_CARD_GEOMETRY: ')]
    cards=[tuple(map(float,line.partition(': ')[2].split())) for line in lines[-8:]]
    if not cards:
        return None
    # A responsive grid can contain cards with different metadata wrapping.
    # The last card may be clipped at the viewport even while a taller card
    # earlier in the same row is already fully visible. Prefer the largest
    # complete card; scroll only if no complete card exists yet.
    complete=[bounds for bounds in cards if bounds[3]>=bounds[4]-1.0]
    return max(complete,key=lambda bounds:bounds[4]) if complete else cards[-1]

def top_palette_clearance():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_TOP_PALETTE_CLEARANCE: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def send():
    flush(display)
    time.sleep(0.06)

def trace(action, *details):
    if os.environ.get('RPL_VERBOSE_UI_TRACE'):
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

def click_at(px, py):
    move(round(px),round(py))
    click_button(1,True)
    click_button(1,False)

def right_click_at(px, py):
    move(round(px),round(py))
    click_button(3,True)
    click_button(3,False)

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
    reference_marker_pixels=0
    recent_artwork_pixels=0
    home_wide_bottom_dark_pixels=0
    recovery_preview_colors=set()
    brush_preview_colors=set()
    saved_swatch_pixels=0
    recovery_preview_bounds=None
    brush_preview_bounds=None
    saved_swatch_bounds=None
    saved_swatch_color=None
    if name == 'recovery-manager-populated-light':
        bounds=wait(recovery_preview_image_rect,'recovery preview image is laid out')
        x,y,preview_width,preview_height,scale,content_top=bounds
        recovery_preview_bounds=(x*scale,(y+content_top)*scale,(x+preview_width)*scale,(y+content_top+preview_height)*scale)
    if name == 'brush-library-light-normal':
        x,y,preview_width,preview_height,scale=wait(brush_preview_rect,'active raster-engine preview is laid out')
        brush_preview_bounds=(x*scale,y*scale,(x+preview_width)*scale,(y+preview_height)*scale)
    if name == 'color-controls-light':
        marker=wait(lambda: color_palette_marker('UI_TEST_COLOR_SWATCH_RECT'),'saved color swatch is laid out')
        x,y,swatch_width,swatch_height,scale,red,green,blue=marker
        saved_swatch_bounds=(x*scale,y*scale,(x+swatch_width)*scale,(y+swatch_height)*scale)
        saved_swatch_color=(int(red),int(green),int(blue))
    bytes_per_pixel=(bits_per_pixel+7)//8
    byte_order_name='little' if byte_order==0 else 'big'
    for y in range(height):
        rows.append(0)
        row=pixel_data[y*bytes_per_line:(y+1)*bytes_per_line]
        for x_pixel in range(width):
            start=x_pixel*bytes_per_pixel
            pixel=int.from_bytes(row[start:start+bytes_per_pixel],byte_order_name)
            red=component(pixel,masks[0]);green=component(pixel,masks[1]);blue=component(pixel,masks[2])
            if name.startswith('reference-board-') and (
                (red<80 and green>150 and blue<80)
                or (green>210 and green-red>50 and green-blue>50)
            ):
                reference_marker_pixels+=1
            if name == 'home-recent-artwork-light' and blue > 100 and blue-red > 45 and blue-green > 20:
                recent_artwork_pixels+=1
            if name == 'home-light-wide' and y >= height-96 and red < 90 and green < 100 and blue < 110:
                home_wide_bottom_dark_pixels+=1
            if recovery_preview_bounds is not None:
                left,top,right,bottom=recovery_preview_bounds
                if left<=x_pixel<right and top<=y<bottom:
                    recovery_preview_colors.add((red,green,blue))
            if brush_preview_bounds is not None:
                left,top,right,bottom=brush_preview_bounds
                if left<=x_pixel<right and top<=y<bottom:
                    brush_preview_colors.add((red,green,blue))
            if saved_swatch_bounds is not None:
                left,top,right,bottom=saved_swatch_bounds
                if left<=x_pixel<right and top<=y<bottom and (red,green,blue)==saved_swatch_color:
                    saved_swatch_pixels+=1
            rows.extend((red,green,blue,255))
    raw=b'\x89PNG\r\n\x1a\n'+png_chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,6,0,0,0))+png_chunk(b'IDAT',zlib.compress(bytes(rows),6))+png_chunk(b'IEND',b'')
    destination=Path(folder) if folder else None
    if destination:
        destination.mkdir(parents=True,exist_ok=True)
        # Retain a failed capture too, so blank/obscured-window assertions are
        # diagnosable from the same run rather than disappearing on exception.
        (destination/f'{name}.png').write_bytes(raw)
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
    if name.startswith('reference-board-'):
        assert reference_marker_pixels>=40, f'{name} did not visibly render the unique reference-image marker ({reference_marker_pixels} matching pixels)'
        print(f'UI_SCREENSHOT_REFERENCE_PIXELS: {name} {reference_marker_pixels}', flush=True)
    if name == 'home-recent-artwork-light':
        card_bounds=wait(home_recent_card_rect,'recent-artwork card geometry is available')
        _left,_top,_width,visible_height,total_height=card_bounds
        assert visible_height>=total_height-1.0, f'{name} crops its visible recent-artwork card ({visible_height:.1f}/{total_height:.1f} points)'
        assert recent_artwork_pixels>=30, f'{name} does not visibly contain the painted blue stroke from the real recent-project thumbnail ({recent_artwork_pixels} matching pixels)'
        print(f'UI_SCREENSHOT_RECENT_ART_PIXELS: {name} {recent_artwork_pixels}', flush=True)
        print(f'UI_SCREENSHOT_RECENT_CARD: {name} visible={visible_height:.1f} total={total_height:.1f}', flush=True)
    if name == 'home-light-wide':
        assert home_wide_bottom_dark_pixels<=width*96//20, f'{name} leaves the canvas exposed beneath Home ({home_wide_bottom_dark_pixels} dark canvas pixels in the final 96 rows)'
        print(f'UI_SCREENSHOT_HOME_FULL_VIEWPORT: {name} dark-bottom-pixels={home_wide_bottom_dark_pixels}', flush=True)
    if name == 'recovery-manager-populated-light':
        assert len(recovery_preview_colors)>=8, f'{name} does not show a rendered artwork preview ({len(recovery_preview_colors)} visible colors)'
        print(f'UI_SCREENSHOT_RECOVERY_PREVIEW_COLORS: {name} {len(recovery_preview_colors)}', flush=True)
    if name == 'brush-library-light-normal':
        assert len(brush_preview_colors)>=6, f'{name} does not visibly render the active raster brush sample ({len(brush_preview_colors)} visible colors)'
        print(f'UI_SCREENSHOT_BRUSH_SAMPLE_COLORS: {name} {len(brush_preview_colors)}', flush=True)
    if name == 'color-controls-light':
        assert saved_swatch_pixels>=8, f'{name} does not visibly render the populated saved color palette ({saved_swatch_pixels} matching pixels)'
        print(f'UI_SCREENSHOT_SAVED_SWATCH_PIXELS: {name} {saved_swatch_pixels}', flush=True)
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
            elif x_pixel < 32 and y < 32:
                # A large, unique green corner makes screenshots prove that the
                # imported reference itself is visible, not merely the canvas.
                color=(0,255,0,255)
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
    initial_swatches=[[245,88,206],[116,88,246],[60,205,180]]
    prefs.write_text(json.dumps({'version':1,'dark_theme':False,'brush_panel_width':280.0,'saved_color_swatches':initial_swatches,'references':[{'path':str(reference),'position':[620.0,100.0],'size':[320.0,260.0],'opacity':0.7}]}))
    with (directory/'application.log').open('w') as log:
        process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            window=wait(find_window,'alpha workspace')
            focus(display,window,2,0);send()
            # Let font/layout requests settle before testing drawing ownership.
            time.sleep(0.5)
            resize_window(window,1280,800)
            wait(lambda: 'UI_TEST_HOME_HEADLINE_CONTRAST: light' in (directory / 'application.log').read_text(),
                 'light Home headline is rendered at accessible contrast')
            assert_slogan_contrast('light')
            wait(lambda: 'UI_TEST_HOME_QUICK_START_VISIBLE' in (directory / 'application.log').read_text(),
                 'first-run Home uses its hero for actual quick-start canvas actions')
            capture_window(window,'home-light-normal')
            resize_window(window,1920,1080)
            capture_window(window,'home-light-wide')
            resize_window(window,640,480)
            capture_window(window,'home-light-narrow')
            resize_window(window,1280,800)
            ctrl('n');new_document_prompt();time.sleep(0.25)
            wait(lambda: (directory / 'application.log').read_text().count('UI_TEST_NEW_ARTWORK_VISIBLE') >= 1,
                 'light New Artwork interface has rendered before capture')
            capture_window(window,'new-artwork-light-normal')
            resize_window(window,640,480)
            capture_window(window,'new-artwork-light-narrow')
            resize_window(window,1280,800)
            key('Return');time.sleep(0.5)
            capture_window(window,'canvas-workspace-light')
            wait(lambda: 'UI_TEST_REFERENCE_IMAGE_VISIBLE:' in (directory / 'application.log').read_text(),
                 'reference screenshot uses a rendered image texture')
            time.sleep(0.35)  # the marker is emitted before egui submits this frame
            capture_window(window,'reference-board-light')
            left, top, width, height, scale = wait(brush_rect, 'Brush control geometry')
            move(round((left + width / 2) * scale), round((top + height / 2) * scale))
            click_button(1, True)
            click_button(1, False)
            wait(lambda: 'UI_TEST_BRUSH_FLYOUT_OPENED' in (directory / 'application.log').read_text(),
                 'palette Brush control opens its flyout')
            time.sleep(0.25)
            preview_bounds=wait(brush_preview_rect,'real raster-engine preview is present in the Brush Library')
            assert preview_bounds[2]>=120 and preview_bounds[3]>=36, f'brush preview is too small: {preview_bounds}'
            preset_preview_count=wait(brush_preset_preview_count,'raster-engine samples are rendered for built-in brush presets')
            assert preset_preview_count==10, f'expected ten rendered brush samples, got {preset_preview_count}'
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
            key('F6')
            wait(lambda: stable_marker('UI_TEST_WORKSPACE_CONTROLS_RECT'),
                 'the detached workspace controls open from their keyboard shortcut')
            wait(lambda: stable_marker('UI_TEST_WORKSPACE_CONTROLS_ACTIVE') == 'Layers',
                 'floating workspace controls show the current Layers tab')
            capture_window(window,'workspace-controls-floating-light-normal')
            tab_bounds=wait(workspace_brush_tab_rect,'floating Brush settings tab is laid out')
            click_at((tab_bounds[0]+tab_bounds[2]/2)*tab_bounds[4],
                     (tab_bounds[1]+tab_bounds[3]/2)*tab_bounds[4])
            wait(lambda: stable_marker('UI_TEST_WORKSPACE_CONTROLS_ACTIVE') == 'Brush',
                 'floating workspace controls switch to Brush settings')
            workspace_save_count=(directory/'application.log').read_text().count('UI_TEST_WORKSPACE_SAVE:')
            key('Escape')
            assert process.poll() is None, 'Escape closed the application instead of dismissing workspace controls'
            wait(lambda: 'UI_TEST_ESCAPE_DISMISSED: workspace controls' in (directory/'application.log').read_text(),
                 'Escape dismisses floating workspace controls')
            wait(lambda: (directory/'application.log').read_text().count('UI_TEST_WORKSPACE_SAVE:')>workspace_save_count,
                 'floating workspace close state is written to preferences')
            preferences=json.loads(prefs.read_text())
            assert preferences['show_controls'] is False, 'closing the floating workspace controls was not persisted'
            controls_position=preferences.get('workspace_controls_position')
            assert controls_position and all(isinstance(value,(int,float)) for value in controls_position), 'floating workspace position was not persisted'
            controls_size=preferences.get('workspace_controls_size')
            assert controls_size and all(200<=value<=1400 for value in controls_size), 'floating workspace size was not persisted within safe bounds'
            key('c')
            wait(lambda: 'UI_TEST_COLOR_FLYOUT_OPENED: shortcut' in (directory/'application.log').read_text(),
                 'C opens Color controls for keyboard users')
            wait(lambda: 'UI_TEST_COLOR_PALETTE_READY: 3' in (directory/'application.log').read_text(),
                 'Color panel restores the three saved workspace swatches')
            time.sleep(0.25)
            capture_window(window,'color-controls-light')
            add_bounds=wait(lambda: color_palette_marker('UI_TEST_COLOR_PALETTE_ADD_RECT'), 'Save current color control is laid out')
            remove_bounds=wait(lambda: color_palette_marker('UI_TEST_COLOR_PALETTE_REMOVE_RECT'), 'saved swatch remove control is laid out')
            click_at((add_bounds[0]+add_bounds[2]/2)*add_bounds[4],(add_bounds[1]+add_bounds[3]/2)*add_bounds[4])
            wait(lambda: 'UI_TEST_COLOR_PALETTE_ADD_CLICKED' in (directory/'application.log').read_text(),
                 'clicking Save current color activates its actual UI control')
            wait(lambda: len(json.loads(prefs.read_text()).get('saved_color_swatches',[]))==4,
                 'Save current color persists a reusable workspace swatch')
            saved_after_add=json.loads(prefs.read_text())['saved_color_swatches']
            assert saved_after_add[-1]==[35,65,180], f'palette saved unexpected color: {saved_after_add[-1]}'
            swatch_bounds=wait(lambda: color_palette_marker('UI_TEST_COLOR_SWATCH_RECT'), 'first saved color action is laid out')
            click_at((swatch_bounds[0]+swatch_bounds[2]/2)*swatch_bounds[4],(swatch_bounds[1]+swatch_bounds[3]/2)*swatch_bounds[4])
            wait(lambda: [245,88,206] in json.loads(prefs.read_text()).get('recent_brush_colors',[]),
                 'selecting a saved swatch applies its color to the active brush')
            click_at((add_bounds[0]+add_bounds[2]/2)*add_bounds[4],(add_bounds[1]+add_bounds[3]/2)*add_bounds[4])
            time.sleep(0.2)
            assert len(json.loads(prefs.read_text())['saved_color_swatches'])==4, 'saving an existing swatch should not create a duplicate'
            click_at((remove_bounds[0]+remove_bounds[2]/2)*remove_bounds[4],(remove_bounds[1]+remove_bounds[3]/2)*remove_bounds[4])
            wait(lambda: len(json.loads(prefs.read_text()).get('saved_color_swatches',[]))==3,
                 'Remove saved color updates workspace preferences')
            saved_after_remove=json.loads(prefs.read_text())['saved_color_swatches']
            assert saved_after_remove==initial_swatches[1:]+[[35,65,180]], f'palette removal changed unrelated colors: {saved_after_remove}'
            key('Escape')
            wait(lambda: 'UI_TEST_ESCAPE_DISMISSED: color' in (directory/'application.log').read_text(),
                 'Escape dismisses Color controls')
            ctrl('l');time.sleep(0.3)
            window=wait(find_window,'window after Ctrl+L opens Layers')
            ctrl_shift('n');ctrl_shift('n');time.sleep(0.3)
            wait(lambda: 'UI_TEST_LAYERS_ADDED: 3' in (directory/'application.log').read_text(), 'Layers panel contains three real editable layers')
            capture_window(window,'layers-panel-populated-light')
            front_row=wait(layer_front_row_rect,'frontmost layer row offers its contextual actions')
            more=wait(layer_more_button_rect,'frontmost layer exposes a keyboard-accessible actions menu')
            opened=layer_actions_open_count()
            click_at((more[0]+more[2]/2)*more[4],(more[1]+more[3]/2)*more[4])
            wait(lambda: layer_actions_open_count()>opened,
                 'the frontmost layer Actions menu opens')
            duplicate_menu=wait(layer_duplicate_menu_rect,'layer context menu exposes Duplicate layer')
            click_at((duplicate_menu[0]+duplicate_menu[2]/2)*duplicate_menu[4],
                     (duplicate_menu[1]+duplicate_menu[3]/2)*duplicate_menu[4])
            wait(lambda: layer_context_duplicate_count()==4,
                 'layer context Duplicate targets the clicked layer and creates an editable copy')
            ctrl('z')
            wait(lambda: history_layer_count()==3,
                 'undo removes the context-menu duplicate without changing the other layers')
            front_id=layer_front_row_id()
            more=wait(layer_more_button_rect,'frontmost layer action button remains available after undo')
            opened=layer_actions_open_count()
            click_at((more[0]+more[2]/2)*more[4],(more[1]+more[3]/2)*more[4])
            wait(lambda: layer_actions_open_count()>opened,
                 'the frontmost layer Actions menu reopens for reordering')
            send_backward=wait(lambda: layer_menu_action_rect('SEND_BACKWARD'),
                               'Send backward is available for a non-bottom layer')
            click_at((send_backward[0]+send_backward[2]/2)*send_backward[4],
                     (send_backward[1]+send_backward[3]/2)*send_backward[4])
            wait(lambda: event_marker('UI_TEST_LAYER_CONTEXT_MOVED') is not None,
                 'Send backward records a targeted layer-order transaction')
            wait(lambda: layer_front_row_id()!=front_id,
                 'targeted Send backward changes the frontmost layer')
            ctrl('z')
            wait(lambda: layer_front_row_id()==front_id,
                 'undo restores the previous layer order')
            more=wait(layer_more_button_rect,'frontmost layer action button remains available after reorder undo')
            opened=layer_actions_open_count()
            click_at((more[0]+more[2]/2)*more[4],(more[1]+more[3]/2)*more[4])
            wait(lambda: layer_actions_open_count()>opened,
                 'the frontmost layer Actions menu reopens for deletion')
            delete=wait(lambda: layer_menu_action_rect('DELETE'),
                        'layer actions expose deletion for a non-empty stack')
            click_at((delete[0]+delete[2]/2)*delete[4],(delete[1]+delete[3]/2)*delete[4])
            wait(lambda: layer_context_delete_count()==2,
                 'targeted Delete removes the layer whose actions menu was opened')
            ctrl('z')
            wait(lambda: history_layer_count()==3 and layer_front_row_id()==front_id,
                 'undo restores the deleted layer and its order')
            right_click_at((front_row[1]+front_row[3]/2)*front_row[5],
                           (front_row[2]+front_row[4]/2)*front_row[5])
            wait(lambda: 'UI_TEST_LAYER_NATIVE_CONTEXT_OPENED' in (directory/'application.log').read_text(),
                 'right-clicking a layer name opens its native context menu')
            key('Escape')
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
            assert not any(pixels[i] < 80 and pixels[i+1] > 120 and pixels[i+2] < 100 for i in range(0,len(pixels),4)), 'workspace reference marker pixels leaked into the exported artwork'
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
            # Put an app-produced saved painting beside the live recovery entry
            # so the manager preview is visibly populated, while the later
            # forced-termination scenario continues to exercise the live journal.
            recovery_preview_fixture=recovery_dir/'recovery-visual-review.rpl'
            shutil.copyfile(project,recovery_preview_fixture)
            ctrl_shift('m');time.sleep(0.4)
            wait(lambda: 'UI_TEST_RECOVERY_LABEL: Visual review' in (directory/'application.log').read_text(),
                 'recovery manager displays a readable, artwork-backed preview fixture')
            light_recovery_contrast = next(line for line in (directory/'application.log').read_text().splitlines()
                if line.startswith('UI_TEST_RECOVERY_LABEL_CONTRAST: light '))
            assert float(light_recovery_contrast.rsplit(' ',1)[1]) >= 4.5, 'light recovery label contrast is below 4.5:1'
            preview_target=wait(recovery_preview_target,'recovery preview button geometry')
            left,top,width,height,scale,content_top=preview_target
            hover_marker='UI_TEST_RECOVERY_PREVIEW_HOVERED'
            application_log=directory/'application.log'
            click_x=round((left+width/2)*scale)
            click_y=None
            # Window-manager client offsets can differ from egui's viewport
            # origin. Probe a bounded vertical range and click only after the
            # real egui button reports hover, instead of assuming a decoration
            # offset from the screenshot dimensions.
            for offset in range(-80,81,4):
                candidate_y=round((top+height/2)*scale)+offset
                move(click_x,candidate_y)
                if hover_marker in application_log.read_text():
                    click_y=candidate_y
                    break
            assert click_y is not None, 'pointer movement did not reach the recovery preview button'
            print(f'UI_TEST_RECOVERY_PREVIEW_HIT: {click_x} {click_y}', flush=True)
            click_button(1,True);click_button(1,False)
            wait(lambda: 'UI_TEST_RECOVERY_PREVIEW_VISIBLE' in (directory/'application.log').read_text(),
                 'recovery manager renders the requested native artwork preview')
            time.sleep(0.25)
            capture_window(window,'recovery-manager-populated-light');ctrl_shift('m')
            recovery_preview_fixture.unlink(missing_ok=True)
            recovery_preview_fixture.with_suffix('.owner').unlink(missing_ok=True)
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
            wait(lambda: 'UI_TEST_HOME_RECENT_PREVIEW_VISIBLE' in (directory / 'application.log').read_text(),
                 'recent Home screenshot renders a real cached project thumbnail')
            resize_window(window,1920,1080)
            card_bounds=wait(home_recent_card_rect,'recent-artwork card layout at wide size')
            move(1500,900)
            for _ in range(6):
                if card_bounds[3]>=card_bounds[4]-1.0:
                    break
                assert button(display,5,1,0);assert button(display,5,0,0);send()
                time.sleep(0.2)
                card_bounds=home_recent_card_rect() or card_bounds
            assert card_bounds[3]>=card_bounds[4]-1.0, 'scrolling Home did not bring the complete recent-artwork card into view'
            capture_window(window,'home-recent-artwork-light')
            key('Escape');assert process.wait(timeout=20)==0
            home_preferences=json.loads(prefs.read_text())
            assert any(item['path']==str(reference) for item in home_preferences.get('references',[])), 'reference board path survives an app restart through Home'
            # Exercise the same application windows under dark mode and high
            # contrast. The dark workspace uses the persisted top dock to cover
            # the horizontal toolbar layout. Seed Recovery with a valid .rpl.
            preferences=json.loads(prefs.read_text())
            preferences['appearance_mode']='dark';preferences['dark_theme']=True;preferences['high_contrast']=False
            preferences['palette_dock']='Top'
            for reference_layout in preferences.get('references',[]):
                reference_layout['position']=[40.0,190.0]
                reference_layout['size']=[360.0,300.0]
            prefs.write_text(json.dumps(preferences))
            recovery_dir.mkdir(parents=True,exist_ok=True)
            visual_checkpoint=recovery_dir/'recovery-visual-checkpoint.rpl'
            visual_checkpoint.write_bytes(recovered.read_bytes())
            assert valid_project(visual_checkpoint),'dark-theme recovery fixture is a valid native project'
            reference_markers_before_dark= (directory / 'application.log').read_text().count('UI_TEST_REFERENCE_IMAGE_VISIBLE:')
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'dark Home after saved artwork restart');focus(display,window,2,0);send();time.sleep(0.5)
            wait(lambda: 'UI_TEST_HOME_HEADLINE_CONTRAST: dark' in (directory / 'application.log').read_text(),
                 'dark Home headline is rendered at accessible contrast')
            assert_slogan_contrast('dark')
            resize_window(window,1280,800);capture_window(window,'home-dark-normal')
            resize_window(window,640,480);capture_window(window,'home-dark-narrow')
            resize_window(window,1280,800)
            ctrl('n');new_document_prompt()
            wait(lambda: (directory / 'application.log').read_text().count('UI_TEST_NEW_ARTWORK_VISIBLE') >= 2,
                 'dark New Artwork interface has rendered before capture')
            time.sleep(0.25);capture_window(window,'new-artwork-dark-normal');key('Return');time.sleep(0.4)
            capture_window(window,'canvas-workspace-dark-toolbar-top')
            clearance=wait(top_palette_clearance,'top-docked palette reports its measured app-toolbar clearance')
            assert clearance[0]>=clearance[1]-0.5, f'top palette overlaps contextual options: {clearance}'
            wait(lambda: (directory / 'application.log').read_text().count('UI_TEST_REFERENCE_IMAGE_VISIBLE:') > reference_markers_before_dark,
                 'dark reference screenshot renders its imported image')
            time.sleep(0.35)  # wait until the marked reference frame is presented
            capture_window(window,'reference-board-dark')
            ctrl('b');time.sleep(0.35);capture_window(window,'brush-library-dark')
            key('Escape');key('c');time.sleep(0.35);capture_window(window,'color-controls-dark')
            key('Escape');ctrl('l');time.sleep(0.35);capture_window(window,'layers-panel-populated-dark')
            ctrl('l');time.sleep(0.2)
            ctrl_shift('m');time.sleep(0.35)
            assert any(p.name.startswith('recovery-') and p.suffix=='.rpl' and valid_project(p) for p in recovery_dir.iterdir()), 'dark recovery manager has a valid checkpoint entry'
            wait(lambda: 'UI_TEST_RECOVERY_LABEL_CONTRAST: dark ' in (directory/'application.log').read_text(),
                 'dark recovery entry uses readable text contrast')
            dark_recovery_contrast = next(line for line in reversed((directory/'application.log').read_text().splitlines())
                if line.startswith('UI_TEST_RECOVERY_LABEL_CONTRAST: dark '))
            assert float(dark_recovery_contrast.rsplit(' ',1)[1]) >= 4.5, 'dark recovery label contrast is below 4.5:1'
            capture_window(window,'recovery-manager-populated-dark')
            ctrl_shift('m')
            # The preceding New Artwork capture creates a fresh unsaved
            # document. Escape correctly enters the close-save path, so end
            # this screenshot-only process explicitly before reopening Home.
            process.terminate();assert process.wait(timeout=10) is not None
            preferences=json.loads(prefs.read_text());preferences['high_contrast']=True;prefs.write_text(json.dumps(preferences))
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'high-contrast Home');focus(display,window,2,0);send();time.sleep(0.5)
            capture_window(window,'home-dark-high-contrast')
            key('Escape');assert process.wait(timeout=20)==0
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                required={'home-light-normal','home-light-wide','home-light-narrow','home-recent-artwork-light','new-artwork-light-normal','new-artwork-light-narrow','reference-board-light','brush-library-light-normal','brush-library-light-narrow','color-controls-light','layers-panel-populated-light','recovery-manager-populated-light','workspace-controls-floating-light-normal','home-dark-normal','home-dark-narrow','new-artwork-dark-normal','canvas-workspace-dark-toolbar-top','reference-board-dark','brush-library-dark','color-controls-dark','layers-panel-populated-dark','recovery-manager-populated-dark','home-dark-high-contrast'}
                missing=required-set(captured_screenshots)
                assert not missing,f'screenshot suite missed required states: {sorted(missing)}'
                for name,(width,height) in captured_screenshots.items():
                    if name.endswith('narrow'):
                        assert width<=700 and height<=520,f'{name} was not captured at a narrow size: {width}x{height}'
                    if name.endswith('normal'):
                        assert width>=900 and height>=650,f'{name} was not captured at a normal size: {width}x{height}'
                    if name.endswith('wide'):
                        assert width>=1800 and height>=1000,f'{name} was not captured at a wide size: {width}x{height}'
                    if name == 'home-recent-artwork-light':
                        assert width>=1800 and height>=1000, f'{name} must show the featured real artwork at wide size: {width}x{height}'
            print('ALPHA_WORKFLOW_SMOKE: PASSED (real mouse raster paint, Home thumbnail after restart, persistent theme/reference board, floating workspace panel/tab interaction, saved-color add/apply/deduplicate/remove, targeted layer duplicate/reorder/delete with undo, native right-click layer menu, reference image excluded from artwork export, save checksum, undo/redo, previous-file backup, PNG export, reopen, brush preset save/load, rectangle selection/fill/undo, zoom/fit, layered OpenRaster export/import, journal-durable process-kill recovery without paired snapshot, source artwork preservation, clean close)')
        except Exception as error:
            print(f'ALPHA_WORKFLOW_FAILURE: {type(error).__name__}: {error}', flush=True)
            raise
        finally:
            if process.poll() is None: process.terminate();process.wait(timeout=10)
            close_display(display)
            # Keep runner diagnostics bounded and leave room for Python's
            # traceback. The full app log remains in the uploaded diagnostics
            # artifact when the workflow reaches its always-run upload step.
            lines=(directory/'application.log').read_text().splitlines()
            relevant=[line for line in lines if line.startswith(('UI_TEST_','TEST_PATH_DIALOG:','Canvas adapter:'))]
            print('\n'.join(relevant[-60:]))
