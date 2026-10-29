#!/usr/bin/env python3
"""Linux/Xvfb integration test: inject real XTest mouse/key events into winit.

Uses only Python stdlib plus the platform's X11/XTest libraries. No mock app input
path, screenshots, user input recording, or additional Python packages.
"""
import ctypes as c
from pathlib import Path
import os, tempfile, hashlib, struct, json, zipfile, zlib, binascii, shutil, math
import subprocess
import time

if os.environ.get('RPL_KEEP_ALPHA_TEMP'):
    _TemporaryDirectory=tempfile.TemporaryDirectory
    class PreserveAlphaDirectory(_TemporaryDirectory):
        def cleanup(self):
            if self._finalizer.detach():
                print(f'ALPHA_WORKFLOW_TEMP_PRESERVED: {self.name}', flush=True)
    tempfile.TemporaryDirectory=PreserveAlphaDirectory

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
move_resize = api(x, 'XMoveResizeWindow', I, D, W, I, I, U, U)
get_geometry = api(x, 'XGetGeometry', I, D, W, c.POINTER(W), c.POINTER(I), c.POINTER(I),
                   c.POINTER(U), c.POINTER(U), c.POINTER(U), c.POINTER(U))
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
    marker=os.environ.get('RPL_UI_TEST_WINDOW_TOKEN','RPL_UI_TEST_WINDOW')
    try:
        return next((children[i] for i in range(count.value)
                     if marker in title(children[i])), 0)
    finally:
        if children:
            free(children)

def wait(predicate, description, seconds=15):
    deadline = time.monotonic() + seconds
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

def quick_brush_preset_rect(kind):
    prefix=f'UI_TEST_QUICK_PRESET_RECT: {kind} '
    lines=[line.partition(prefix)[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].split()[:5]))

def select_quick_brush(kind):
    menu=wait(lambda: stable_window_rect_marker('UI_TEST_QUICK_BRUSH_MENU_RECT'),
              'canvas options expose the current brush preset picker')
    click_at((menu[0]+menu[2]/2)*menu[4],(menu[1]+menu[3]/2)*menu[4])
    preset=wait(lambda: quick_brush_preset_rect(kind),
                f'quick preset menu exposes {kind}')
    click_at((preset[0]+preset[2]/2)*preset[4],(preset[1]+preset[3]/2)*preset[4])
    wait(lambda: event_marker('UI_TEST_QUICK_BRUSH_SELECTED')==kind,
         f'{kind} preset applies its real brush settings')

def brush_narrow_layout():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_BRUSH_NARROW_LAYOUT: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def brush_narrow_visible_card():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_BRUSH_NARROW_CARD_VISIBLE: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def brush_narrow_visible_card_count():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_BRUSH_NARROW_VISIBLE_CARDS: ')]
    return int(lines[-1].partition(': ')[2]) if lines else None

def brush_narrow_visible_cards_ready():
    count=brush_narrow_visible_card_count()
    return count if count is not None and count>=2 else None

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

def hue_wheel_rect():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_HUE_WHEEL_RECT: ')]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].partition(': ')[2].split()))

def original_saved_color_rect():
    return color_palette_marker('UI_TEST_COLOR_ORIGINAL_SWATCH_RECT')

def current_brush_rgb():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_COLOR_CURRENT_RGB: ')]
    return tuple(map(int,lines[-1].partition(': ')[2].split())) if lines else None

def wheel_point(hue, bounds):
    x,y,width,height,scale=bounds
    radius=(min(width,height)*0.5-5.0)*0.84
    angle=hue*math.tau-math.pi/2
    return ((x+width/2+radius*math.cos(angle))*scale,
            (y+height/2+radius*math.sin(angle))*scale)

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

def event_marker_count(name):
    return sum(line.startswith(name + ': ')
               for line in (directory/'application.log').read_text().splitlines())

def selection_transform_handles():
    lines=[line.partition(': ')[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_SELECTION_TRANSFORM_HANDLES: ')]
    return tuple(map(float,lines[-1].split())) if lines else None

def gradient_control_rect(mode):
    return stable_window_rect_marker(f'UI_TEST_GRADIENT_{mode.upper()}_RECT')

def vector_delete_rect():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_VECTOR_DELETE_RECT: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def vector_width_rect():
    marker=stable_window_rect_marker('UI_TEST_VECTOR_WIDTH_RECT')
    return tuple(marker) if marker else None

def vector_node_handle():
    marker=event_marker('UI_TEST_VECTOR_NODE_HANDLE')
    return tuple(map(float,marker.split())) if marker else None

def vector_anchor_controls_rect():
    return stable_window_rect_marker('UI_TEST_VECTOR_ANCHOR_CONTROLS_RECT')

def vector_anchor_action_rect(action):
    return stable_window_rect_marker(f'UI_TEST_VECTOR_ANCHOR_{action.upper()}_RECT')

def vector_curve_segment_rect():
    return stable_window_rect_marker('UI_TEST_VECTOR_CURVE_SEGMENT_RECT')

def vector_handles_section_rect():
    return stable_window_rect_marker('UI_TEST_VECTOR_HANDLES_SECTION_RECT')

def vector_handle_selector_rect(handle):
    prefix=f'UI_TEST_VECTOR_HANDLE_SELECTOR_RECT: {handle} '
    lines=[line.partition(prefix)[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if not lines:
        return None
    return tuple(map(float,lines[-1].split()[:5]))

def vector_control_handle(handle, anchor=0):
    prefix='UI_TEST_VECTOR_CONTROL_HANDLE: '
    for line in reversed((directory/'application.log').read_text().splitlines()):
        if line.startswith(prefix):
            values=line.partition(prefix)[2].split()
            if len(values)==6 and values[1]==str(anchor) and values[2]==handle:
                return tuple(map(float,values[3:]))
    return None

def click_scaled_rect(rect):
    x,y,width,height,scale=rect
    click_at((x+width/2)*scale,(y+height/2)*scale)

def symmetry_mode_rect(label):
    prefix=f'UI_TEST_SYMMETRY_MODE_RECT: {label} '
    lines=[line[len(prefix):] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].split()[:5]))

def symmetry_origin_button_rect():
    return stable_window_rect_marker('UI_TEST_SYMMETRY_ORIGIN_BUTTON_RECT')

def symmetry_canvas_bounds():
    marker=stable_marker('UI_TEST_SYMMETRY_CANVAS')
    return tuple(map(float,marker.split())) if marker else None

def screenshot_pixel_difference(left, right):
    width,height,a=decode_rgba_png(left)
    other_width,other_height,b=decode_rgba_png(right)
    assert (width,height)==(other_width,other_height)
    return sum(a[offset:offset+3]!=b[offset:offset+3]
               for offset in range(0,len(a),4))

def screenshot_region_difference(left, right, region):
    width,height,a=decode_rgba_png(left)
    other_width,other_height,b=decode_rgba_png(right)
    assert (width,height)==(other_width,other_height)
    x1,y1,x2,y2=map(int,region)
    x1=max(0,min(width,x1));x2=max(x1,min(width,x2))
    y1=max(0,min(height,y1));y2=max(y1,min(height,y2))
    return sum(a[(y*width+x)*4:(y*width+x)*4+3]!=
               b[(y*width+x)*4:(y*width+x)*4+3]
               for y in range(y1,y2) for x in range(x1,x2))

def palette_bounds():
    return window_rect_marker('UI_TEST_PALETTE_RECT')

def palette_chrome_clearance():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_PALETTE_CHROME_CLEARANCE: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def home_content_bounds():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_HOME_CONTENT_BOUNDS: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def window_rect_marker(name):
    prefix=name + ': '
    lines=[line.partition(': ')[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    return tuple(map(float,lines[-1].split()[:5])) if lines else None

def window_size(window):
    root_return=W();x_return=I();y_return=I()
    width_return=U();height_return=U();border_return=U();depth_return=U()
    if not get_geometry(display,window,c.byref(root_return),c.byref(x_return),c.byref(y_return),
                        c.byref(width_return),c.byref(height_return),c.byref(border_return),
                        c.byref(depth_return)):
        return None
    return width_return.value,height_return.value

def home_preset_card_rect(name='Portrait'):
    prefix=f'UI_TEST_HOME_PRESET_CARD_RECT|{name}|'
    lines=[line[len(prefix):] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    return tuple(map(float,lines[-1].split()[:5])) if lines else None

def settled_home_preset_card_rect(name='Portrait'):
    first=home_preset_card_rect(name)
    if first is None: return None
    time.sleep(0.2)
    return first if first==home_preset_card_rect(name) else None

def assert_new_document_name_and_create_visible(width, height):
    name=wait(lambda: window_rect_marker('UI_TEST_NEW_DOCUMENT_NAME_RECT'),
              'New Artwork has a visible, top-positioned document-name field')
    create=wait(lambda: window_rect_marker('UI_TEST_NEW_DOCUMENT_CREATE_RECT'),
                'New Artwork exposes its primary Create action')
    first_card=wait(lambda: stable_format_card_rect('Portrait'),
                    'New Artwork has a visually illustrated Portrait preset')
    name_x,name_y,name_w,name_h,scale=name
    create_x,create_y,create_w,create_h,create_scale=create
    assert name_y < first_card[1], \
        f'Document name must appear before the format choices: field={name}, card={first_card}'
    assert name_x >= -1 and name_y >= -1 and name_x+name_w <= width/scale+1 \
        and name_y+name_h <= height/scale+1, \
        f'Document name is clipped in {width}x{height}: {name}'
    if width >= 1200 and height >= 700:
        assert create_scale == scale and create_x >= -1 and create_y >= -1 \
            and create_x+create_w <= width/scale+1 and create_y+create_h <= height/scale-2, \
            f'Primary Create action should fit without scrolling at {width}x{height}: {create}'
    return name, create

def stable_window_rect_marker(name):
    prefix=name + ': '
    lines=[line.partition(': ')[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].split()[:5]))

def settled_window_rect_marker(name):
    # Panels can reflow for a frame after their visibility changes. Avoid
    # clicking the previous frame's valid but now stale geometry.
    first=window_rect_marker(name)
    if first is None: return None
    time.sleep(0.2)
    return first if first==window_rect_marker(name) else None

def stable_format_preview_rect(label):
    prefix=f'UI_TEST_FORMAT_PREVIEW_RECT|{label}|'
    lines=[line[len(prefix):] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].split()[:5]))

def stable_format_card_rect(label):
    prefix=f'UI_TEST_FORMAT_CARD_RECT|{label}|'
    lines=[line[len(prefix):] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].split()[:5]))

def assert_format_card_fully_visible(label, width, height):
    rect=wait(lambda: stable_format_card_rect(label),
              f'{label} format card has stable bounds')
    assert rect[0] >= 0.0 and rect[1] >= 0.0 \
        and rect[0]+rect[2] <= width/rect[4]+1.0 \
        and rect[1]+rect[3] <= height/rect[4]-4.0, \
        f'{label} format card is clipped in {width}x{height}: {rect}'
    return rect

def reference_image_rect():
    return stable_window_rect_marker('UI_TEST_REFERENCE_LAST_IMAGE_RECT')

def rects_overlap(left, right):
    return left[0] < right[0] + right[2] and right[0] < left[0] + left[2] \
        and left[1] < right[1] + right[3] and right[1] < left[1] + left[3]

def assert_slogan_contrast(theme):
    prefix=f'UI_TEST_HOME_SLOGAN_CONTRAST: {theme} '
    line=next((line for line in reversed((directory/'application.log').read_text().splitlines())
               if line.startswith(prefix)),None)
    assert line is not None,f'{theme} Home did not report the approved slogan contrast'
    contrast=float(line.partition(prefix)[2])
    assert contrast>=4.5,f'{theme} Home slogan contrast is below 4.5:1 ({contrast:.2f})'

def assert_strong_text_contrast(theme):
    prefix=f'UI_TEST_STRONG_TEXT_CONTRAST: {theme} '
    line=next((line for line in reversed((directory/'application.log').read_text().splitlines())
               if line.startswith(prefix)),None)
    assert line is not None,f'{theme} theme did not report its strong-text foreground contrast'
    contrast=float(line.partition(prefix)[2])
    assert contrast>=4.5,f'{theme} strong labels are below 4.5:1 ({contrast:.2f})'

def workspace_brush_tab_rect():
    marker=stable_marker('UI_TEST_WORKSPACE_BRUSH_TAB_RECT')
    return tuple(map(float,marker.split())) if marker else None

def workspace_open_layers_rect():
    marker=stable_marker('UI_TEST_WORKSPACE_OPEN_LAYERS_RECT')
    return tuple(map(float,marker.split())) if marker else None

def workspace_open_brush_rect():
    marker=stable_marker('UI_TEST_WORKSPACE_OPEN_BRUSH_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_front_row_rect():
    marker=stable_marker('UI_TEST_LAYER_FRONT_ROW_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_front_row_id():
    marker=stable_marker('UI_TEST_LAYER_FRONT_ROW_RECT')
    return int(marker.split()[0]) if marker else None

def layer_thumbnail_update(layer_id):
    prefix='UI_TEST_LAYER_THUMBNAIL_UPDATED: '
    for line in reversed((directory/'application.log').read_text().splitlines()):
        if line.startswith(prefix):
            values=line.partition(': ')[2].split()
            if int(values[0])==layer_id:
                return int(values[1],16),int(values[2]),int(values[3])
    return None

def layer_thumbnail_bounds():
    marker=stable_marker('UI_TEST_LAYER_THUMBNAIL_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_visibility_button_rect():
    marker=stable_marker('UI_TEST_LAYER_VISIBILITY_BUTTON_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_lock_button_rect():
    marker=stable_marker('UI_TEST_LAYER_LOCK_BUTTON_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_row_rect(row_index):
    prefix='UI_TEST_LAYER_ROW_RECT: '
    rows=[line.partition(': ')[2] for line in (directory/'application.log').read_text().splitlines()
          if line.startswith(prefix) and line.partition(': ')[2].split()[0]==str(row_index)]
    if len(rows)<2 or rows[-1]!=rows[-2]:
        return None
    return tuple(map(float,rows[-1].split()))

def reference_layout_rects():
    lines=[line.partition(': ')[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_REFERENCE_LAYOUT_RECT: ')]
    by_index={}
    for line in lines:
        parts=line.split()
        index=int(parts[0])
        by_index.setdefault(index,[]).append(tuple(map(float,parts[1:6])))
    if not by_index or any(len(values)<2 or values[-1]!=values[-2]
                           for values in by_index.values()):
        return None
    return [by_index[index][-1] for index in sorted(by_index)]

def reference_panel_count():
    marker=event_marker('UI_TEST_REFERENCE_PANEL_COUNT')
    return int(marker) if marker is not None else None

def reference_last_window_rect():
    marker=stable_marker('UI_TEST_REFERENCE_LAST_WINDOW_RECT')
    return tuple(map(float,marker.split())) if marker else None

def reference_window_rect(index):
    prefix=f'UI_TEST_REFERENCE_LAYOUT_RECT: {index} '
    lines=[line.partition(prefix)[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if not lines: return None
    values=tuple(map(float,lines[-1].split()))
    return values if len(values)==5 else None

def layers_panel_open():
    marker=event_marker('UI_TEST_LAYERS_PANEL_STATE')
    return marker=='true' if marker is not None else None

def compact_reference_active():
    prefix='UI_TEST_COMPACT_REFERENCE_ACTIVE: '
    lines=[line.partition(prefix)[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(int,lines[-1].split()))

def compact_reference_navigation():
    prefix='UI_TEST_REFERENCE_COMPACT_NAV: '
    lines=[line.partition(prefix)[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    values=lines[-1].split()
    return int(values[0]),int(values[1]),*map(float,values[2:])

def compact_reference_window_rect():
    active=compact_reference_active()
    if not active:
        return None
    index=active[0]
    prefix=f'UI_TEST_REFERENCE_LAYOUT_RECT: {index} '
    lines=[line.partition(prefix)[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].split()))

def compact_reference_image_rect():
    active=compact_reference_active()
    if not active:
        return None
    index=active[0]
    prefix=f'UI_TEST_REFERENCE_COMPACT_IMAGE_RECT: {index} '
    lines=[line.partition(prefix)[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].split()))

def layers_window_rect():
    return stable_window_rect_marker('UI_TEST_LAYERS_WINDOW_RECT')

def layer_row_id(row_index):
    marker=layer_row_rect(row_index)
    return int(marker[1]) if marker else None

def layer_drag_handle_rect(row_index):
    prefix='UI_TEST_LAYER_DRAG_HANDLE_RECT: '
    rows=[line.partition(': ')[2] for line in (directory/'application.log').read_text().splitlines()
          if line.startswith(prefix) and line.partition(': ')[2].split()[0]==str(row_index)]
    if len(rows)<2 or rows[-1]!=rows[-2]:
        return None
    return tuple(map(float,rows[-1].split()))

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

def layer_copy_selection_menu_rect():
    return layer_menu_action_rect('COPY_SELECTION')

def layer_move_selection_menu_rect():
    return layer_menu_action_rect('MOVE_SELECTION')

def layer_mask_add_rect():
    marker=stable_marker('UI_TEST_LAYER_MASK_ADD_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_mask_edit_rect():
    marker=stable_marker('UI_TEST_LAYER_MASK_EDIT_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_mask_enabled_rect():
    marker=stable_marker('UI_TEST_LAYER_MASK_ENABLED_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_mask_invert_rect():
    marker=stable_marker('UI_TEST_LAYER_MASK_INVERT_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_mask_clear_rect():
    marker=stable_marker('UI_TEST_LAYER_MASK_CLEAR_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_mask_remove_rect():
    marker=stable_marker('UI_TEST_LAYER_MASK_REMOVE_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layers_scroll_clip_rect():
    return stable_window_rect_marker('UI_TEST_LAYERS_SCROLL_CLIP_RECT')

def layer_settings_header():
    marker=stable_marker('UI_TEST_LAYER_SETTINGS_HEADER_RECT')
    return tuple(marker.split()) if marker else None

def layer_opacity_slider_rect():
    marker=stable_marker('UI_TEST_LAYER_OPACITY_SLIDER_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_blend_combo_rect():
    marker=stable_marker('UI_TEST_LAYER_BLEND_COMBO_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_blend_option_rect(mode):
    prefix=f'UI_TEST_LAYER_BLEND_OPTION_RECT: {mode} '
    lines=[line.partition(prefix)[2] for line in (directory/'application.log').read_text().splitlines()
           if line.startswith(prefix)]
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].split()))

def layer_duplicate_menu_rect():
    return layer_menu_action_rect('DUPLICATE')

def layer_more_button_rect():
    marker=stable_marker('UI_TEST_LAYER_MORE_RECT')
    return tuple(map(float,marker.split())) if marker else None

def layer_front_name_rect():
    marker=stable_marker('UI_TEST_LAYER_NAME_RECT')
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
    if len(lines)<2 or lines[-1]!=lines[-2]:
        return None
    return tuple(map(float,lines[-1].partition(': ')[2].split()))

def recovery_preview_image_rect():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_RECOVERY_PREVIEW_IMAGE_RECT: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def recovery_modal_bounds():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_RECOVERY_MODAL_BOUNDS: ')]
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

def home_recent_thumbnail_rect():
    marker=stable_marker('UI_TEST_HOME_RECENT_THUMBNAIL_RECT')
    return tuple(map(float,marker.split())) if marker else None

def top_palette_clearance():
    lines=[line for line in (directory/'application.log').read_text().splitlines()
           if line.startswith('UI_TEST_TOP_PALETTE_CLEARANCE: ')]
    return tuple(map(float,lines[-1].partition(': ')[2].split())) if lines else None

def visible_shape_preview_bounds():
    bounds=window_rect_marker('UI_TEST_SHAPE_PREVIEW_BOUNDS')
    return bounds if bounds and bounds[2]-bounds[0]>=12 and bounds[3]-bounds[1]>=12 else None

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

def wheel_at(px, py, notches=1, down=True):
    move(round(px),round(py))
    button_number=5 if down else 4
    for _ in range(notches):
        assert button(display,button_number,1,0)
        assert button(display,button_number,0,0)
    send()

def scroll_layers_marker_into_view(marker_fn, description):
    marker=wait(marker_fn,description)
    for _ in range(20):
        clip=wait(layers_scroll_clip_rect,'Layers reports its live scroll clip rectangle')
        clip_left,clip_top,clip_width,clip_height,scale=clip
        if len(marker)==6:  # settings-header marker includes an open/closed flag
            _,x,y,width,height,_=marker
            x,y,width,height=map(float,(x,y,width,height))
        else:
            x,y,width,height,_=marker
        # Use the ScrollArea's live clip rectangle rather than approximating it
        # from the outer window. Window chrome, compact sizing, and egui version
        # changes all affect the difference between those bounds.
        visible_top=clip_top+2
        visible_bottom=clip_top+clip_height-2
        if y>=visible_top and y+height<=visible_bottom:
            return marker
        wheel_at(clip_left+clip_width*0.72,
                 visible_bottom-12 if visible_bottom>clip_top else clip_top+12,
                 1,down=y+height>visible_bottom)
        marker=wait(marker_fn,description)
    raise AssertionError(f'{description}: control did not scroll into the Layers viewport ({marker})')

def drag_at(start_x, start_y, end_x, end_y):
    move(round(start_x),round(start_y))
    click_button(1,True)
    time.sleep(0.18)
    move(round((start_x+end_x)/2),round((start_y+end_y)/2))
    move(round(end_x),round(end_y))
    time.sleep(0.12)
    click_button(1,False)

def drag_control_with_frame_ack(start_x, start_y, end_x, end_y, marker_name):
    """Keep a control drag held until the app has observed each pointer frame."""
    def pointer_state_after(previous_count, description, predicate):
        def observed():
            if event_marker_count(marker_name) <= previous_count:
                return None
            marker=event_marker(marker_name)
            return marker if marker and predicate(marker) else None
        return wait(observed,description)

    start_x,start_y=round(start_x),round(start_y)
    end_x,end_y=round(end_x),round(end_y)
    move(start_x,start_y)
    before=event_marker_count(marker_name)
    click_button(1,True)
    pointer_state_after(before,'the application observes the pressed control pointer',
                        lambda marker:'down=true' in marker and 'released=true' not in marker)
    for px,py in [((start_x+end_x)//2,(start_y+end_y)//2),(end_x,end_y)]:
        before=event_marker_count(marker_name)
        move(px,py)
        pointer_state_after(before,'the application observes the held pointer moving over the control',
                            lambda marker:'down=true' in marker and 'released=true' not in marker)
    before=event_marker_count(marker_name)
    click_button(1,False)
    pointer_state_after(before,'the application observes the control pointer release',
                        lambda marker:'released=true' in marker)

def drag_path(points):
    assert len(points)>=2
    move(round(points[0][0]),round(points[0][1]))
    click_button(1,True)
    time.sleep(0.12)
    for x,y in points[1:]:
        move(round(x),round(y))
    time.sleep(0.1)
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

def close_application(process):
    key('Escape')
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        # Escape closes the topmost egui popup first, then the application.
        key('Escape')
    assert process.wait(timeout=20)==0

def status(text):
    try:
        return wait(lambda: text in title(window), text)
    except AssertionError as error:
        raise AssertionError(f'{error}; current window title: {title(window)!r}') from error

def active_brush_size():
    values = [line.partition(': ')[2] for line in
              (directory/'application.log').read_text().splitlines()
              if line.startswith('UI_TEST_BRUSH_SIZE: ')]
    return float(values[-1]) if values else None

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
    if not path.exists(): return None
    return valid_project_bytes(path.read_bytes())
def valid_project_bytes(data):
    try:
        if len(data)<48 or data[:8] not in (b'RPLALP01',b'RPLALP02',b'RPLALP03') or hashlib.sha256(data[:-32]).digest()!=data[-32:]: return None
        _,layers,_=native_document_tables(data)
        # Preserve the helper's original contract: valid files return their
        # raster tile count, so zero distinguishes a clean empty document from
        # a file that still contains painted pixels. None means malformed.
        return sum(len(tiles) for tiles in layers.values())
    except (AssertionError,IndexError,ValueError,struct.error,json.JSONDecodeError):
        return None
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
        if valid_project_bytes(payload) is None: return None
        latest=payload;last_sequence=sequence;offset=record_end
    return latest
def native_state(path):
    return native_state_bytes(path.read_bytes())

def latest_recovery_project(directory):
    candidates=sorted(directory.glob('recovery-[0-9]*.rpl'),
                      key=lambda path:path.stat().st_mtime_ns,reverse=True)
    return next((path for path in candidates if valid_project(path) is not None),None)

def latest_recovery_owner(directory):
    owners=sorted(directory.glob('recovery-[0-9]*.owner'),
                  key=lambda path:path.stat().st_mtime_ns,reverse=True)
    return owners[0] if owners else None

def recovery_complete_count():
    return sum(line.startswith('UI_TEST_RECOVERY_COMPLETE: ')
               for line in (directory/'application.log').read_text().splitlines())

def native_state_bytes(data):
    n=struct.unpack('<I',data[8:12])[0]
    return json.loads(data[12:12+n])

def replace_native_state(data,state):
    """Replace bounded v3 metadata while preserving checked raster/mask tables."""
    assert data[:8]==b'RPLALP03' and hashlib.sha256(data[:-32]).digest()==data[-32:]
    metadata=struct.unpack('<I',data[8:12])[0]
    tables=data[12+metadata:-32]
    encoded=json.dumps(state,separators=(',',':'),allow_nan=False).encode('utf-8')
    body=data[:8]+struct.pack('<I',len(encoded))+encoded+tables
    result=body+hashlib.sha256(body).digest()
    assert valid_project_bytes(result) is not None
    return result
def native_document_tables(data):
    """Read bounded raster/mask tables from an already checksummed RPL file."""
    assert len(data)>=48 and data[:8] in (b'RPLALP01',b'RPLALP02',b'RPLALP03')
    checksum_offset=len(data)-32
    metadata=struct.unpack('<I',data[8:12])[0]
    assert metadata<=(8*1024*1024 if data[:8]==b'RPLALP03' else 65536) and 16+metadata<=checksum_offset
    state=native_state_bytes(data)
    count_offset=12+metadata
    raster_count=struct.unpack('<I',data[count_offset:count_offset+4])[0]
    assert raster_count<=8192
    offset=count_offset+4
    layers={}
    for _ in range(raster_count):
        assert offset+16+16384<=checksum_offset
        layer_id,tile_x,tile_y=struct.unpack('<QII',data[offset:offset+16]);offset+=16
        tile=data[offset:offset+16384];offset+=16384
        layers.setdefault(layer_id,{})[(tile_x,tile_y)]=tile
    masks={}
    if data[:8] in (b'RPLALP02',b'RPLALP03'):
        assert offset+4<=checksum_offset
        mask_count=struct.unpack('<I',data[offset:offset+4])[0];offset+=4
        assert mask_count<=8192-raster_count
        for _ in range(mask_count):
            assert offset+16+4096<=checksum_offset
            layer_id,tile_x,tile_y=struct.unpack('<QII',data[offset:offset+16]);offset+=16
            tile=data[offset:offset+4096];offset+=4096
            masks.setdefault(layer_id,{})[(tile_x,tile_y)]=tile
    assert offset==checksum_offset, 'native raster/mask tile tables do not match document length'
    return state,layers,masks

def native_values_near(left, right):
    """Compare JSON values allowing only tiny f64 serialization roundoff."""
    if isinstance(left,bool) or isinstance(right,bool): return left==right
    if isinstance(left,(int,float)) and isinstance(right,(int,float)):
        return abs(float(left)-float(right))<=1e-12
    if isinstance(left,dict) and isinstance(right,dict):
        return left.keys()==right.keys() and all(
            native_values_near(left[key],right[key]) for key in left)
    if isinstance(left,list) and isinstance(right,list):
        return len(left)==len(right) and all(
            native_values_near(a,b) for a,b in zip(left,right))
    return left==right

def native_projects_equivalent(left, right):
    """Compare restored projects exactly except sub-picopixel JSON float noise."""
    if valid_project_bytes(left) is None or valid_project_bytes(right) is None:
        return False
    if left[:8]!=right[:8]:
        return False
    left_state,left_layers,left_masks=native_document_tables(left)
    right_state,right_layers,right_masks=native_document_tables(right)
    return native_values_near(left_state,right_state) \
        and left_layers==right_layers and left_masks==right_masks

def native_project_difference_summary(expected, actual):
    expected_state,expected_tiles,expected_masks=native_document_tables(expected)
    actual_state,actual_tiles,actual_masks=native_document_tables(actual)
    different_tiles=[]
    different_bytes=0
    for layer_id in set(expected_tiles)|set(actual_tiles):
        expected_layer=expected_tiles.get(layer_id,{})
        actual_layer=actual_tiles.get(layer_id,{})
        for tile_key in set(expected_layer)|set(actual_layer):
            before=expected_layer.get(tile_key,b'\0'*16384)
            after=actual_layer.get(tile_key,b'\0'*16384)
            if before!=after:
                different_tiles.append((layer_id,tile_key))
                different_bytes+=sum(a!=b for a,b in zip(before,after))
    return (
        f'expected_sha256={hashlib.sha256(expected).hexdigest()} '
        f'actual_sha256={hashlib.sha256(actual).hexdigest()} '
        f'state_equal={expected_state==actual_state} '
        f'mask_equal={expected_masks==actual_masks} '
        f'different_tiles={len(different_tiles)} different_bytes={different_bytes} '
        f'tiles={different_tiles[:12]}'
    )
def native_layer_pixel(path, x, y):
    return native_layer_pixel_bytes(path.read_bytes(),x,y)
def native_layer_pixel_bytes(data, x, y):
    state=native_state_bytes(data)
    return native_layer_pixel_by_id_bytes(data,state['selected'],x,y)
def native_layer_pixel_by_id_bytes(data, layer_id, x, y):
    _,layers,_=native_document_tables(data)
    tile=layers.get(layer_id,{}).get((x//64,y//64))
    if tile is not None:
        pixel=((y%64)*64+x%64)*4
        return tile[pixel:pixel+4]
    return b'\0\0\0\0'
def native_layer_mask_pixel_bytes(data, layer_id, x, y):
    state,_,masks=native_document_tables(data)
    layer=next((layer for layer in state['layers'] if layer['id']==layer_id),None)
    if layer is None or not layer.get('mask'):
        return 255
    tile=masks.get(layer_id,{}).get((x//64,y//64))
    if tile is None:
        return 255
    return tile[(y%64)*64+x%64]
def native_layer_tiles(data):
    state=native_state_bytes(data)
    layer_id=state['selected']
    _,layers,_=native_document_tables(data)
    return layers.get(layer_id,{})
def native_layer_tiles_bytes(data):
    _,layers,_=native_document_tables(data)
    return layers

def native_layer_changed_pixels(before_data, after_data, bounds):
    """Count nontransparent changed pixels in a saved native layer region."""
    before_state=native_state_bytes(before_data)
    after_state=native_state_bytes(after_data)
    assert before_state['selected']==after_state['selected'], \
        'the active layer must remain stable while checking a raster edit'
    layer_id=after_state['selected']
    before_tiles=native_layer_tiles_bytes(before_data).get(layer_id,{})
    after_tiles=native_layer_tiles_bytes(after_data).get(layer_id,{})
    left,top,right,bottom=bounds
    x_start=max(0,math.floor(left)); y_start=max(0,math.floor(top))
    x_end=max(x_start,math.ceil(right)); y_end=max(y_start,math.ceil(bottom))
    changed=0
    for tile_y in range(y_start//64,(y_end-1)//64+1):
        for tile_x in range(x_start//64,(x_end-1)//64+1):
            after=after_tiles.get((tile_x,tile_y))
            if after is None:
                continue
            before=before_tiles.get((tile_x,tile_y),bytes(16384))
            x0=max(x_start,tile_x*64); x1=min(x_end,(tile_x+1)*64)
            y0=max(y_start,tile_y*64); y1=min(y_end,(tile_y+1)*64)
            for y in range(y0,y1):
                row=(y-tile_y*64)*64
                for x in range(x0,x1):
                    offset=(row+x-tile_x*64)*4
                    if after[offset+3] and after[offset:offset+4]!=before[offset:offset+4]:
                        changed+=1
    return changed

def png_chunk(kind, data):
    body=kind+data
    return struct.pack('>I',len(data))+body+struct.pack('>I',binascii.crc32(body)&0xffffffff)

def capture_window(window, name):
    folder=os.environ.get('RPL_UI_SCREENSHOT_DIR')
    if not folder: return
    Path(folder).mkdir(parents=True,exist_ok=True)
    print(f'UI_SCREENSHOT_CAPTURE_BEGIN: {name}', flush=True)
    if os.environ.get('RPL_UI_CAPTURE_BACKEND','wgpu') == 'wgpu':
        request=Path(folder)/'capture.request'
        capture=Path(folder)/f'{name}.rplcap'
        capture.unlink(missing_ok=True)
        temporary_request=Path(folder)/'.capture.request.tmp'
        temporary_request.write_text(name)
        os.replace(temporary_request,request)
        # Wake winit's wait loop so the requested surface is composed and read
        # back on the next present. F12 has no application action today.
        key('F12')
        deadline=time.monotonic()+15
        while not capture.is_file() and time.monotonic()<deadline and process.poll() is None:
            time.sleep(0.03)
        assert capture.is_file(), f'wgpu surface capture did not complete for {name}'
        data=capture.read_bytes()
        # The raw RGBA readback is several megabytes per screenshot. The PNG
        # below is the retained artifact, so remove each temporary surface copy
        # immediately instead of filling the constrained CI/tmp filesystem.
        capture.unlink(missing_ok=True)
        assert len(data)>=16 and data[:8]==b'RPLCAP01', f'{name} has an invalid GPU capture header'
        width,height=struct.unpack('<II',data[8:16])
        assert width>0 and height>0 and width<=4096 and height<=4096, f'{name} GPU capture dimensions are invalid'
        pixel_data=data[16:]
        bits_per_pixel=32
        bytes_per_line=width*4
        byte_order=0
        masks=(0x000000ff,0x0000ff00,0x00ff0000)
        assert len(pixel_data)==bytes_per_line*height, f'{name} GPU pixel buffer is truncated'
    else:
        # xwd is retained as an explicit diagnostic fallback for backends where
        # surface copy-source readback is unsupported.
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
    recent_thumbnail_pixels=0
    recent_thumbnail_bounds=None
    home_open_card_bounds=None
    home_open_card_colors=set()
    recent_fixture_badge_pixels=0
    recent_fixture_badge_bounds=None
    saved_swatch_pixels=0
    recovery_preview_bounds=None
    brush_preview_bounds=None
    layer_thumbnail_screen_bounds=None
    layer_thumbnail_content_pixels=0
    saved_swatch_bounds=None
    saved_swatch_color=None
    hue_wheel_bounds=None
    hue_wheel_colors=set()
    format_preview_bounds={}
    format_preview_pixels={}
    format_preview_color_counts={}
    reference_image_bounds=None
    reference_image_colors=set()
    reference_image_share=None
    if name.startswith('recovery-manager-populated-'):
        bounds=wait(recovery_preview_image_rect,'recovery preview image is laid out')
        x,y,preview_width,preview_height,scale,content_top=bounds
        recovery_preview_bounds=(x*scale,(y+content_top)*scale,(x+preview_width)*scale,(y+content_top+preview_height)*scale)
    if name == 'home-recent-artwork-light':
        x,y,preview_width,preview_height,scale=wait(
            home_recent_thumbnail_rect,'first recent project exposes its real thumbnail geometry')
        recent_thumbnail_bounds=(x*scale,y*scale,(x+preview_width)*scale,(y+preview_height)*scale)
        x,y,badge_width,badge_height,scale=wait(
            lambda: window_rect_marker('UI_TEST_HOME_FIXTURE_BADGE_RECT'),
            'generated Home artwork is visibly marked as a development fixture')
        recent_fixture_badge_bounds=(x*scale,y*scale,(x+badge_width)*scale,(y+badge_height)*scale)
        x,y,card_width,card_height,scale=wait(
            lambda: window_rect_marker('UI_TEST_HOME_OPEN_CARD_RECT'),
            'Home open-project action card is laid out after populated recent artwork')
        home_open_card_bounds=(x*scale,y*scale,(x+card_width)*scale,(y+card_height)*scale)
    if name == 'new-artwork-light-normal':
        # Use the actual paper rectangles reported by egui instead of guessing
        # card positions. These disjoint bounds verify the illustrations (not
        # the dialog labels) at any supported pixels-per-point scale.
        assert (width,height)==(1280,800), +            f'{name} format illustration review expects 1280x800, got {width}x{height}'
        for label in ['Portrait','Landscape','Square','A4 Print','Comic Page','Full HD','Custom']:
            x,y,preview_width,preview_height,scale=wait(
                lambda label=label: stable_format_preview_rect(label),
                f'{label} paper illustration is laid out',
            )
            format_preview_bounds[label]=(x*scale,y*scale,
                                          (x+preview_width)*scale,
                                          (y+preview_height)*scale)
            format_preview_pixels[label]=0
            format_preview_color_counts[label]=set()
    if name == 'brush-library-light-normal':
        x,y,preview_width,preview_height,scale=wait(brush_preview_rect,'active raster-engine preview is laid out')
        brush_preview_bounds=(x*scale,y*scale,(x+preview_width)*scale,(y+preview_height)*scale)
    if name == 'color-controls-light':
        marker=wait(lambda: color_palette_marker('UI_TEST_COLOR_SWATCH_RECT'),'saved color swatch is laid out')
        x,y,swatch_width,swatch_height,scale,red,green,blue=marker
        saved_swatch_bounds=(x*scale,y*scale,(x+swatch_width)*scale,(y+swatch_height)*scale)
        saved_swatch_color=(int(red),int(green),int(blue))
        x,y,wheel_width,wheel_height,scale=wait(hue_wheel_rect,'accessible hue wheel is laid out')
        hue_wheel_bounds=(x*scale,y*scale,(x+wheel_width)*scale,(y+wheel_height)*scale)
    if name.startswith('layers-panel-populated-light'):
        x,y,preview_width,preview_height,scale=wait(
            layer_thumbnail_bounds,'selected raster-layer thumbnail reports its rendered bounds')
        layer_thumbnail_screen_bounds=(x*scale,y*scale,(x+preview_width)*scale,
                                       (y+preview_height)*scale)
    if name.startswith('reference-board-'):
        if width < 760:
            image_marker=wait(compact_reference_image_rect,
                              'compact reference screenshot reports the selected image bounds')
            x,y,image_width,image_height,scale=image_marker
            reference_window=wait(compact_reference_window_rect,
                                  'compact reference screenshot reports stable window bounds')
        else:
            x,y,image_width,image_height,scale=wait(reference_image_rect,
                                                     'reference screenshot reports its actual image bounds')
            reference_window=wait(
                lambda: stable_window_rect_marker('UI_TEST_REFERENCE_LAST_WINDOW_RECT'),
                'reference screenshot reports stable outer-window bounds',
            )
        reference_image_bounds=(x*scale,y*scale,(x+image_width)*scale,(y+image_height)*scale)
        reference_image_share=image_width/reference_window[2]
    bytes_per_pixel=(bits_per_pixel+7)//8
    byte_order_name='little' if byte_order==0 else 'big'
    for y in range(height):
        rows.append(0)
        row=pixel_data[y*bytes_per_line:(y+1)*bytes_per_line]
        for x_pixel in range(width):
            start=x_pixel*bytes_per_pixel
            pixel=int.from_bytes(row[start:start+bytes_per_pixel],byte_order_name)
            red=component(pixel,masks[0]);green=component(pixel,masks[1]);blue=component(pixel,masks[2])
            if reference_image_bounds is not None:
                left,top,right,bottom=reference_image_bounds
                if left<=x_pixel<right and top<=y<bottom:
                    reference_image_colors.add((red,green,blue))
                    if (red<80 and green>150 and blue<80) or (
                        green>210 and green-red>50 and green-blue>50
                    ):
                        reference_marker_pixels+=1
            if name == 'home-recent-artwork-light' and blue > 100 and blue-red > 45 and blue-green > 20:
                recent_artwork_pixels+=1
            if recent_thumbnail_bounds is not None:
                left,top,right,bottom=recent_thumbnail_bounds
                if left<=x_pixel<right and top<=y<bottom \
                    and blue > 100 and blue-red > 45 and blue-green > 20:
                    recent_thumbnail_pixels+=1
            if recent_fixture_badge_bounds is not None:
                left,top,right,bottom=recent_fixture_badge_bounds
                if left<=x_pixel<right and top<=y<bottom \
                    and red>140 and red-green>35 and blue>180 and blue-green>80:
                    recent_fixture_badge_pixels+=1
            if home_open_card_bounds is not None:
                left,top,right,bottom=home_open_card_bounds
                if left<=x_pixel<right and top<=y<bottom:
                    home_open_card_colors.add((red,green,blue))
            for label,(left,top,right,bottom) in format_preview_bounds.items():
                if left<=x_pixel<right and top<=y<bottom:
                    format_preview_color_counts[label].add((red,green,blue))
                    if max(red,green,blue)-min(red,green,blue)>48:
                        format_preview_pixels[label]+=1
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
            if hue_wheel_bounds is not None:
                left,top,right,bottom=hue_wheel_bounds
                if left<=x_pixel<right and top<=y<bottom and max(red,green,blue)-min(red,green,blue)>55:
                    hue_wheel_colors.add((red,green,blue))
            if layer_thumbnail_screen_bounds is not None:
                left,top,right,bottom=layer_thumbnail_screen_bounds
                # Exclude the selected-thumbnail frame: require pixels from
                # the raster content inside the actual preview, not its purple
                # selection border.
                if left+2<=x_pixel<right-2 and top+2<=y<bottom-2:
                    checker_colors=((255,255,255),(240,233,246))
                    contrast=min(max(abs(channel-base) for channel,base in zip(
                        (red,green,blue),checker)) for checker in checker_colors)
                    if contrast>=30:
                        layer_thumbnail_content_pixels+=1
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
    sample_x_step=max(1,width//64)
    sample_y_step=max(1,height//64)
    for sample_y in range(0,height,sample_y_step):
        for sample_x in range(0,width,sample_x_step):
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
        assert len(reference_image_colors)>=12, \
            f'{name} image bounds contain too few colors to prove the reference is visible ({len(reference_image_colors)})'
        print(f'UI_SCREENSHOT_REFERENCE_COLORS: {name} {len(reference_image_colors)}', flush=True)
        # The dark 640 × 480 multi-reference state includes Previous/Next,
        # counter, zoom and Settings targets in the compact panel. It retains
        # a useful 75% image share while keeping those controls accessible.
        minimum_reference_share = 0.75 if name == 'reference-board-dark-narrow' else 0.85
        assert minimum_reference_share<=reference_image_share<=1.0, \
            f'{name} has an invalid reference image/window width ratio ({reference_image_share:.1%})'
        print(f'UI_SCREENSHOT_REFERENCE_IMAGE_WIDTH_SHARE: {name} {reference_image_share:.1%}', flush=True)
    if name == 'home-recent-artwork-light':
        card_bounds=wait(home_recent_card_rect,'recent-artwork card geometry is available')
        _left,_top,_width,visible_height,total_height=card_bounds
        assert visible_height>=total_height-1.0, f'{name} crops its visible recent-artwork card ({visible_height:.1f}/{total_height:.1f} points)'
        assert recent_artwork_pixels>=30, f'{name} does not visibly contain the painted blue stroke from the real recent-project thumbnail ({recent_artwork_pixels} matching pixels)'
        assert recent_thumbnail_pixels>=30, \
            f'{name} does not show the painted stroke inside the first actual project thumbnail ({recent_thumbnail_pixels} matching pixels)'
        assert recent_fixture_badge_pixels>=25, \
            f'{name} does not visibly identify generated artwork as a development fixture ({recent_fixture_badge_pixels} matching pixels)'
        print(f'UI_SCREENSHOT_RECENT_ART_PIXELS: {name} {recent_artwork_pixels}', flush=True)
        print(f'UI_SCREENSHOT_RECENT_THUMBNAIL_PIXELS: {name} {recent_thumbnail_pixels}', flush=True)
        print(f'UI_SCREENSHOT_RECENT_FIXTURE_BADGE_PIXELS: {name} {recent_fixture_badge_pixels}', flush=True)
        assert len(home_open_card_colors)>=8, \
            f'{name} does not show the real open-project action in the remaining gallery slot ({len(home_open_card_colors)} colors)'
        print(f'UI_SCREENSHOT_HOME_OPEN_CARD_COLORS: {name} {len(home_open_card_colors)}', flush=True)
        print(f'UI_SCREENSHOT_RECENT_CARD: {name} visible={visible_height:.1f} total={total_height:.1f}', flush=True)
        if width < 1800 or height < 1000:
            print(f'UI_SCREENSHOT_RECENT_ART_WIDE: SKIPPED (capture is {width}x{height}; need 1800x1000)', flush=True)
    if name == 'new-artwork-light-normal':
        assert set(format_preview_bounds)=={
            'Portrait','Landscape','Square','A4 Print','Comic Page','Full HD','Custom'
        }, f'{name} is missing one or more visual format choices: {sorted(format_preview_bounds)}'
        for label in sorted(format_preview_bounds):
            colors=len(format_preview_color_counts[label])
            vivid=format_preview_pixels[label]
            assert colors>=6 and vivid>=12, \
                f'{name} {label} page preview is blank or lacks its illustrated content ({colors} colors, {vivid} vivid pixels)'
            print(f'UI_SCREENSHOT_FORMAT_PREVIEW: {label} colors={colors} vivid-pixels={vivid}', flush=True)
    if name == 'home-light-wide':
        if width < 1920 or height < 1080:
            print(
                f'UI_SCREENSHOT_HOME_FULL_VIEWPORT: SKIPPED (capture clipped to {width}x{height}; need 1920x1080)',
                flush=True,
            )
        else:
            assert home_wide_bottom_dark_pixels<=width*96//20, f'{name} leaves the canvas exposed beneath Home ({home_wide_bottom_dark_pixels} dark canvas pixels in the final 96 rows)'
            print(f'UI_SCREENSHOT_HOME_FULL_VIEWPORT: {name} dark-bottom-pixels={home_wide_bottom_dark_pixels}', flush=True)
    if name.startswith('recovery-manager-populated-'):
        assert len(recovery_preview_colors)>=8, f'{name} does not show a rendered artwork preview ({len(recovery_preview_colors)} visible colors)'
        print(f'UI_SCREENSHOT_RECOVERY_PREVIEW_COLORS: {name} {len(recovery_preview_colors)}', flush=True)
        modal=recovery_modal_bounds()
        assert modal is not None, f'{name} did not report the recovery manager bounds'
        left,top,right,bottom,viewport_width,viewport_height=modal
        assert left>=-0.5 and top>=-0.5 and right<=viewport_width+0.5 and bottom<=viewport_height+0.5, \
            f'{name} recovery dialog extends beyond {viewport_width:.0f}x{viewport_height:.0f}: {modal}'
        if name.endswith('-narrow'):
            assert viewport_width<=640 and viewport_height<=480, f'{name} is not a narrow recovery capture: {modal}'
        print(f'UI_SCREENSHOT_RECOVERY_MODAL_BOUNDS: {name} {left:.1f},{top:.1f}–{right:.1f},{bottom:.1f} inside {viewport_width:.0f}x{viewport_height:.0f}', flush=True)
    if name == 'brush-library-light-normal':
        assert len(brush_preview_colors)>=6, f'{name} does not visibly render the active raster brush sample ({len(brush_preview_colors)} visible colors)'
        print(f'UI_SCREENSHOT_BRUSH_SAMPLE_COLORS: {name} {len(brush_preview_colors)}', flush=True)
    if name == 'color-controls-light':
        assert saved_swatch_pixels>=8, f'{name} does not visibly render the populated saved color palette ({saved_swatch_pixels} matching pixels)'
        print(f'UI_SCREENSHOT_SAVED_SWATCH_PIXELS: {name} {saved_swatch_pixels}', flush=True)
        assert len(hue_wheel_colors)>=32, f'{name} does not visibly render the hue wheel ({len(hue_wheel_colors)} chromatic colors)'
        print(f'UI_SCREENSHOT_HUE_WHEEL_COLORS: {name} {len(hue_wheel_colors)}', flush=True)
    if name.startswith('layers-panel-populated-light'):
        assert layer_thumbnail_content_pixels>=1, \
            f'{name} selected-layer thumbnail did not visibly alter checkerboard pixels inside its frame ({layer_thumbnail_content_pixels})'
        print(f'UI_SCREENSHOT_LAYER_THUMBNAIL_CONTENT_PIXELS: {name} {layer_thumbnail_content_pixels}',flush=True)
    captured_screenshots[name] = (width, height)
    print(f'UI_SCREENSHOT_CAPTURED: {name} {width}x{height}', flush=True)

def resize_window(window, width, height):
    def actual_size():
        root_return=W();x_return=I();y_return=I()
        width_return=U();height_return=U();border_return=U();depth_return=U()
        if not get_geometry(display,window,c.byref(root_return),c.byref(x_return),c.byref(y_return),
                            c.byref(width_return),c.byref(height_return),c.byref(border_return),
                            c.byref(depth_return)):
            return None
        return width_return.value,height_return.value
    for attempt in range(4):
        if attempt == 0:
            resize(display,window,width,height)
        else:
            # Some X11 window managers defer or drop resize-only requests.
            # Reissue with position and size, then verify the server's result.
            move_resize(display,window,24,24,width,height)
        sync(display,0)
        time.sleep(0.12)
        if actual_size()==(width,height):
            return
    raise AssertionError(
        f'X11 refused requested window size {width}x{height}; actual size is {actual_size()}'
    )
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
    if os.environ.get('RPL_KEEP_ALPHA_TEMP'):
        print(f'ALPHA_WORKFLOW_TEMP: {directory}', flush=True)
    executable=Path(__file__).resolve().parent.parent/'target/debug/canvas-demo'
    window_token=f'RPL_UI_TEST_WINDOW_{directory.name}'
    os.environ['RPL_UI_TEST_WINDOW_TOKEN']=window_token
    env={**os.environ,'XDG_STATE_HOME':str(directory/'state'),'RPL_TEST_PATH_DIALOG':'1',
         'RPL_TEST_DOCUMENT_NAME':'artwork','RPL_UI_TEST_WINDOW_TOKEN':window_token}
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
            elif 224 <= x_pixel < 256 and 144 <= y < 176:
                # A large, unique green center marker stays in view after the
                # persisted test zoom/pan and adaptive multi-board placement.
                # It lets screenshots prove the imported reference is visible,
                # not merely the canvas behind a clipped image corner.
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
    second_reference=directory/'reference-imported.png'
    second_reference.write_bytes(reference.read_bytes())
    env['RPL_TEST_REFERENCE_IMPORT_PATH']=str(second_reference)
    prefs=directory/'state/rust-paint-lab/workspace.json'
    prefs.parent.mkdir(parents=True)
    initial_swatches=[[245,88,206],[116,88,246],[60,205,180]]
    prefs.write_text(json.dumps({'version':1,'dark_theme':False,'brush_panel_width':280.0,'saved_color_swatches':initial_swatches,'references':[{'path':str(reference),'position':[620.0,100.0],'size':[320.0,260.0],'opacity':0.7,'view_zoom':1.1,'view_pan':[20.0,20.0]}]}))
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
            assert_strong_text_contrast('light')
            wait(lambda: 'UI_TEST_HOME_QUICK_START_VISIBLE' in (directory / 'application.log').read_text(),
                 'first-run Home uses its hero for actual quick-start canvas actions')
            wait(lambda: 'UI_TEST_HOME_SURFACE_CLEAR: enabled' in (directory / 'application.log').read_text(),
                 'Home clears the document viewport to the active application theme')
            capture_window(window,'home-light-normal')
            resize_window(window,1920,1080)
            capture_window(window,'home-light-wide')
            home_bounds=wait(home_content_bounds,'wide Home content is measured inside its centered layout')
            assert abs(home_bounds[0]+home_bounds[2]/2-960.0)<=4.0 and home_bounds[2]<=1600.0, \
                f'wide Home should remain centered and readable instead of stretching edge to edge: {home_bounds}'
            resize_window(window,640,480)
            capture_window(window,'home-light-narrow')
            resize_window(window,1280,800)
            preset=wait(lambda: settled_home_preset_card_rect('Portrait'),
                        'Home exposes a measured standard Portrait canvas preset')
            preset_target=((preset[0]+preset[2]/2)*preset[4],
                           (preset[1]+preset[3]/2)*preset[4])
            title_before_preset=title(window)
            focus(display,window,2,0);send()
            click_at(*preset_target)
            time.sleep(0.25)
            print(f'UI_TEST_HOME_PRESET_CLICK: target={preset_target} bounds={preset} '
                  f'title={title_before_preset!r}->{title(window)!r}',flush=True)
            wait(lambda: 'UI_TEST_DOCUMENT_CREATED: 1800 2400 artwork 1' in
                 (directory/'application.log').read_text(),
                 'selecting the Home Portrait preset creates a named 1800x2400 painting document directly')
            capture_window(window,'home-preset-created')
            ctrl('n');new_document_prompt();time.sleep(0.25)
            wait(lambda: (directory / 'application.log').read_text().count('UI_TEST_NEW_ARTWORK_VISIBLE') >= 1,
                 'light New Artwork interface has rendered before capture')
            assert_new_document_name_and_create_visible(1280,800)
            capture_window(window,'new-artwork-light-normal')
            resize_window(window,640,480)
            assert_new_document_name_and_create_visible(640,480)
            capture_window(window,'new-artwork-light-narrow')
            # Constrained windows may scroll, but the primary action must be
            # reachable and visible after ordinary wheel navigation.
            wheel_at(320,430,notches=8)
            time.sleep(0.25)
            create=wait(lambda: window_rect_marker('UI_TEST_NEW_DOCUMENT_CREATE_RECT'),
                        'narrow New Artwork scrolling exposes the Create action')
            assert create[1]>=-1 and create[1]+create[3]<=480/create[4]+1, \
                f'Create action is not visible after scrolling the narrow dialog: {create}'
            capture_window(window,'new-artwork-light-narrow-scrolled')
            created_before=sum(line.startswith('UI_TEST_DOCUMENT_CREATED: 1800 2400 artwork 1')
                               for line in (directory/'application.log').read_text().splitlines())
            click_at((create[0]+create[2]/2)*create[4],
                     (create[1]+create[3]/2)*create[4])
            wait(lambda: sum(line.startswith('UI_TEST_DOCUMENT_CREATED: 1800 2400 artwork 1')
                             for line in (directory/'application.log').read_text().splitlines())>created_before,
                 'the visible narrow-dialog Create action creates the selected native document')
            resize_window(window,1280,800)
            capture_window(window,'canvas-workspace-light')
            key('bracketright')
            increased=wait(active_brush_size,'right bracket reports a brush size adjustment')
            assert abs(increased-19.8)<0.01, f'right bracket should increase the 18 px brush by ten percent: {increased}'
            key('bracketleft')
            restored=wait(lambda: active_brush_size()
                          if active_brush_size() is not None and abs(active_brush_size()-18.0)<0.01
                          else None,
                          'left bracket restores the starting brush size')
            palette=wait(palette_bounds,'default compact palette reports its bounds')
            clearance=wait(palette_chrome_clearance,'tool rail reports spacing below the application chrome')
            print(f'UI_PALETTE_CHROME_CLEARANCE: {clearance}', flush=True)
            assert clearance[0] >= clearance[1] + 4.0, \
                f'docked toolbar overlaps the contextual controls: top={clearance[0]:.1f}, chrome_bottom={clearance[1]:.1f}'
            assert palette[2] <= 90.0, f'default artist toolbar should be a compact rail: {palette}'
            assert palette[3] >= 440.0, f'normal-height tool rail should show more than a clipped handful of tools: {palette}'
            assert palette[1] + palette[3] <= 800/palette[4] - 40.0, \
                f'tool rail must leave the bottom status area accessible: {palette}'
            toggle=wait(lambda: settled_window_rect_marker('UI_TEST_PALETTE_TOGGLE_RECT'),
                        'compact palette exposes a keyboard-labeled expand control')
            click_at((toggle[0]+toggle[2]/2)*toggle[4],
                     (toggle[1]+toggle[3]/2)*toggle[4])
            wait(lambda: event_marker('UI_TEST_PALETTE_STATE') == 'collapsed=false',
                 'the toolbar can expand from its compact default')
            wait(lambda: json.loads(prefs.read_text()).get('palette_collapsed') is False,
                 'expanded toolbar preference is persisted')
            expanded=wait(lambda: (lambda bounds: bounds if bounds and bounds[2]>100 else None)(palette_bounds()),
                          'expanded toolbar exposes labeled tools')
            assert expanded[2] > palette[2], \
                f'expanded toolbar should provide more room for tool names: compact={palette}, expanded={expanded}'
            toggle=wait(lambda: settled_window_rect_marker('UI_TEST_PALETTE_TOGGLE_RECT'),
                        'expanded toolbar exposes its updated collapse control')
            click_at((toggle[0]+toggle[2]/2)*toggle[4],
                     (toggle[1]+toggle[3]/2)*toggle[4])
            wait(lambda: event_marker('UI_TEST_PALETTE_STATE') == 'collapsed=true',
                 'the expanded toolbar collapses back to its compact rail')
            wait(lambda: json.loads(prefs.read_text()).get('palette_collapsed') is True,
                 'compact toolbar preference is persisted')
            palette=wait(lambda: (lambda bounds: bounds if bounds and bounds[2]<=90 else None)(palette_bounds()),
                         'compact toolbar width returns after collapse')
            assert json.loads(prefs.read_text()).get('palette_dock') == 'Left', \
                'clicking toolbar controls must not move or redock the palette'
            grip=wait(lambda: window_rect_marker('UI_TEST_PALETTE_GRIP_RECT'),
                      'the toolbar exposes a distinct drag grip')
            drag_at((grip[0]+grip[2]/2)*grip[4], (grip[1]+grip[3]/2)*grip[4],
                    1270, 250)
            wait(lambda: json.loads(prefs.read_text()).get('palette_dock') == 'Right',
                 'dragging the grip to the opposite edge snaps and saves the dock side')
            grip=wait(lambda: window_rect_marker('UI_TEST_PALETTE_GRIP_RECT'),
                      'the repositioned toolbar reports its new grip')
            drag_at((grip[0]+grip[2]/2)*grip[4], (grip[1]+grip[3]/2)*grip[4],
                    10, 250)
            wait(lambda: json.loads(prefs.read_text()).get('palette_dock') == 'Left',
                 'the grip can return the toolbar to its default edge')
            wait(lambda: 'UI_TEST_REFERENCE_IMAGE_VISIBLE:' in (directory / 'application.log').read_text(),
                 'reference screenshot uses a rendered image texture')
            time.sleep(0.35)  # the marker is emitted before egui submits this frame
            capture_window(window,'reference-board-light')
            resize_window(window,640,480);capture_window(window,'reference-board-light-narrow')
            resize_window(window,1280,800)
            settings=wait(lambda: color_palette_marker('UI_TEST_REFERENCE_SETTINGS_RECT'),
                          'reference board exposes its accessible Settings menu')
            click_at((settings[0]+settings[2]/2)*settings[4],
                     (settings[1]+settings[3]/2)*settings[4])
            center_image=wait(lambda: color_palette_marker('UI_TEST_REFERENCE_CENTER_IMAGE_RECT'),
                              'reference Settings menu exposes Center image')
            click_at((center_image[0]+center_image[2]/2)*center_image[4],
                     (center_image[1]+center_image[3]/2)*center_image[4])
            wait(lambda: (lambda state: state.get('view_pan')==[0.0,0.0]
                          and abs(state.get('view_zoom',0.0)-1.1)<0.01)(
                              json.loads(prefs.read_text())['references'][0]),
                 'Center image persists a centered pan while preserving the existing zoom')
            wait(lambda: event_marker('UI_TEST_REFERENCE_INITIAL_POSITION') is not None,
                 'the real reference import path places a second image after Center image')
            wait(lambda: len(json.loads(prefs.read_text()).get('references',[]))==2,
                 'the imported reference layout is persisted beside the existing image')
            second_layout=json.loads(prefs.read_text())['references'][1]
            palette=wait(lambda: window_rect_marker('UI_TEST_PALETTE_RECT'),
                         'the movable tool palette reports bounds for import placement')
            second_rect=tuple(second_layout['position']+second_layout['size'])
            assert not rects_overlap(second_rect,palette[:4]), \
                f'new reference overlaps the tool palette: reference={second_rect}, palette={palette[:4]}'
            assert second_rect[0]>=0 and second_rect[1]>=0 \
                and second_rect[0]+second_rect[2]<=1280/palette[4] \
                and second_rect[1]+second_rect[3]<=800/palette[4], \
                f'new reference is outside the normal viewport: {second_rect}'
            time.sleep(0.3)
            capture_window(window,'reference-board-multi-light')
            saved_multi_reference_geometry=[
                (entry['position'][:],entry['size'][:])
                for entry in json.loads(prefs.read_text()).get('references',[])
            ]
            resize_window(window,640,480)
            time.sleep(0.3)
            narrow_palette=wait(lambda: window_rect_marker('UI_TEST_PALETTE_RECT'),
                                'the palette reports current bounds in a narrow window')
            assert wait(compact_reference_active,'compact reference board selects one active image safely') == (0,2)
            narrow_reference=wait(compact_reference_window_rect,
                                  'the active reference window reports its compact bounds')
            assert not rects_overlap(narrow_reference[:4],narrow_palette[:4]), \
                f'narrow reference overlaps the tool palette: reference={narrow_reference[:4]}, palette={narrow_palette[:4]}'
            assert narrow_reference[0]>=0 and narrow_reference[1]>=0 \
                and narrow_reference[0]+narrow_reference[2]<=640/narrow_palette[4] \
                and narrow_reference[1]+narrow_reference[3]<=480/narrow_palette[4], \
                f'narrow reference is outside the viewport: {narrow_reference}'
            nav=wait(compact_reference_navigation,
                     'the compact board exposes keyboard/touch-sized reference navigation')
            click_at((nav[2]+nav[4]/2)*nav[6],(nav[3]+nav[5]/2)*nav[6])
            assert wait(compact_reference_active,'Next changes the compact reference selection') == (1,2)
            second_narrow_reference=wait(compact_reference_window_rect,
                                         'the second reference image receives a reachable compact window')
            assert second_narrow_reference[0]>=0 and second_narrow_reference[1]>=0 \
                and second_narrow_reference[0]+second_narrow_reference[2]<=640/narrow_palette[4] \
                and second_narrow_reference[1]+second_narrow_reference[3]<=480/narrow_palette[4], \
                f'the second reference is outside the viewport: {second_narrow_reference}'
            second_image=wait(reference_image_rect,'the selected second reference has a visible image viewport')
            assert second_image[2]>=48 and second_image[3]>=48, \
                f'the selected second reference image is too small: {second_image}'
            capture_window(window,'compact-reference-second')
            nav=wait(compact_reference_navigation,'the compact board still exposes Previous')
            click_at((nav[2]+nav[4]/2)*nav[6],(nav[3]+nav[5]/2)*nav[6])
            assert wait(compact_reference_active,'Previous restores the first compact reference') == (0,2)
            first_image=wait(compact_reference_image_rect,
                             'the first reference remains visible after compact navigation')
            assert first_image[2]>=48 and first_image[3]>=48, \
                f'the first compact reference image is too small: {first_image}'
            compact_reference_geometry=[
                (entry['position'][:],entry['size'][:])
                for entry in json.loads(prefs.read_text()).get('references',[])
            ]
            assert compact_reference_geometry==saved_multi_reference_geometry, \
                'a temporary compact reference layout must preserve roomy saved geometry'
            capture_window(window,'compact-reference-first')
            resize_window(window,1280,800)
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
            time.sleep(0.8)  # allow egui to finish its newly opened Window sizing pass
            capture_window(window,'brush-library-light-normal')
            select_quick_brush('Ink')
            select_quick_brush('Round')
            print('UI_SCREENSHOT_BRUSH_LAYOUT: ' + str(event_marker('UI_TEST_BRUSH_PANEL')), flush=True)
            brush_bounds=wait(lambda: window_rect_marker('UI_TEST_BRUSH_WINDOW_RECT'),
                              'brush window reports its final bounds')
            reference_bounds=wait(lambda: stable_window_rect_marker('UI_TEST_REFERENCE_LAST_WINDOW_RECT'),
                                  'reference board reports its final bounds')
            brush_rect=tuple(value*brush_bounds[4] for value in brush_bounds[:4])
            reference_rect=tuple(value*reference_bounds[4] for value in reference_bounds[:4])
            assert brush_rect[2] <= 600.0, \
                f'brush flyout exceeded its compact maximum width: {brush_rect[2]:.1f}'
            assert not rects_overlap(brush_rect,reference_rect), \
                f'brush flyout covers the reference board: brush={brush_rect}, reference={reference_rect}'
            resize_window(window,640,480)
            narrow_layout=wait(brush_narrow_layout,
                               'narrow Brush flyout and toolbar report their bounded layout')
            panel_rect=tuple(value*narrow_layout[8] for value in narrow_layout[:4])
            palette_rect=tuple(value*narrow_layout[8] for value in narrow_layout[4:8])
            assert panel_rect[0]>=0 and panel_rect[1]>=0 and panel_rect[0]+panel_rect[2]<=640.0 \
                and panel_rect[1]+panel_rect[3]<=480.0, \
                f'narrow Brush flyout escapes the window: {panel_rect}'
            assert not rects_overlap(panel_rect,palette_rect), \
                f'narrow Brush flyout obscures the tool palette: panel={panel_rect}, palette={palette_rect}'
            visible_card=wait(brush_narrow_visible_card,
                              'at least one complete brush preset card is visible without scrolling')
            assert visible_card[3]>=60.0, \
                f'narrow Brush preset card is clipped: {visible_card}'
            visible_count=wait(brush_narrow_visible_cards_ready,
                               'narrow brush gallery shows at least two fully visible presets')
            assert visible_count>=2, \
                f'narrow Brush gallery should expose at least two presets before scrolling, got {visible_count}'
            wait(lambda: 'UI_TEST_COMPACT_BRUSH_FOCUS: layers_hidden=' in (directory/'application.log').read_text()
                 and 'UI_TEST_COMPACT_BRUSH_FOCUS: layers_hidden=false references_hidden=2' in
                     (directory/'application.log').read_text(),
                 'compact Brush focus keeps the two saved references available without overlapping the gallery')
            assert len(json.loads(prefs.read_text()).get('references',[]))==2, \
                'compact Brush focus must preserve the hidden reference layouts'
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
            wait(lambda: json.loads(prefs.read_text()).get('show_controls') is True,
                 'opening workspace controls is persisted before the dismiss test')
            wait(lambda: stable_marker('UI_TEST_WORKSPACE_CONTROLS_ACTIVE') == 'Layers',
                 'floating workspace controls show the current Layers tab')
            capture_window(window,'workspace-controls-floating-light-normal')
            launch_layers=wait(workspace_open_layers_rect,
                               'the redundant Layers tab offers its integrated floating-panel action')
            layer_window_count=sum(line.startswith('UI_TEST_LAYERS_WINDOW_RECT: ')
                                   for line in (directory/'application.log').read_text().splitlines())
            click_at((launch_layers[0]+launch_layers[2]/2)*launch_layers[4],
                     (launch_layers[1]+launch_layers[3]/2)*launch_layers[4])
            wait(lambda: event_marker('UI_TEST_LAYERS_FLYOUT_OPENED')=='workspace controls',
                 'the Layers tab opens the real thumbnail-backed Layers panel')
            wait(lambda: sum(line.startswith('UI_TEST_LAYERS_WINDOW_RECT: ')
                             for line in (directory/'application.log').read_text().splitlines())>
                 layer_window_count,
                 'the integrated Layers panel reports its real floating-window bounds')
            wait(lambda: json.loads(prefs.read_text()).get('show_controls') is False,
                 'launching Layers closes the duplicate inspector and persists that workspace state')
            key('Escape')
            wait(lambda: event_marker('UI_TEST_ESCAPE_DISMISSED')=='layers',
                 'Escape dismisses the launched Layers panel')
            key('F6')
            wait(lambda: stable_marker('UI_TEST_WORKSPACE_CONTROLS_ACTIVE')=='Layers',
                 'F6 reopens the workspace launcher after closing Layers')
            tab_bounds=wait(workspace_brush_tab_rect,'floating Brush settings tab is laid out')
            click_at((tab_bounds[0]+tab_bounds[2]/2)*tab_bounds[4],
                     (tab_bounds[1]+tab_bounds[3]/2)*tab_bounds[4])
            wait(lambda: stable_marker('UI_TEST_WORKSPACE_CONTROLS_ACTIVE') == 'Brush',
                 'floating workspace controls switch to the Brush Library launcher')
            launch_brush=wait(workspace_open_brush_rect,
                              'the Brush tab offers its integrated Brush Library action')
            brush_panel_count=sum(line == 'UI_TEST_BRUSH_PANEL_VISIBLE'
                                   for line in (directory/'application.log').read_text().splitlines())
            click_at((launch_brush[0]+launch_brush[2]/2)*launch_brush[4],
                     (launch_brush[1]+launch_brush[3]/2)*launch_brush[4])
            wait(lambda: event_marker('UI_TEST_BRUSH_FLYOUT_OPENED')=='workspace controls',
                 'the Brush tab opens the actual preset and settings Library')
            wait(lambda: sum(line == 'UI_TEST_BRUSH_PANEL_VISIBLE'
                             for line in (directory/'application.log').read_text().splitlines())>
                 brush_panel_count,
                 'the launched Brush Library renders its actual floating panel')
            wait(lambda: json.loads(prefs.read_text()).get('show_controls') is False,
                 'launching Brush closes the duplicate inspector and persists that workspace state')
            key('Escape')
            wait(lambda: event_marker('UI_TEST_ESCAPE_DISMISSED')=='brush',
                 'Escape dismisses the launched Brush Library')
            key('F6')
            wait(lambda: stable_marker('UI_TEST_WORKSPACE_CONTROLS_ACTIVE')=='Brush',
                 'F6 restores the workspace launcher to its saved Brush tab')
            workspace_save_count=(directory/'application.log').read_text().count('UI_TEST_WORKSPACE_SAVE:')
            key('Escape')
            assert process.poll() is None, 'Escape closed the application instead of dismissing workspace controls'
            wait(lambda: 'UI_TEST_ESCAPE_DISMISSED: workspace controls' in (directory/'application.log').read_text(),
                 'Escape dismisses floating workspace controls')
            wait(lambda: (directory/'application.log').read_text().count('UI_TEST_WORKSPACE_SAVE:')>workspace_save_count,
                 'floating workspace close state is written to preferences')
            wait(lambda: json.loads(prefs.read_text()).get('show_controls') is False,
                 'floating workspace close state reaches the saved preference file')
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
            assert current_brush_rgb()==(245,88,206), f'hue test expects the selected magenta swatch, got {current_brush_rgb()}'
            wheel_bounds=wait(hue_wheel_rect,'hue wheel is available for direct pointer testing')
            top_hue_point=wheel_point(0.0,wheel_bounds)
            print(f'UI_TEST_HUE_WHEEL_POINTER_TARGET: {wheel_bounds} {top_hue_point}',flush=True)
            click_at(*top_hue_point)
            wait(lambda: current_brush_rgb()==(245,88,88),
                 'clicking the hue wheel updates brush color while preserving saturation and brightness')
            original_swatch=wait(original_saved_color_rect,'the original brush color remains restorable from Saved palette')
            click_at((original_swatch[0]+original_swatch[2]/2)*original_swatch[4],
                     (original_swatch[1]+original_swatch[3]/2)*original_swatch[4])
            wait(lambda: current_brush_rgb()==(35,65,180),
                 'the saved original brush color restores the active color after hue-wheel testing')
            key('Escape')
            wait(lambda: 'UI_TEST_ESCAPE_DISMISSED: color' in (directory/'application.log').read_text(),
                 'Escape dismisses Color controls')
            ctrl('l');time.sleep(0.3)
            window=wait(find_window,'window after Ctrl+L opens Layers')
            # Arrange the two saved references to the right of the fitted
            # portrait artboard and the compact Layers panel immediately left
            # of it, leaving an unobstructed strip for genuine paint input.
            layer_bounds=wait(layers_window_rect,'Layers panel is ready to be moved for canvas interaction')
            layer_target_x=90.0
            drag_at((layer_bounds[0]+layer_bounds[2]/2)*layer_bounds[4],
                    (layer_bounds[1]+16.0)*layer_bounds[4],
                    (layer_target_x+layer_bounds[2]/2)*layer_bounds[4],
                    (layer_bounds[1]+16.0)*layer_bounds[4])
            moved_layers=wait(lambda: layers_window_rect()
                               if layers_window_rect() and abs(layers_window_rect()[0]-layer_target_x)<3
                               else None,
                               'Layers panel movement leaves the fitted artboard reachable')
            for reference_index,target in [(0,(900.0,90.0)),(1,(900.0,425.0))]:
                reference_bounds=wait(lambda reference_index=reference_index:
                                       reference_window_rect(reference_index),
                                       f'Reference {reference_index+1} reports its panel geometry')
                drag_at((reference_bounds[0]+40.0)*reference_bounds[4],
                        (reference_bounds[1]+16.0)*reference_bounds[4],
                        (target[0]+40.0)*reference_bounds[4],
                        (target[1]+16.0)*reference_bounds[4])
                wait(lambda reference_index=reference_index,target=target:
                     (lambda bounds: bounds if bounds and abs(bounds[0]-target[0])<3
                      and abs(bounds[1]-target[1])<3 else None)(reference_window_rect(reference_index)),
                     f'Reference {reference_index+1} moves outside the fitted artboard')
            ctrl_shift('n');ctrl_shift('n');time.sleep(0.3)
            wait(lambda: 'UI_TEST_LAYERS_ADDED: 3' in (directory/'application.log').read_text(), 'Layers panel contains three real editable layers')
            populated_layer_id=wait(layer_front_row_id,'the frontmost editable layer is selected')
            empty_thumbnail=wait(lambda: layer_thumbnail_update(populated_layer_id),
                                 'the selected layer starts with a cached transparent thumbnail')
            assert empty_thumbnail[1]==0 and empty_thumbnail[2]==0, f'new layer should begin transparent, got {empty_thumbnail}'
            paint_start_count=sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                                  for line in (directory/'application.log').read_text().splitlines())
            key('b');drag_at(460,250,550,330)
            wait(lambda: sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>paint_start_count,
                 'painting on the selected layer uses the ordinary input pipeline')
            painted_thumbnail=wait(
                lambda: (lambda value: value if value and value[0]!=empty_thumbnail[0]
                         and value[1]>0 and value[2]>=5 else None)(layer_thumbnail_update(populated_layer_id)),
                'painting refreshes the actual selected-layer thumbnail content',
            )
            assert painted_thumbnail[0]!=empty_thumbnail[0]
            print(f'UI_TEST_LAYER_THUMBNAIL_PAINTED: {populated_layer_id} covered={painted_thumbnail[1]} colored={painted_thumbnail[2]}',flush=True)
            capture_window(window,'layers-panel-populated-light')
            resize_window(window,1920,1080);capture_window(window,'layers-panel-populated-light-wide')
            resize_window(window,640,480);capture_window(window,'layers-panel-populated-light-narrow')
            resize_window(window,1280,800)
            ctrl('z')
            restored_thumbnail=wait(
                lambda: (lambda value: value if value and value[0]==empty_thumbnail[0]
                         else None)(layer_thumbnail_update(populated_layer_id)),
                'undo restores the exact cached thumbnail of the transparent layer',
            )
            assert restored_thumbnail[1]==0 and restored_thumbnail[2]==0
            visibility_marker_count=sum(
                line.startswith('UI_TEST_LAYER_VISIBILITY_BUTTON_RECT: ')
                for line in (directory/'application.log').read_text().splitlines()
            )
            resize_window(window,640,480)
            wait(lambda: sum(
                line.startswith('UI_TEST_LAYER_VISIBILITY_BUTTON_RECT: ')
                for line in (directory/'application.log').read_text().splitlines()
            )>visibility_marker_count,
                 'the layer controls have been rendered again after narrowing the viewport')
            wait(lambda: 'viewport=640.0x480.0' in (event_marker('UI_TEST_COMPACT_PLAN') or ''),
                 'the layer controls have recomputed their positions for the narrow viewport')
            compact_more=wait(layer_more_button_rect,
                              'compact layer rows keep their contextual visibility controls reachable')
            compact_layer_id=layer_front_row_id()
            compact_visibility=wait(layer_visibility_button_rect,
                                    'compact rows expose a direct accessible visibility control')
            visibility_point=((compact_visibility[0]+compact_visibility[2]/2)*compact_visibility[4],
                              (compact_visibility[1]+compact_visibility[3]/2)*compact_visibility[4])
            print(f'UI_TEST_LAYER_VISIBILITY_CLICK: {visibility_point}', flush=True)
            click_at(*visibility_point)
            wait(lambda: event_marker('UI_TEST_LAYER_VISIBILITY_BUTTON_CHANGED')==
                 f'{compact_layer_id} false',
                 'the direct visibility button hides the actual document layer')
            wait(lambda: event_marker('UI_TEST_LAYER_STATE_APPLIED')==
                 f'{compact_layer_id} visible=false locked=false',
                 'direct visibility changes persist into document state')
            compact_visibility=wait(layer_visibility_button_rect,
                                    'the selected hidden layer remains directly showable')
            click_at((compact_visibility[0]+compact_visibility[2]/2)*compact_visibility[4],
                     (compact_visibility[1]+compact_visibility[3]/2)*compact_visibility[4])
            wait(lambda: event_marker('UI_TEST_LAYER_STATE_APPLIED')==
                 f'{compact_layer_id} visible=true locked=false',
                 'the direct visibility control restores the layer')
            compact_lock=wait(layer_lock_button_rect,
                              'compact rows expose a direct accessible lock control')
            click_at((compact_lock[0]+compact_lock[2]/2)*compact_lock[4],
                     (compact_lock[1]+compact_lock[3]/2)*compact_lock[4])
            wait(lambda: event_marker('UI_TEST_LAYER_LOCK_BUTTON_CHANGED')==
                 f'{compact_layer_id} true',
                 'the direct lock button locks the actual document layer')
            wait(lambda: event_marker('UI_TEST_LAYER_STATE_APPLIED')==
                 f'{compact_layer_id} visible=true locked=true',
                 'direct locking persists into document state')
            compact_lock=wait(layer_lock_button_rect,
                              'the selected locked layer remains directly unlockable')
            click_at((compact_lock[0]+compact_lock[2]/2)*compact_lock[4],
                     (compact_lock[1]+compact_lock[3]/2)*compact_lock[4])
            wait(lambda: event_marker('UI_TEST_LAYER_STATE_APPLIED')==
                 f'{compact_layer_id} visible=true locked=false',
                 'the direct lock control restores the unlocked layer')
            compact_open_count=layer_actions_open_count()
            click_at((compact_more[0]+compact_more[2]/2)*compact_more[4],
                     (compact_more[1]+compact_more[3]/2)*compact_more[4])
            wait(lambda: layer_actions_open_count()>compact_open_count,
                 'compact layer actions open without growing the panel stack')
            compact_visible=wait(lambda: layer_menu_action_rect('VISIBLE'),
                                 'compact layer menu exposes the real visibility toggle')
            click_at((compact_visible[0]+compact_visible[2]/2)*compact_visible[4],
                     (compact_visible[1]+compact_visible[3]/2)*compact_visible[4])
            wait(lambda: event_marker('UI_TEST_LAYER_VISIBILITY_CHANGED') ==
                 f'{compact_layer_id} false',
                 'the compact visibility checkbox applies a real layer-state change')
            wait(lambda: event_marker('UI_TEST_LAYER_STATE_APPLIED') ==
                 f'{compact_layer_id} visible=false locked=false',
                 'compact visibility action changes the document layer state')
            wait(lambda: event_marker('UI_TEST_LAYER_MENU_STATE') ==
                 f'{compact_layer_id} visible=false locked=false',
                 'the open compact menu reflects the changed visibility state')
            compact_visible=wait(lambda: layer_menu_action_rect('VISIBLE'),
                                 'the compact visibility toggle remains available for restoration')
            click_at((compact_visible[0]+compact_visible[2]/2)*compact_visible[4],
                     (compact_visible[1]+compact_visible[3]/2)*compact_visible[4])
            wait(lambda: event_marker('UI_TEST_LAYER_VISIBILITY_CHANGED') ==
                 f'{compact_layer_id} true',
                 'the compact visibility toggle restores the original layer state')
            wait(lambda: event_marker('UI_TEST_LAYER_STATE_APPLIED') ==
                 f'{compact_layer_id} visible=true locked=false',
                 'the document layer is restored before continuing regression checks')
            wait(lambda: event_marker('UI_TEST_LAYER_MENU_STATE') ==
                 f'{compact_layer_id} visible=true locked=false',
                 'the open compact menu reflects the restored visibility')
            compact_locked=wait(lambda: layer_menu_action_rect('LOCKED'),
                                'compact layer menu exposes the lock toggle')
            click_at((compact_locked[0]+compact_locked[2]/2)*compact_locked[4],
                     (compact_locked[1]+compact_locked[3]/2)*compact_locked[4])
            wait(lambda: event_marker('UI_TEST_LAYER_LOCK_CHANGED') ==
                 f'{compact_layer_id} true',
                 'the compact lock checkbox applies a real layer-state change')
            wait(lambda: event_marker('UI_TEST_LAYER_STATE_APPLIED') ==
                 f'{compact_layer_id} visible=true locked=true',
                 'compact locking changes the document layer state')
            wait(lambda: event_marker('UI_TEST_LAYER_MENU_STATE') ==
                 f'{compact_layer_id} visible=true locked=true',
                 'the open compact menu reflects the locked state')
            compact_locked=wait(lambda: layer_menu_action_rect('LOCKED'),
                                'compact layer lock toggle remains available for restoration')
            click_at((compact_locked[0]+compact_locked[2]/2)*compact_locked[4],
                     (compact_locked[1]+compact_locked[3]/2)*compact_locked[4])
            wait(lambda: event_marker('UI_TEST_LAYER_LOCK_CHANGED') ==
                 f'{compact_layer_id} false',
                 'the compact lock checkbox restores the original layer state')
            wait(lambda: event_marker('UI_TEST_LAYER_STATE_APPLIED') ==
                 f'{compact_layer_id} visible=true locked=false',
                 'the layer is restored before continuing regression checks')
            wait(lambda: event_marker('UI_TEST_LAYER_MENU_STATE') ==
                 f'{compact_layer_id} visible=true locked=false',
                 'the open compact menu reflects the restored unlocked state')
            key('Escape')
            wait(lambda: event_marker('UI_TEST_ESCAPE_DISMISSED')=='menu',
                 'Escape closes the compact layer menu before the Layers panel')
            menu_state_count=sum(line.startswith('UI_TEST_LAYER_MENU_STATE: ')
                                 for line in (directory/'application.log').read_text().splitlines())
            time.sleep(0.25)
            assert sum(line.startswith('UI_TEST_LAYER_MENU_STATE: ')
                       for line in (directory/'application.log').read_text().splitlines()) == menu_state_count, \
                'Escape must dismiss the compact layer menu before resizing'
            front_row_marker_count=sum(
                line.startswith('UI_TEST_LAYER_FRONT_ROW_RECT: ')
                for line in (directory/'application.log').read_text().splitlines()
            )
            resize_window(window,1280,800)
            wait(lambda: sum(
                line.startswith('UI_TEST_LAYER_FRONT_ROW_RECT: ')
                for line in (directory/'application.log').read_text().splitlines()
            )>front_row_marker_count,
                 'the full-size layer row has been laid out after widening the viewport')
            front_row=wait(layer_front_row_rect,'frontmost layer row offers its contextual actions')
            more=wait(layer_more_button_rect,'frontmost layer exposes a keyboard-accessible actions menu')
            opened=layer_actions_open_count()
            click_at((more[0]+more[2]/2)*more[4],(more[1]+more[3]/2)*more[4])
            wait(lambda: layer_actions_open_count()>opened,
                 'the frontmost layer Actions menu opens')
            duplicate_menu=wait(layer_duplicate_menu_rect,'layer context menu exposes Duplicate layer')
            click_at((duplicate_menu[0]+duplicate_menu[2]/2)*duplicate_menu[4],
                     (duplicate_menu[1]+duplicate_menu[3]/2)*duplicate_menu[4])
            wait(lambda: event_marker('UI_TEST_LAYER_DUPLICATE_CLICKED') is not None,
                 'the Duplicate layer menu item receives the real pointer click')
            wait(lambda: layer_context_duplicate_count()==4,
                 'layer context Duplicate targets the clicked layer and creates an editable copy')
            row_count_before_undo=sum(
                line.startswith('UI_TEST_LAYER_FRONT_ROW_RECT: ')
                for line in (directory/'application.log').read_text().splitlines()
            )
            ctrl('z')
            wait(lambda: history_layer_count()==3,
                 'undo removes the context-menu duplicate without changing the other layers')
            wait(lambda: sum(
                line.startswith('UI_TEST_LAYER_FRONT_ROW_RECT: ')
                for line in (directory/'application.log').read_text().splitlines()
            )>row_count_before_undo,
                 'the restored front layer has been laid out after undoing duplication')
            front_id=layer_front_row_id()
            more=wait(layer_more_button_rect,'frontmost layer action button remains available after undo')
            opened=layer_actions_open_count()
            click_at((more[0]+more[2]/2)*more[4],(more[1]+more[3]/2)*more[4])
            wait(lambda: layer_actions_open_count()>opened and
                 event_marker('UI_TEST_LAYER_ACTIONS_OPENED')==str(front_id),
                 'the frontmost layer Actions menu reopens for reordering')
            try:
                send_backward=wait(lambda: layer_menu_action_rect('SEND_BACKWARD'),
                                   'Send backward is available for a non-bottom layer')
            except AssertionError:
                lines=(directory/'application.log').read_text().splitlines()
                start=max((i for i,line in enumerate(lines)
                           if line.startswith('UI_TEST_LAYER_ACTIONS_OPENED: ')),default=-1)
                print('UI_TEST_LAYER_REORDER_MENU_DIAGNOSTIC:\n'+'\n'.join(lines[start:][-80:]),
                      flush=True)
                raise
            capture_window(window,'layers-reorder-send-backward-menu')
            move_click_count=sum(
                line.startswith('UI_TEST_LAYER_MOVE_MENU_CLICKED: ')
                for line in (directory/'application.log').read_text().splitlines()
            )
            click_at((send_backward[0]+16)*send_backward[4],
                     (send_backward[1]+send_backward[3]/2)*send_backward[4])
            wait(lambda: sum(
                line.startswith('UI_TEST_LAYER_MOVE_MENU_CLICKED: ')
                for line in (directory/'application.log').read_text().splitlines()
            )>move_click_count,
                 'Send backward receives the real pointer click in the Layers menu')
            wait(lambda: event_marker('UI_TEST_LAYER_CONTEXT_MOVED') is not None,
                 'Send backward records a targeted layer-order transaction')
            wait(lambda: layer_front_row_id()!=front_id,
                 'targeted Send backward changes the frontmost layer')
            ctrl('z')
            wait(lambda: layer_front_row_id()==front_id,
                 'undo restores the previous layer order')
            front_name=wait(layer_front_name_rect,
                            'the frontmost target can be selected directly before painting')
            click_at((front_name[0]+front_name[2]/2)*front_name[4],
                     (front_name[1]+front_name[3]/2)*front_name[4])
            wait(lambda: event_marker('UI_TEST_LAYER_SELECTED')==str(front_id),
                 'direct layer-row selection targets the layer to clear')
            paint_count=sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                            for line in (directory/'application.log').read_text().splitlines())
            key('b');drag_at(460,250,550,330)
            wait(lambda: sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>paint_count,
                 'the targeted front layer receives raster content through the real mouse pipeline')
            before_clear_thumbnail=wait(lambda: layer_thumbnail_update(front_id),
                                        'the selected layer has a rendered thumbnail before clearing')
            assert before_clear_thumbnail[2]>0, \
                'the context-clear test requires real selected-layer raster content'
            more=wait(layer_more_button_rect,'frontmost layer exposes its targeted clear action')
            opened=layer_actions_open_count()
            click_at((more[0]+more[2]/2)*more[4],(more[1]+more[3]/2)*more[4])
            wait(lambda: layer_actions_open_count()>opened and
                 event_marker('UI_TEST_LAYER_ACTIONS_OPENED')==str(front_id),
                 'the populated front layer Actions menu opens for a targeted clear')
            clear_contents=wait(lambda: layer_menu_action_rect('CLEAR'),
                                'layer Actions exposes Clear layer contents')
            click_at((clear_contents[0]+clear_contents[2]/2)*clear_contents[4],
                     (clear_contents[1]+clear_contents[3]/2)*clear_contents[4])
            wait(lambda: event_marker('UI_TEST_LAYER_CLEAR_CLICKED')==str(front_id),
                 'Clear layer contents targets the actual frontmost row')
            wait(lambda: event_marker('UI_TEST_LAYER_CONTEXT_CLEARED')==str(front_id),
                 'the document engine clears pixels on the requested layer ID')
            cleared_thumbnail=wait(
                lambda: (lambda value: value if value and value[0]!=before_clear_thumbnail[0]
                         and value[1]==0 and value[2]==0 else None)(layer_thumbnail_update(front_id)),
                'the selected layer thumbnail becomes transparent after contextual Clear',
            )
            ctrl('z')
            restored_clear_thumbnail=wait(
                lambda: (lambda value: value if value and value[0]==before_clear_thumbnail[0]
                         else None)(layer_thumbnail_update(front_id)),
                'Undo restores the exact raster-thumbnail pixels after contextual Clear',
            )
            assert restored_clear_thumbnail==before_clear_thumbnail
            ctrl('z')
            wait(lambda: (lambda value: value if value and value[1]==0 and value[2]==0 else None)(
                     layer_thumbnail_update(front_id)),
                 'a second Undo removes the temporary clear-test stroke before the save regression')
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
            order_before=[layer_row_id(index) for index in range(3)]
            assert all(layer_id is not None for layer_id in order_before), f'layer order markers are incomplete: {order_before}'
            drag=wait(lambda: layer_drag_handle_rect(0),
                      'frontmost layer has a dedicated drag-to-reorder handle')
            target=wait(lambda: layer_row_rect(2),
                        'backmost populated layer exposes a drop target')
            drag_count=sum(line.startswith('UI_TEST_LAYER_DRAG_REORDERED: ')
                           for line in (directory/'application.log').read_text().splitlines())
            drag_start_count=sum(line.startswith('UI_TEST_LAYER_DRAG_STARTED: ')
                                 for line in (directory/'application.log').read_text().splitlines())
            scale=drag[6]
            drag_at((drag[2]+drag[4]/2)*scale,(drag[3]+drag[5]/2)*scale,
                    (target[2]+target[4]/2)*target[6],(target[3]+target[5]/2)*target[6])
            wait(lambda: sum(line.startswith('UI_TEST_LAYER_DRAG_STARTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>drag_start_count,
                 'layer drag grip starts a pointer transaction')
            wait(lambda: sum(line.startswith('UI_TEST_LAYER_DRAG_REORDERED: ')
                             for line in (directory/'application.log').read_text().splitlines())>drag_count,
                 'dragging a layer row commits one targeted reorder transaction')
            wait(lambda: layer_row_id(2)==order_before[0],
                 'dragging the frontmost layer onto the last row moves it to the back')
            ctrl('z')
            wait(lambda: [layer_row_id(index) for index in range(3)]==order_before,
                 'undo restores the exact pre-drag layer order')
            name_rect=wait(layer_front_name_rect,'frontmost layer name remains a native context-menu target')
            right_click_at((name_rect[0]+name_rect[2]/2)*name_rect[4],
                           (name_rect[1]+name_rect[3]/2)*name_rect[4])
            wait(lambda: 'UI_TEST_LAYER_NATIVE_CONTEXT_OPENED' in (directory/'application.log').read_text(),
                 'right-clicking a layer name opens its native context menu')
            key('Escape')
            move(460,250);click_button(1,True);move(550,350);click_button(1,False)
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
            assert struct.unpack('>II',data[16:24])==(1800,2400)
            png_width,png_height,pixels=decode_rgba_png(data)
            assert (png_width,png_height)==(1800,2400)
            assert not any(pixels[i:i+4]==b'\xff\x00\xff\xff' for i in range(0,len(pixels),4)), 'workspace reference pixels leaked into the exported artwork'
            assert not any(pixels[i] < 80 and pixels[i+1] > 120 and pixels[i+2] < 100 for i in range(0,len(pixels),4)), 'workspace reference marker pixels leaked into the exported artwork'
            assert json.loads(prefs.read_text())['references'][0]['path']==str(reference)
            close_application(process)
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'reopened workspace');focus(display,window,2,0);send()
            # A newly created native window starts at 1000×700 even though the
            # preceding process exercised a 1280×800 layout. Restore the
            # expected viewport before using canvas-space test coordinates.
            resize_window(window,1280,800);time.sleep(0.35)
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
            layer_window_count=sum(
                line.startswith('UI_TEST_LAYERS_WINDOW_RECT: ')
                for line in (directory/'application.log').read_text().splitlines()
            )
            ctrl('l')
            wait(lambda: sum(
                line.startswith('UI_TEST_LAYERS_WINDOW_RECT: ')
                for line in (directory/'application.log').read_text().splitlines()
            )>layer_window_count,
                 'the reopened project exposes Layers before testing layer actions')
            reopened_layers=wait(layers_window_rect,'the reopened Layers panel reports its viewport bounds')
            assert reopened_layers[1]>=0 and reopened_layers[1]+reopened_layers[3]<=800, \
                f'Layers must remain within the 1280×800 application viewport: {reopened_layers}'
            # Exercise actual selection-tool ownership and transactional fill.
            time.sleep(0.5)
            saved=project.read_bytes()
            # Keep the captured selection gesture on exposed portrait-canvas
            # pixels; the moved Layers panel occupies x=90..450.
            selection_commits=event_marker_count('UI_TEST_SELECTION_COMMITTED')
            key('m');move(480,260);click_button(1,True);move(540,330);click_button(1,False)
            wait(lambda: event_marker_count('UI_TEST_SELECTION_COMMITTED') > selection_commits
                 and (event_marker('UI_TEST_SELECTION_COMMITTED') or '').startswith('true '),
                 'the real mouse drag commits a document-space selection before layer editing')
            assert '*' not in title(window), 'selection alone changed artwork'
            source_id=native_state(project)['selected']
            assert layer_front_row_id()==source_id, 'the selected layer row and document active layer must agree'
            # Esc cancels a provisional canvas move without altering the saved
            # artwork; a second drag commits the selected pixels as one edit.
            move(500,280);click_button(1,True);move(530,310)
            key('Escape')
            wait(lambda: event_marker('UI_TEST_SELECTION_DRAG_CANCELLED')=='escape',
                 'Escape cancels a selected-pixel move preview')
            click_button(1,False)
            assert project.read_bytes()==saved and '*' not in title(window), \
                'cancelling the selection move must leave artwork and save state unchanged'
            drag_commit_count=sum(line.startswith('UI_TEST_SELECTION_DRAG_COMMITTED: ')
                                  for line in (directory/'application.log').read_text().splitlines())
            move(500,280);click_button(1,True);move(530,310);click_button(1,False)
            wait(lambda: sum(line.startswith('UI_TEST_SELECTION_DRAG_COMMITTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>
                 drag_commit_count,
                 'dragging inside the selection moves the selected raster pixels')
            wait(lambda: '*' in title(window),
                 'moving selected pixels marks the native artwork dirty')
            ctrl('s')
            wait(lambda: valid_project(project) and project.read_bytes()!=saved
                 and '*' not in title(window),
                 'the moved pixel region saves to the native document')
            moved_selection_bytes=project.read_bytes()
            assert native_layer_tiles_bytes(moved_selection_bytes)!=native_layer_tiles_bytes(saved), \
                'the selection drag must change actual raster-layer tile data'
            capture_window(window,'selection-move-canvas')
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==saved,
                 'Undo restores the exact pre-move artwork bytes')
            ctrl('y');ctrl('s')
            wait(lambda: project.read_bytes()==moved_selection_bytes,
                 'Redo restores the moved pixel data byte for byte')
            # The move clears the active selection. Re-select its translated
            # screen-space rectangle before exercising the layer-copy workflow.
            selection_commits=event_marker_count('UI_TEST_SELECTION_COMMITTED')
            handle_reports=event_marker_count('UI_TEST_SELECTION_TRANSFORM_HANDLES')
            key('m');move(510,290);click_button(1,True);move(570,360);click_button(1,False)
            wait(lambda: event_marker_count('UI_TEST_SELECTION_COMMITTED') > selection_commits
                 and (event_marker('UI_TEST_SELECTION_COMMITTED') or '').startswith('true '),
                 'the moved artwork can be selected again for layer actions')
            before_copy=project.read_bytes()
            handles=wait(selection_transform_handles,
                         'selected artwork exposes visible scale and rotation handles')
            wait(lambda: event_marker_count('UI_TEST_SELECTION_TRANSFORM_HANDLES') > handle_reports,
                 'the newly selected artwork reports fresh transform-handle coordinates')
            handles=selection_transform_handles()
            assert len(handles)==5 and handles[4]>0
            scale_handle=(handles[0],handles[1],handles[4])
            # A canceled scale preview is geometry-only and cannot enter history.
            transform_starts=event_marker_count('UI_TEST_SELECTION_TRANSFORM_STARTED')
            transform_cancels=event_marker_count('UI_TEST_SELECTION_TRANSFORM_CANCELLED')
            move(round(scale_handle[0]),round(scale_handle[1]));click_button(1,True)
            wait(lambda: event_marker_count('UI_TEST_SELECTION_TRANSFORM_STARTED') > transform_starts,
                 'the scale handle starts a fresh selection transform transaction')
            move(round(scale_handle[0]+12*scale_handle[2]),
                 round(scale_handle[1]+12*scale_handle[2]))
            key('Escape')
            wait(lambda: event_marker_count('UI_TEST_SELECTION_TRANSFORM_CANCELLED') > transform_cancels
                 and event_marker('UI_TEST_SELECTION_TRANSFORM_CANCELLED')=='escape',
                 'Escape cancels a proportional scale preview')
            click_button(1,False)
            unchanged=project.read_bytes()==before_copy
            clean='*' not in title(window)
            assert unchanged and clean, \
                f'canceling a scale preview must preserve document bytes and dirty state: bytes_unchanged={unchanged} title={title(window)!r}'

            def apply_selection_handle(kind, dx, dy, baseline, message):
                current_handles=wait(selection_transform_handles,
                                     'selection transform handles return after history restore')
                x,y,rx,ry,handle_scale=current_handles
                point=(x,y) if kind=='Scale' else (rx,ry)
                commit_count=sum(line.startswith('UI_TEST_SELECTION_TRANSFORM_COMMITTED: ')
                                 and f' {kind} ' in line
                                 for line in (directory/'application.log').read_text().splitlines())
                move(round(point[0]),round(point[1]));click_button(1,True)
                move(round(point[0]+dx*handle_scale),
                     round(point[1]+dy*handle_scale))
                click_button(1,False)
                wait(lambda: sum(line.startswith('UI_TEST_SELECTION_TRANSFORM_COMMITTED: ')
                                 and f' {kind} ' in line
                                 for line in (directory/'application.log').read_text().splitlines())>
                     commit_count,
                     message)
                wait(lambda: '*' in title(window),
                     f'{kind.lower()} changes selected document pixels')
                ctrl('s')
                wait(lambda: valid_project(project) and project.read_bytes()!=baseline
                     and '*' not in title(window),
                     f'{kind.lower()} selection transform saves native pixels')
                changed=project.read_bytes()
                ctrl('z');ctrl('s')
                wait(lambda: project.read_bytes()==baseline,
                     f'Undo restores exact pixels before {kind.lower()}')
                ctrl('y');ctrl('s')
                wait(lambda: project.read_bytes()==changed,
                     f'Redo restores exact pixels after {kind.lower()}')
                ctrl('z');ctrl('s')
                wait(lambda: project.read_bytes()==baseline,
                     f'final Undo restores selection and document for next workflow')
                return changed

            scale_bytes=apply_selection_handle(
                'Scale',18,12,before_copy,'dragging the lower-right handle commits proportional scale')
            assert native_layer_tiles_bytes(scale_bytes)!=native_layer_tiles_bytes(before_copy), \
                'scaling must change actual raster tile data'
            rotate_bytes=apply_selection_handle(
                'Rotate',20,12,before_copy,'dragging the top handle commits rotation')
            assert native_layer_tiles_bytes(rotate_bytes)!=native_layer_tiles_bytes(before_copy), \
                'rotation must change actual raster tile data'
            capture_window(window,'selection-scale-rotate-workspace')
            more=wait(layer_more_button_rect,'selected layer exposes its contextual editing actions')
            opened=layer_actions_open_count()
            click_at((more[0]+more[2]/2)*more[4],(more[1]+more[3]/2)*more[4])
            wait(lambda: layer_actions_open_count()>opened,
                 'the selected source layer Actions button receives the real click')
            opened_layer=event_marker('UI_TEST_LAYER_ACTIONS_OPENED')
            assert opened_layer==str(source_id), \
                f'layer Actions opened for {opened_layer}, while active selected layer is {source_id}'
            copy_action=wait(layer_copy_selection_menu_rect,
                             'layer Actions exposes Copy selected pixels to new layer')
            click_at((copy_action[0]+copy_action[2]/2)*copy_action[4],
                     (copy_action[1]+copy_action[3]/2)*copy_action[4])
            wait(lambda: event_marker('UI_TEST_LAYER_COPY_SELECTION_CLICKED')==str(source_id),
                 'the real layer menu accepts the selection-copy command')
            wait(lambda: event_marker('UI_TEST_LAYER_SELECTION_COPIED') is not None,
                 'the document engine creates a new layer from the selected source pixels')
            wait(lambda: '*' in title(window),
                 'selection copying marks the native document as unsaved')
            ctrl('s')
            wait(lambda: valid_project(project) and project.read_bytes()!=before_copy
                 and '*' not in title(window),
                 'the copied selection is saved to the native project')
            copy_bytes=project.read_bytes()
            copy_state=native_state_bytes(copy_bytes)
            copied_id=copy_state['selected']
            copied_layer=next(layer for layer in copy_state['layers']
                              if layer['id']==copied_id)
            before_copy_state=native_state_bytes(before_copy)
            print(f'UI_TEST_LAYER_COPY_STATE: before={[layer["id"] for layer in before_copy_state["layers"]]} '
                  f'after={[layer["id"] for layer in copy_state["layers"]]} selected={copied_id}', flush=True)
            assert len(copy_state['layers'])==len(before_copy_state['layers'])+1, \
                f'copy selected pixels must add exactly one editable layer: before={len(before_copy_state["layers"])} after={len(copy_state["layers"])}'
            assert copied_id!=source_id and copy_state['layers'][-1]['id']==copied_id, \
                'the new copied layer is inserted above its source and selected'
            before_tiles=native_layer_tiles_bytes(before_copy)
            after_tiles=native_layer_tiles_bytes(copy_bytes)
            assert after_tiles.get(source_id,{})==before_tiles.get(source_id,{}), \
                'copying selected artwork must preserve every source-layer tile byte'
            copied_tiles=after_tiles.get(copied_id,{})
            copied_pixel_count=0
            for tile_key,copied_tile in copied_tiles.items():
                source_tile=before_tiles.get(source_id,{}).get(tile_key,bytes(16384))
                for pixel_offset in range(0,len(copied_tile),4):
                    copied_pixel=copied_tile[pixel_offset:pixel_offset+4]
                    if copied_pixel[3]:
                        assert copied_pixel==source_tile[pixel_offset:pixel_offset+4], \
                            'the copied layer must preserve source premultiplied RGBA exactly'
                        copied_pixel_count+=1
            assert copied_pixel_count>0, 'the XTest selection must intersect existing artwork'
            capture_window(window,'layers-panel-copy-selection')
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==before_copy,
                 'Undo removes the copied layer and preserves its source artwork exactly')
            ctrl('y');ctrl('s')
            wait(lambda: project.read_bytes()==copy_bytes,
                 'Redo restores the copied layer and its selected pixels exactly')

            more=wait(layer_more_button_rect,
                      'the copied layer exposes the selection move action')
            opened=layer_actions_open_count()
            click_at((more[0]+more[2]/2)*more[4],(more[1]+more[3]/2)*more[4])
            wait(lambda: layer_actions_open_count()>opened,
                 'the copied layer Actions menu opens before moving pixels')
            move_action=wait(layer_move_selection_menu_rect,
                             'layer Actions exposes Move selected pixels to new layer')
            click_at((move_action[0]+move_action[2]/2)*move_action[4],
                     (move_action[1]+move_action[3]/2)*move_action[4])
            wait(lambda: event_marker('UI_TEST_LAYER_SELECTION_MOVED') is not None,
                 'the document engine moves the selected pixels transactionally')
            ctrl('s')
            wait(lambda: valid_project(project) and project.read_bytes()!=copy_bytes
                 and '*' not in title(window),
                 'moved pixels are saved to the native project')
            move_bytes=project.read_bytes()
            move_state=native_state_bytes(move_bytes)
            moved_id=move_state['selected']
            moved_tiles=native_layer_tiles_bytes(move_bytes)
            assert moved_id!=copied_id and len(move_state['layers'])==len(copy_state['layers'])+1
            assert not moved_tiles.get(copied_id,{}), \
                'moving the selected copied pixels must clear their original layer'
            assert moved_tiles.get(moved_id,{})==copied_tiles, \
                'the moved layer must preserve the exact premultiplied source pixels'
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==copy_bytes,
                 'Undo restores the original copied-layer pixels and ordering exactly')

            header=scroll_layers_marker_into_view(
                layer_settings_header,
                'selected layer settings remain reachable in Layers',
            )
            if header[0]!='true':
                click_at((float(header[1])+float(header[3])/2)*float(header[5]),
                         (float(header[2])+float(header[4])/2)*float(header[5]))
            wait(lambda: (lambda value: value if value and value[0]=='true' else None)(layer_settings_header()),
                 'the selected layer opacity and blend controls expand on demand')
            blend_control=scroll_layers_marker_into_view(
                layer_blend_combo_rect,
                'scrolling Layers brings the blend-mode control into view',
            )
            opacity_control=wait(layer_opacity_slider_rect,
                                 'the selected layer exposes its real opacity slider')
            property_before=event_marker('UI_TEST_LAYER_COMPOSITING_APPLIED')
            capture_window(window,'layers-panel-settings-expanded')
            opacity_target=(opacity_control[0]+opacity_control[2]*0.68,
                            opacity_control[1]+opacity_control[3]/2)
            click_at(opacity_target[0]*opacity_control[4],
                     opacity_target[1]*opacity_control[4])
            wait(lambda: (lambda marker: marker if marker and marker!=property_before
                          and marker.startswith(f'{copied_id} opacity=') else None)(
                              event_marker('UI_TEST_LAYER_COMPOSITING_APPLIED')),
                 'moving the layer opacity slider changes actual document compositing state')
            wait(lambda: '*' in title(window),
                 'changing layer opacity marks the native document as unsaved')
            ctrl('s')
            wait(lambda: valid_project(project) and project.read_bytes()!=copy_bytes
                 and '*' not in title(window),
                 'the adjusted layer opacity is saved to the native project')
            opacity_bytes=project.read_bytes()
            opacity_layer=next(layer for layer in native_state_bytes(opacity_bytes)['layers']
                               if layer['id']==copied_id)
            assert 0<opacity_layer['opacity']<255, \
                f'the opacity control did not set a partial layer opacity: {opacity_layer["opacity"]}'
            undo_before=event_marker('UI_TEST_HISTORY_UNDO')
            ctrl('z')
            wait(lambda: (lambda marker: marker if marker and marker!=undo_before else None)(
                    event_marker('UI_TEST_HISTORY_UNDO')),
                 'Ctrl+Z remains available after adjusting the layer-opacity slider')
            undo_state=event_marker('UI_TEST_HISTORY_UNDO')
            assert undo_state.split()[-1]==str(copied_layer['opacity']), \
                f'Undo should restore the copied layer opacity to {copied_layer["opacity"]}, got {undo_state}'
            ctrl('s')
            wait(lambda: project.read_bytes()==copy_bytes,
                 'Undo restores the prior layer opacity and exact copied document')
            redo_before=event_marker('UI_TEST_HISTORY_REDO')
            ctrl('y')
            wait(lambda: (lambda marker: marker if marker and marker!=redo_before else None)(
                    event_marker('UI_TEST_HISTORY_REDO')),
                 'Ctrl+Y remains available after adjusting the layer-opacity slider')
            redo_state=event_marker('UI_TEST_HISTORY_REDO')
            assert redo_state.split()[-1]==str(opacity_layer['opacity']), \
                f'Redo should restore the edited opacity, got {redo_state}'
            ctrl('s')
            wait(lambda: project.read_bytes()==opacity_bytes,
                 'Redo restores the saved layer opacity exactly')

            blend_control=scroll_layers_marker_into_view(
                layer_blend_combo_rect,
                'the selected layer exposes its blend-mode selector',
            )
            click_at((blend_control[0]+blend_control[2]/2)*blend_control[4],
                     (blend_control[1]+blend_control[3]/2)*blend_control[4])
            multiply=wait(lambda: layer_blend_option_rect('Multiply'),
                          'Multiply is available in the actual blend-mode menu')
            click_at((multiply[0]+multiply[2]/2)*multiply[4],
                     (multiply[1]+multiply[3]/2)*multiply[4])
            wait(lambda: (lambda marker: marker if marker and
                          marker.endswith(f'opacity={opacity_layer["opacity"]} blend=Multiply') else None)(
                              event_marker('UI_TEST_LAYER_COMPOSITING_APPLIED')),
                 'choosing Multiply changes the selected layer compositing mode')
            wait(lambda: '*' in title(window),
                 'changing layer blend mode marks the native document as unsaved')
            ctrl('s')
            wait(lambda: valid_project(project) and project.read_bytes()!=opacity_bytes
                 and '*' not in title(window),
                 'the selected layer blend mode is saved to the native project')
            blend_bytes=project.read_bytes()
            blend_layer=next(layer for layer in native_state_bytes(blend_bytes)['layers']
                             if layer['id']==copied_id)
            assert blend_layer['blend']=='Multiply' and blend_layer['opacity']==opacity_layer['opacity'], \
                f'blend settings did not persist together: {blend_layer}'
            capture_window(window,'layers-panel-compositing-settings')
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==opacity_bytes,
                 'Undo restores the prior blend mode without losing layer opacity')
            ctrl('y');ctrl('s')
            wait(lambda: project.read_bytes()==blend_bytes,
                 'Redo restores the saved blend mode and opacity exactly')

            # Make the mask paint test's selection state explicit. The selected
            # box covers the copied artwork and the later brush dab stays inside.
            ctrl('d')
            key('m')
            selection_commits=sum(line.startswith('UI_TEST_SELECTION_COMMITTED: ')
                                  for line in (directory/'application.log').read_text().splitlines())
            drag_at(500,280,580,360)
            wait(lambda: sum(line.startswith('UI_TEST_SELECTION_COMMITTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>
                 selection_commits,
                 'the mask workflow establishes a fresh bounded selection through the canvas')
            assert (event_marker('UI_TEST_SELECTION_COMMITTED') or '').startswith('true '), \
                'the mask test selection must be committed before painting'
            key('b')

            add_mask=scroll_layers_marker_into_view(
                layer_mask_add_rect,
                'Selected layer settings exposes the direct Add white mask control',
            )
            click_at((add_mask[0]+add_mask[2]/2)*add_mask[4],
                     (add_mask[1]+add_mask[3]/2)*add_mask[4])
            wait(lambda: event_marker('UI_TEST_LAYER_MASK_ADDED')==str(copied_id),
                 'the selected-layer control adds a non-destructive mask')
            ctrl('s')
            wait(lambda: valid_project(project) and project.read_bytes()!=blend_bytes
                 and project.read_bytes()[:8]==b'RPLALP02' and '*' not in title(window),
                 'the masked project is saved in v2 without changing the v1 baseline')
            mask_added=project.read_bytes()
            mask_state=native_state_bytes(mask_added)
            mask_layer=next(layer for layer in mask_state['layers'] if layer['id']==copied_id)
            assert mask_layer['mask']['enabled'] and not mask_layer['mask']['inverted']
            assert native_layer_mask_pixel_bytes(mask_added,copied_id,0,0)==255, \
                'missing mask samples must remain fully revealing'
            capture_window(window,'layers-panel-white-mask-settings')

            def click_mask_control(marker_fn, description):
                control=scroll_layers_marker_into_view(marker_fn,description)
                target=((control[0]+control[2]/2)*control[4],
                        (control[1]+control[3]/2)*control[4])
                print(f'UI_TEST_MASK_CONTROL_CLICK: {description} '
                      f'rect={control[:4]} target={target}',flush=True)
                click_at(*target)
                return control

            click_mask_control(
                layer_mask_edit_rect,
                'Selected layer settings exposes the direct Edit mask with Brush control',
            )
            wait(lambda: event_marker('UI_TEST_LAYER_MASK_EDIT_CLICKED')==f'{copied_id} true',
                 'the direct mask control receives its real pointer click')
            wait(lambda: event_marker('UI_TEST_LAYER_MASK_EDITING')==f'{copied_id} true',
                 'the selected layer enters native brush mask-paint mode')
            capture_window(window,'layers-panel-mask-editing')
            before_mask_pixels=native_layer_tiles_bytes(mask_added)
            # Editing a mask opens Brush Library so its current settings are
            # immediately available. Dismiss that floating panel before the
            # routed canvas pointer stroke; it otherwise owns the test point.
            key('Escape')
            wait(lambda: event_marker('UI_TEST_ESCAPE_DISMISSED')=='brush',
                 'closing Brush Library exposes the canvas without leaving mask-edit mode')
            assert event_marker('UI_TEST_LAYER_MASK_EDITING')==f'{copied_id} true', \
                'dismissing Brush Library must preserve the active mask target'
            # The selection used to create the copied layer is still active.
            # Paint inside it so the mask transaction exercises selection
            # clipping while remaining clear of the Layers panel at x < 368.
            move(540,325);click_button(1,True);click_button(1,False)
            wait(lambda: '*' in title(window),'mask brush stroke changes the native document')
            ctrl('s')
            wait(lambda: valid_project(project) and project.read_bytes()!=mask_added
                 and '*' not in title(window),'mask pixels are saved into the v2 tile section')
            mask_painted=project.read_bytes()
            _,raster_layers,mask_tiles=native_document_tables(mask_painted)
            assert raster_layers==native_layer_tiles_bytes(mask_added), \
                'painting a mask must not rewrite the source raster layer pixels'
            painted_mask=mask_tiles.get(copied_id,{})
            assert painted_mask and any(value<255 for tile in painted_mask.values() for value in tile), \
                'the real mouse stroke must write non-white alpha-mask samples'
            capture_window(window,'layers-panel-painted-mask')
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==mask_added,
                 'Undo removes mask-paint samples and preserves the editable mask')
            ctrl('y');ctrl('s')
            wait(lambda: project.read_bytes()==mask_painted,
                 'Redo restores mask-paint samples exactly')

            # Reopen the actual masked v2 project through the app's file picker.
            ctrl('o');path_prompt();ctrl('Return')
            wait(lambda: valid_project(project) and project.read_bytes()==mask_painted,
                 'the application reopens the complete editable v2 mask project')

            def verify_mask_edit(marker_name, expected_event, marker_fn, description, validate):
                before=project.read_bytes()
                previous_count=event_marker_count(marker_name)
                click_mask_control(marker_fn,description)
                outcome=wait(lambda: (lambda value: value if event_marker_count(marker_name)>previous_count
                                      and value else None)(event_marker(marker_name)),
                             description+' changes the document')
                assert outcome==expected_event(outcome), \
                    f'{description} targeted unexpected state: {outcome}'
                ctrl('s')
                wait(lambda: valid_project(project) and project.read_bytes()!=before
                     and '*' not in title(window),description+' saves to the native project')
                after=project.read_bytes()
                validate(before,after)
                ctrl('z');ctrl('s')
                wait(lambda: project.read_bytes()==before,
                     'Undo restores the exact pre-action mask project')
                ctrl('y');ctrl('s')
                wait(lambda: project.read_bytes()==after,
                     'Redo restores the exact saved mask action')
                ctrl('z');ctrl('s')
                wait(lambda: project.read_bytes()==before,
                     'the mask action is undone before the next control check')

            raster_before_mask_controls=native_layer_tiles_bytes(mask_painted)
            def validate_disabled(before, after):
                state=native_state_bytes(after)
                layer=next(item for item in state['layers'] if item['id']==copied_id)
                assert not layer['mask']['enabled']
                assert native_document_tables(after)[1]==native_layer_tiles_bytes(before)
                assert native_document_tables(after)[2]==native_document_tables(before)[2]
            verify_mask_edit(
                'UI_TEST_LAYER_MASK_ENABLED',lambda value:f'{copied_id} false',
                layer_mask_enabled_rect,'Disabling the layer mask',validate_disabled,
            )

            # Exercise the reverse checkbox transition from the disabled state as
            # well. Save each state and walk history both directions byte-exactly.
            enabled_before=project.read_bytes()
            disabled_events=event_marker_count('UI_TEST_LAYER_MASK_ENABLED')
            click_mask_control(layer_mask_enabled_rect,'The mask-enabled checkbox can disable the mask')
            wait(lambda: event_marker_count('UI_TEST_LAYER_MASK_ENABLED')>disabled_events
                 and event_marker('UI_TEST_LAYER_MASK_ENABLED')==f'{copied_id} false',
                 'the mask-enabled checkbox disables the mask through its real UI control')
            ctrl('s')
            wait(lambda: project.read_bytes()!=enabled_before and '*' not in title(window),
                 'disabling the mask saves its native metadata')
            disabled_again=project.read_bytes()
            validate_disabled(enabled_before,disabled_again)

            enabled_events=event_marker_count('UI_TEST_LAYER_MASK_ENABLED')
            click_mask_control(layer_mask_enabled_rect,'Disabled mask can be enabled again')
            wait(lambda: event_marker_count('UI_TEST_LAYER_MASK_ENABLED')>enabled_events
                 and event_marker('UI_TEST_LAYER_MASK_ENABLED')==f'{copied_id} true',
                 'the mask-enabled checkbox restores the mask through its real UI control')
            ctrl('s')
            wait(lambda: project.read_bytes()!=disabled_again and '*' not in title(window),
                 're-enabling the mask saves its native metadata')
            enabled_again=project.read_bytes()
            enabled_state=native_state_bytes(enabled_again)
            enabled_layer=next(item for item in enabled_state['layers'] if item['id']==copied_id)
            assert enabled_layer['mask']['enabled']
            assert native_document_tables(enabled_again)[2]==native_document_tables(enabled_before)[2]
            assert native_document_tables(enabled_again)[1]==native_layer_tiles_bytes(enabled_before)
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==disabled_again,
                 'Undo disables the mask again after its explicit re-enable')
            ctrl('y');ctrl('s')
            wait(lambda: project.read_bytes()==enabled_again,
                 'Redo restores the explicitly re-enabled mask exactly')
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==disabled_again,
                 'Undo restores the disabled state exactly')
            ctrl('z');ctrl('s')
            wait(lambda: project.read_bytes()==enabled_before,
                 'Undo of the original disable restores the painted enabled mask')

            def validate_inverted(before, after):
                state=native_state_bytes(after)
                layer=next(item for item in state['layers'] if item['id']==copied_id)
                assert layer['mask']['enabled'] and layer['mask']['inverted']
                assert native_document_tables(after)[2]==native_document_tables(before)[2]
                assert native_document_tables(after)[1]==native_layer_tiles_bytes(before)
            verify_mask_edit(
                'UI_TEST_LAYER_MASK_INVERTED',lambda value:f'{copied_id} true',
                layer_mask_invert_rect,'Inverting the editable layer mask',validate_inverted,
            )

            def validate_cleared(before, after):
                state=native_state_bytes(after)
                layer=next(item for item in state['layers'] if item['id']==copied_id)
                assert layer['mask']['enabled'] and not layer['mask']['inverted']
                assert native_document_tables(after)[2].get(copied_id,{})=={}
                assert native_document_tables(after)[1]==native_layer_tiles_bytes(before)
            verify_mask_edit(
                'UI_TEST_LAYER_MASK_CLEARED',lambda value:f'{copied_id}',
                layer_mask_clear_rect,'Clearing the layer mask to white',validate_cleared,
            )

            def validate_removed(before, after):
                assert after[:8]==b'RPLALP01', \
                    'removing the only mask should return to the compatible v1 container'
                assert next(item for item in native_state_bytes(after)['layers']
                            if item['id']==copied_id).get('mask') is None
                assert native_layer_tiles_bytes(after)==native_layer_tiles_bytes(before)
            verify_mask_edit(
                'UI_TEST_LAYER_MASK_REMOVED',lambda value:f'{copied_id}',
                layer_mask_remove_rect,'Removing the editable layer mask',validate_removed,
            )
            assert native_layer_tiles_bytes(project.read_bytes())==raster_before_mask_controls
            key('m');key('b')
            saved=mask_painted

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
            wait(lambda: event_marker('UI_TEST_OPENRASTER_EXPORT_STARTED') is not None,
                 'OpenRaster export shortcut reaches the application')
            outcome=wait(lambda: event_marker('UI_TEST_OPENRASTER_EXPORT_COMPLETED')
                         or event_marker('UI_TEST_OPENRASTER_EXPORT_FAILED'),
                         'layered OpenRaster export finishes',seconds=120)
            assert event_marker('UI_TEST_OPENRASTER_EXPORT_FAILED') is None, \
                f'OpenRaster export failed: {outcome}'
            assert ora.exists(), 'OpenRaster export reported completion without publishing its file'
            assert event_marker('UI_TEST_OPENRASTER_MASKS_FLATTENED')=='true', \
                'OpenRaster export must disclose that native editable masks are flattened'
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
            capture_window(window,'recovery-manager-preview-target')
            left,top,width,height,scale,content_top=preview_target
            application_log=directory/'application.log'
            hover_count=sum(line.startswith('UI_TEST_RECOVERY_PREVIEW_HOVERED')
                            for line in application_log.read_text().splitlines())
            click_x=round((left+width/2)*scale)
            click_y=None
            # Window-manager client offsets can differ from egui's viewport
            # origin. Probe a bounded vertical range and click only after the
            # real egui button reports hover, instead of assuming a decoration
            # offset from the screenshot dimensions.
            for offset in range(-80,81,4):
                candidate_y=round((top+height/2)*scale)+offset
                move(click_x,candidate_y)
                hovered=sum(line.startswith('UI_TEST_RECOVERY_PREVIEW_HOVERED')
                            for line in application_log.read_text().splitlines())
                if hovered>hover_count:
                    click_y=candidate_y
                    break
            assert click_y is not None, \
                f'pointer movement did not reach the recovery preview button: target={preview_target}, hover markers={hover_count}->{hovered}'
            print(f'UI_TEST_RECOVERY_PREVIEW_HIT: {click_x} {click_y}', flush=True)
            click_button(1,True);click_button(1,False)
            wait(lambda: 'UI_TEST_RECOVERY_PREVIEW_VISIBLE' in (directory/'application.log').read_text(),
                 'recovery manager renders the requested native artwork preview')
            time.sleep(0.25)
            capture_window(window,'recovery-manager-populated-light')
            resize_window(window,640,480);capture_window(window,'recovery-manager-populated-light-narrow')
            resize_window(window,1280,800);ctrl_shift('m')
            recovery_preview_fixture.unlink(missing_ok=True)
            recovery_preview_fixture.with_suffix('.owner').unlink(missing_ok=True)
            ctrl('s')
            wait(lambda: bool(valid_project(imported)) and '*' not in title(window),'OpenRaster reopened and saved as editable native project')
            # Remove utility panels before the canvas-only shape and gradient
            # gestures. XTest must never rely on drawing through floating UI.
            reference_layouts_before_canvas=json.loads(prefs.read_text()).get('references',[])
            if layers_panel_open():
                layer_window=wait(layers_window_rect,'Layers close control has current geometry')
                click_at((layer_window[0]+layer_window[2]-28)*layer_window[4],
                         (layer_window[1]+28)*layer_window[4])
                wait(lambda: layers_panel_open() is False,
                     'closing Layers returns its area to the canvas')
            for _ in range(8):
                open_references=reference_panel_count() or 0
                if open_references==0:
                    break
                reference_window=wait(reference_last_window_rect,
                                      'last reference window exposes its close control')
                click_at((reference_window[0]+reference_window[2]-26)*reference_window[4],
                         (reference_window[1]+26)*reference_window[4])
                wait(lambda expected=open_references-1: reference_panel_count()==expected,
                     'closing a reference window leaves other references independent')
            assert reference_panel_count()==0, 'canvas-only regression phase must close every reference window'
            # Shape tools use the actual pointer canvas and document raster
            # engine. Escape drops a live preview without mutating native data;
            # each committed primitive survives save/reopen and has one undo.
            shape_markers=lambda: [line.partition(': ')[2].split()
                                   for line in (directory/'application.log').read_text().splitlines()
                                   if line.startswith('UI_TEST_SHAPE_COMMITTED: ')]
            preview_base=imported.read_bytes()
            shape_before=len(shape_markers())
            key('r');wait(lambda: event_marker('UI_TEST_ACTIVE_TOOL')=='ShapeRectangle',
                           'R selects the document-backed rectangle shape tool')
            move(480,180);click_button(1,True);move(560,260)
            preview_bounds=wait(
                visible_shape_preview_bounds,
                'active shape drag reports its clipped preview geometry')
            capture_window(window,'shape-preview-rectangle')
            preview_width,preview_height,preview_pixels=decode_rgba_png(
                (Path(os.environ['RPL_UI_SCREENSHOT_DIR'])/'shape-preview-rectangle.png').read_bytes())
            left,top,right,bottom,preview_scale=preview_bounds
            left=max(0,int(left*preview_scale));top=max(0,int(top*preview_scale))
            right=min(preview_width,int(right*preview_scale));bottom=min(preview_height,int(bottom*preview_scale))
            contrasting_preview_pixels=sum(
                1 for y in range(top,bottom) for x_pixel in range(left,right)
                if max(preview_pixels[(y*preview_width+x_pixel)*4:(y*preview_width+x_pixel)*4+3])<64)
            assert contrasting_preview_pixels>=12, \
                f'live shape preview is not visibly contrasted over artwork: {contrasting_preview_pixels} dark pixels'
            key('Escape');click_button(1,False)
            assert imported.read_bytes()==preview_base,'Escape cancellation must discard the preview without changing artwork'
            assert len(shape_markers())==shape_before,'cancelled shape preview must not create a document command'
            for shortcut,kind,start,end in [
                ('r','Rectangle',(480,180),(560,260)),
                ('o','Ellipse',(480,320),(560,400)),
                ('l','Line',(480,470),(580,540)),
            ]:
                key(shortcut)
                wait(lambda expected=f'Shape{kind}': event_marker('UI_TEST_ACTIVE_TOOL')==expected,
                     f'{shortcut.upper()} selects the {kind.lower()} shape tool')
                before_shape=imported.read_bytes()
                prior_count=len(shape_markers())
                drag_at(*start,*end)
                wait(lambda: len(shape_markers())>prior_count,
                     f'XTest pointer drag commits a {kind.lower()} into the document engine')
                marker=shape_markers()[-1]
                assert marker[0]==kind, f'wrong document operation committed: {marker}'
                ctrl('s')
                wait(lambda: imported.read_bytes()!=before_shape and '*' not in title(window),
                     f'{kind.lower()} raster data is saved to the native project')
                sx,sy,ex,ey=map(float,marker[1:])
                if kind=='Line': sample_x,sample_y=(sx+ex)/2,(sy+ey)/2
                else: sample_x,sample_y=(sx+ex)/2,sy
                pixel=native_layer_pixel(imported,round(sample_x),round(sample_y))
                assert pixel[3]>0, f'{kind} outline is missing from the selected native layer at {sample_x:.1f},{sample_y:.1f}'
                before_pixel=native_layer_pixel_bytes(before_shape,round(sample_x),round(sample_y))
                assert pixel!=before_pixel, \
                    f'{kind} did not change the selected-layer pixel at {sample_x:.1f},{sample_y:.1f}'
                shape_bytes=imported.read_bytes()
                ctrl('z');ctrl('s')
                wait(lambda: imported.read_bytes()==before_shape,
                     f'{kind} is one exact undoable document transaction')
                ctrl('y');ctrl('s')
                wait(lambda: imported.read_bytes()==shape_bytes and '*' not in title(window),
                     f'{kind} redo restores the saved shape pixels exactly')
            quickshape_markers=lambda: [line.partition(': ')[2].split()
                                        for line in (directory/'application.log').read_text().splitlines()
                                        if line.startswith('UI_TEST_QUICKSHAPE_COMMITTED: ')]
            quickshape_rejections=sum(
                line.startswith('UI_TEST_QUICKSHAPE_REJECTED: ')
                for line in (directory/'application.log').read_text().splitlines())
            quickshape_base=imported.read_bytes()
            key('q');wait(lambda: event_marker('UI_TEST_ACTIVE_TOOL')=='QuickShape',
                           'Q selects the accessible QuickShape tool')
            move(480,290);click_button(1,True);move(520,310);key('Escape');click_button(1,False)
            assert imported.read_bytes()==quickshape_base, \
                'Escape during a QuickShape gesture must not mutate document pixels'
            drag_path([(480,300),(520,340),(480,340)])
            wait(lambda: sum(line.startswith('UI_TEST_QUICKSHAPE_REJECTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>
                 quickshape_rejections,
                 'unrecognized open polygon is safely rejected without document mutation')
            assert imported.read_bytes()==quickshape_base, \
                'unrecognized gesture must not create a document transaction'
            for shape_name,points in [
                ('Rectangle',[(700,180),(740,180),(780,180),(780,220),(780,260),
                              (740,260),(700,260),(700,220),(700,180)]),
                ('Ellipse',[(650+40*math.cos(i*math.tau/16),330+40*math.sin(i*math.tau/16))
                            for i in range(17)]),
                # Keep the guide line away from the earlier mouse demo stroke
                # and the ellipse preview so pixel-difference validation sees
                # newly painted samples rather than an existing opaque color.
                ('Line',[(500,450),(620,490),(740,530)]),
            ]:
                before_quickshape=imported.read_bytes()
                prior_quickshape_count=len(quickshape_markers())
                drag_path(points)
                wait(lambda: len(quickshape_markers())>prior_quickshape_count,
                     f'QuickShape recognizes a freehand {shape_name.lower()} gesture')
                marker=quickshape_markers()[-1]
                assert marker[0]==shape_name, f'QuickShape misclassified the gesture: {marker}'
                ctrl('s')
                wait(lambda: imported.read_bytes()!=before_quickshape and '*' not in title(window),
                     f'QuickShape {shape_name.lower()} commits and saves real layer pixels')
                changed_quickshape=imported.read_bytes()
                sx,sy,ex,ey=map(float,marker[1:])
                # Rasterized line coverage extends half a brush diameter
                # beyond its centerline bounds. Include that real stroke area
                # while keeping the assertion local to this one gesture.
                brush_radius=float(native_state_bytes(before_quickshape)['brush']['size'])/2.0+2.0
                changed_bounds=(min(sx,ex)-brush_radius,min(sy,ey)-brush_radius,
                                max(sx,ex)+brush_radius,max(sy,ey)+brush_radius)
                changed_pixels=native_layer_changed_pixels(
                    before_quickshape,changed_quickshape,changed_bounds)
                print(f'UI_TEST_QUICKSHAPE_PIXELS: {shape_name} {changed_pixels} '
                      f'bounds={changed_bounds}',flush=True)
                assert changed_pixels>0, \
                    f'QuickShape {shape_name.lower()} changes saved native-layer pixels in rasterized bounds {changed_bounds}'
                if shape_name=='Rectangle':
                    capture_window(window,'quick-shape-workspace')
                ctrl('z');ctrl('s')
                wait(lambda: imported.read_bytes()==before_quickshape,
                     f'QuickShape {shape_name.lower()} is one exact undoable transaction')
                ctrl('y');ctrl('s')
                wait(lambda: imported.read_bytes()==changed_quickshape,
                     f'QuickShape {shape_name.lower()} redo restores saved pixels exactly')
                ctrl('z');ctrl('s')
                wait(lambda: imported.read_bytes()==before_quickshape,
                     f'QuickShape {shape_name.lower()} cleanup restores prior artwork')
            # Editable vector paths stay structured in RPL v3 while the GPU,
            # PNG and OpenRaster consumers receive a derived raster composite.
            vector_markers=lambda: [line.partition(': ')[2].split()
                                    for line in (directory/'application.log').read_text().splitlines()
                                    if line.startswith('UI_TEST_VECTOR_PATH_CREATED: ')]
            vector_move_markers=lambda: [line.partition(': ')[2].split()
                                         for line in (directory/'application.log').read_text().splitlines()
                                         if line.startswith('UI_TEST_VECTOR_PATH_MOVED: ')]
            vector_before=imported.read_bytes()
            vector_raster_before=native_document_tables(vector_before)[1]
            vector_mask_before=native_document_tables(vector_before)[2]
            key('v')
            wait(lambda: event_marker('UI_TEST_ACTIVE_TOOL')=='VectorPath',
                 'V selects the editable vector path tool')
            capture_window(window,'vector-path-before')
            vector_before_image=(Path(os.environ['RPL_UI_SCREENSHOT_DIR'])/'vector-path-before.png').read_bytes()
            prior_vector_count=len(vector_markers())
            drag_path([(520,540),(560,570),(610,550)])
            wait(lambda: len(vector_markers())>prior_vector_count,
                 'a real canvas drag creates one editable vector path')
            capture_window(window,'vector-path-created')
            vector_after_image=(Path(os.environ['RPL_UI_SCREENSHOT_DIR'])/'vector-path-created.png').read_bytes()
            visible_vector_delta=screenshot_region_difference(
                vector_before_image,vector_after_image,(480,500,660,610))
            assert visible_vector_delta>=20, \
                f'vector path did not visibly render in the canvas preview: {visible_vector_delta} changed pixels'
            vector_created_marker=vector_markers()[-1]
            assert int(vector_created_marker[1])>=2, \
                f'editable vector gesture should preserve multiple anchor points: {vector_created_marker}'
            ctrl('s')
            wait(lambda: imported.exists() and imported.read_bytes()[:8]==b'RPLALP03'
                 and '*' not in title(window),
                 'the app saves editable paths in the version 3 native format')
            vector_created=imported.read_bytes()
            vector_state=native_state_bytes(vector_created)
            assert vector_state['next_vector_id']>0
            vector_layer=next(layer for layer in vector_state['layers']
                              if layer['id']==vector_state['selected'])
            assert len(vector_layer['vector_paths'])==1, \
                'new path remains an editable object in the selected layer metadata'
            vector_object=vector_layer['vector_paths'][0]
            assert vector_object['id']==int(vector_created_marker[0])
            assert len(vector_object['segments'])>=1
            assert native_document_tables(vector_created)[1]==vector_raster_before, \
                'vector creation must not write derived pixels into authoritative raster tiles'
            assert native_document_tables(vector_created)[2]==vector_mask_before, \
                'vector creation must preserve independent editable masks'
            ctrl('z');ctrl('s')
            wait(lambda: imported.read_bytes()==vector_before,
                 'vector creation is one exact undoable native-document transaction')
            ctrl('y');ctrl('s')
            wait(lambda: imported.read_bytes()==vector_created,
                 'redo restores the exact editable vector object and native bytes')
            opened_vector_count=sum(line.startswith('UI_TEST_PROJECT_OPENED: ')
                                    for line in (directory/'application.log').read_text().splitlines())
            ctrl('o');path_prompt();ctrl('Return')
            wait(lambda: sum(line.startswith('UI_TEST_PROJECT_OPENED: ')
                             for line in (directory/'application.log').read_text().splitlines())>
                 opened_vector_count,
                 'the application reopens the saved v3 project through its real Open workflow')
            opened_marker=event_marker('UI_TEST_PROJECT_OPENED')
            assert opened_marker and opened_marker.split()[-1]=='1', \
                f'reopened document must restore its editable path object: {opened_marker}'
            path_selection_count=event_marker_count('UI_TEST_VECTOR_PATH_SELECTED')
            click_at(585,560)
            selected_path=wait(
                lambda:(lambda marker:marker if marker and
                        event_marker_count('UI_TEST_VECTOR_PATH_SELECTED')>path_selection_count
                        else None)(event_marker('UI_TEST_VECTOR_PATH_SELECTED')),
                'the reopened editable vector path can be selected directly on the canvas')
            assert int(selected_path)==int(vector_created_marker[0])
            ctrl('l')
            wait(lambda:event_marker('UI_TEST_LAYERS_PANEL_STATE')=='true',
                 'the real Layers panel opens to verify vector thumbnail invalidation')
            selected_layer_id=native_state_bytes(imported.read_bytes())['selected']
            wait(lambda:layer_thumbnail_update(selected_layer_id),
                 'the reopened vector layer has a cached thumbnail before style editing')

            # The selected path exposes an accessible node editor. Dragging a
            # rendered anchor changes editable geometry while preserving its
            # raster/mask tables and creates one undoable transaction.
            node_edit_count=event_marker_count('UI_TEST_VECTOR_NODE_EDITING')
            node_handle_count=event_marker_count('UI_TEST_VECTOR_NODE_HANDLE')
            node_edit_rect=wait(
                lambda:stable_window_rect_marker('UI_TEST_VECTOR_EDIT_NODES_RECT'),
                'the selected vector path exposes its node editor action')
            click_at((node_edit_rect[0]+node_edit_rect[2]/2)*node_edit_rect[4],
                     (node_edit_rect[1]+node_edit_rect[3]/2)*node_edit_rect[4])
            wait(lambda:event_marker_count('UI_TEST_VECTOR_NODE_EDITING')>node_edit_count
                 and event_marker('UI_TEST_VECTOR_NODE_EDITING')
                 ==f'{vector_created_marker[0]} true',
                 'the selected path enters anchor-edit mode')
            node_handle=wait(
                lambda:(lambda marker:marker if marker
                        and event_marker_count('UI_TEST_VECTOR_NODE_HANDLE')>node_handle_count
                        else None)(vector_node_handle()),
                'anchor-edit mode draws a selectable point handle on the canvas')
            assert int(node_handle[0])==int(vector_created_marker[0])
            assert int(node_handle[1])==0
            width_control=wait(vector_width_rect,
                               'anchor editing keeps the vector stroke-width control available')
            current_window_size=window_size(window)
            assert current_window_size is not None
            assert width_control[0]>=0 and width_control[0]+width_control[2] \
                <=current_window_size[0]/width_control[4]+1, \
                f'anchor editing must not push the stroke-width control outside the window: {width_control} in {current_window_size}'
            capture_window(window,'vector-node-editor')
            node_before=imported.read_bytes()
            node_before_state=native_state_bytes(node_before)
            node_layer=next(layer for layer in node_before_state['layers']
                            if layer['id']==node_before_state['selected'])
            node_path_before=node_layer['vector_paths'][0]
            node_thumbnail_before=layer_thumbnail_update(node_layer['id'])
            node_move_count=event_marker_count('UI_TEST_VECTOR_NODE_MOVED')
            hx,hy=round(node_handle[2]*node_handle[4]),round(node_handle[3]*node_handle[4])
            drag_at(hx,hy,hx+12,hy+8)
            wait(lambda:event_marker_count('UI_TEST_VECTOR_NODE_MOVED')>node_move_count,
                 'dragging a canvas anchor commits one editable vector-node transaction')
            moved_node_marker=event_marker('UI_TEST_VECTOR_NODE_MOVED').split()
            assert int(moved_node_marker[0])==node_path_before['id']
            assert int(moved_node_marker[1])==0
            ctrl('s')
            wait(lambda:valid_project(imported) and imported.read_bytes()!=node_before
                 and '*' not in title(window),
                 'the moved vector anchor is saved in the native project')
            node_after=imported.read_bytes()
            node_after_state=native_state_bytes(node_after)
            node_layer_after=next(layer for layer in node_after_state['layers']
                                  if layer['id']==node_after_state['selected'])
            node_path_after=node_layer_after['vector_paths'][0]
            assert node_path_after['start']!=node_path_before['start']
            assert node_path_after['segments']==node_path_before['segments']
            assert native_document_tables(node_after)[1]==native_document_tables(node_before)[1]
            assert native_document_tables(node_after)[2]==native_document_tables(node_before)[2]
            wait(lambda:(lambda update:update and node_thumbnail_before
                         and update[0]!=node_thumbnail_before[0])(
                             layer_thumbnail_update(node_layer['id'])),
                 'moving a vector anchor invalidates the owning layer thumbnail')
            ctrl('z');ctrl('s')
            wait(lambda:imported.read_bytes()==node_before,
                 'Undo restores the exact pre-node-edit vector project')
            ctrl('y');ctrl('s')
            wait(lambda:imported.read_bytes()==node_after,
                 'Redo restores the saved vector anchor exactly')

            # Anchor insertion and removal are real editing operations, not
            # decorative node controls. Exercise their contextual popup with
            # XTest pointer input and validate saved geometry, raster/mask
            # isolation, thumbnail invalidation, and exact history replay.
            insertion_before=imported.read_bytes()
            insertion_before_state=native_state_bytes(insertion_before)
            insertion_layer=next(layer for layer in insertion_before_state['layers']
                                 if layer['id']==insertion_before_state['selected'])
            insertion_path=insertion_layer['vector_paths'][0]
            insertion_preview=layer_thumbnail_update(insertion_layer['id'])
            anchor_controls_count=event_marker_count('UI_TEST_VECTOR_ANCHOR_CONTROLS_RECT')
            anchor_controls=wait(
                lambda:(lambda rect:rect if rect and
                        event_marker_count('UI_TEST_VECTOR_ANCHOR_CONTROLS_RECT')>
                        anchor_controls_count else None)(vector_anchor_controls_rect()),
                'anchor editing exposes the accessible anchor selector')
            click_scaled_rect(anchor_controls)
            insert_rect_count=event_marker_count('UI_TEST_VECTOR_ANCHOR_INSERT_RECT')
            insert_rect=wait(
                lambda:(lambda rect:rect if rect and
                        event_marker_count('UI_TEST_VECTOR_ANCHOR_INSERT_RECT')>insert_rect_count
                        else None)(vector_anchor_action_rect('insert')),
                'the anchor selector exposes midpoint insertion')
            inserted_count=event_marker_count('UI_TEST_VECTOR_ANCHOR_INSERTED')
            click_scaled_rect(insert_rect)
            wait(lambda:event_marker_count('UI_TEST_VECTOR_ANCHOR_INSERTED')>inserted_count,
                 'midpoint insertion dispatches through the document edit transaction')
            ctrl('s')
            wait(lambda:valid_project(imported) and imported.read_bytes()!=insertion_before
                 and '*' not in title(window),
                 'the inserted vector anchor is saved in the native project')
            insertion_after=imported.read_bytes()
            insertion_after_state=native_state_bytes(insertion_after)
            insertion_after_layer=next(layer for layer in insertion_after_state['layers']
                                       if layer['id']==insertion_after_state['selected'])
            insertion_after_path=insertion_after_layer['vector_paths'][0]
            assert len(insertion_after_path['segments'])==len(insertion_path['segments'])+1
            first_segment=insertion_path['segments'][0]
            first_half=insertion_after_path['segments'][0]
            second_half=insertion_after_path['segments'][1]
            if first_segment.get('kind')=='line_to':
                first_endpoint=first_segment['point']
                midpoint=[(insertion_path['start'][axis]+first_endpoint[axis])*0.5
                          for axis in range(2)]
                assert first_half=={'kind':'line_to','point':midpoint}
                assert second_half==first_segment
            else:
                assert first_segment.get('kind')=='cubic_to', \
                    f'unsupported vector segment in live insertion fixture: {first_segment}'
                original_curve=first_segment
                start=insertion_path['start']
                control1=original_curve['control1']
                control2=original_curve['control2']
                endpoint=original_curve['point']
                midpoint_pair=lambda a,b:[(a[axis]+b[axis])*0.5 for axis in range(2)]
                q0=midpoint_pair(start,control1)
                q1=midpoint_pair(control1,control2)
                q2=midpoint_pair(control2,endpoint)
                r0=midpoint_pair(q0,q1)
                r1=midpoint_pair(q1,q2)
                midpoint=midpoint_pair(r0,r1)
                assert first_half=={'kind':'cubic_to',
                    'control1':q0,'control2':r0,'point':midpoint}
                assert second_half=={'kind':'cubic_to',
                    'control1':r1,'control2':q2,'point':endpoint}
            assert native_document_tables(insertion_after)[1] \
                ==native_document_tables(insertion_before)[1], \
                'inserting an editable vector anchor must not rasterize the path'
            assert native_document_tables(insertion_after)[2] \
                ==native_document_tables(insertion_before)[2], \
                'inserting an editable vector anchor must preserve independent masks'
            capture_window(window,'vector-node-editor-with-inserted-midpoint')
            wait(lambda:(lambda update:update and insertion_preview
                         and update[0]!=insertion_preview[0])(
                             layer_thumbnail_update(insertion_layer['id'])),
                 'inserting an anchor invalidates the owning layer thumbnail')
            ctrl('z');ctrl('s')
            wait(lambda:imported.read_bytes()==insertion_before,
                 'Undo removes the inserted anchor and restores the exact native document')
            ctrl('y');ctrl('s')
            wait(lambda:imported.read_bytes()==insertion_after,
                 'Redo restores the inserted anchor exactly')

            delete_rect_count=event_marker_count('UI_TEST_VECTOR_ANCHOR_DELETE_RECT')
            anchor_controls=wait(vector_anchor_controls_rect,
                                 'the selected midpoint remains available for deletion')
            click_scaled_rect(anchor_controls)
            delete_rect=wait(
                lambda:(lambda rect:rect if rect and
                        event_marker_count('UI_TEST_VECTOR_ANCHOR_DELETE_RECT')>delete_rect_count
                        else None)(vector_anchor_action_rect('delete')),
                'the midpoint exposes an enabled exact line-anchor deletion action')
            deleted_count=event_marker_count('UI_TEST_VECTOR_ANCHOR_DELETED')
            click_scaled_rect(delete_rect)
            wait(lambda:event_marker_count('UI_TEST_VECTOR_ANCHOR_DELETED')>deleted_count,
                 'anchor deletion dispatches through the document edit transaction')
            ctrl('s')
            wait(lambda:imported.read_bytes()==insertion_before and '*' not in title(window),
                 'deleting the midpoint restores the exact pre-insertion native document')
            ctrl('z');ctrl('s')
            wait(lambda:imported.read_bytes()==insertion_after,
                 'Undo restores the exact deleted midpoint')
            ctrl('y');ctrl('s')
            wait(lambda:imported.read_bytes()==insertion_before,
                 'Redo restores the exact line geometry after anchor deletion')

            # A line drawn in the application can be promoted to editable
            # cubic geometry in place; its initial one-third handles preserve
            # the straight appearance, after which the handle editor can bend it.
            curve_before=imported.read_bytes()
            assert curve_before==insertion_before
            # Explicitly leave and re-enter anchor editing so the selector is
            # known closed. Escape is also used by the workspace to dismiss
            # panels, so it is not a reliable way to reset popup state here.
            node_edit_count=event_marker_count('UI_TEST_VECTOR_NODE_EDITING')
            node_edit=wait(lambda:stable_window_rect_marker('UI_TEST_VECTOR_EDIT_NODES_RECT'),
                           'the vector path exposes its anchor editor toggle')
            click_scaled_rect(node_edit)
            wait(lambda:event_marker_count('UI_TEST_VECTOR_NODE_EDITING')>node_edit_count
                 and event_marker('UI_TEST_VECTOR_NODE_EDITING')
                 ==f"{insertion_path['id']} false",
                 'anchor editing closes before reopening the segment menu')
            node_edit_count=event_marker_count('UI_TEST_VECTOR_NODE_EDITING')
            node_edit=wait(lambda:stable_window_rect_marker('UI_TEST_VECTOR_EDIT_NODES_RECT'),
                           'the vector path exposes its anchor editor toggle')
            click_scaled_rect(node_edit)
            wait(lambda:event_marker_count('UI_TEST_VECTOR_NODE_EDITING')>node_edit_count
                 and event_marker('UI_TEST_VECTOR_NODE_EDITING')
                 ==f"{insertion_path['id']} true",
                 'anchor editing reopens with the segment menu closed')
            curve_rect_count=event_marker_count('UI_TEST_VECTOR_CURVE_SEGMENT_RECT')
            curve_anchor_menu=wait(
                vector_anchor_controls_rect,
                'the selected line path keeps its accessible anchor menu')
            click_scaled_rect(curve_anchor_menu)
            curve_rect=wait(
                lambda:(lambda rect:rect if rect and
                        event_marker_count('UI_TEST_VECTOR_CURVE_SEGMENT_RECT')>=
                        curve_rect_count+3 else None)(vector_curve_segment_rect()),
                'the selected line segment exposes its curve conversion action')
            curve_count=event_marker_count('UI_TEST_VECTOR_SEGMENT_CURVED')
            click_scaled_rect(curve_rect)
            wait(lambda:event_marker_count('UI_TEST_VECTOR_SEGMENT_CURVED')>curve_count,
                 'the curve action creates an editable cubic segment through the UI')
            ctrl('s')
            wait(lambda:valid_project(imported) and imported.read_bytes()!=curve_before
                 and '*' not in title(window),
                 'the editable cubic segment is saved in the native project')
            curve_after=imported.read_bytes()
            curve_after_state=native_state_bytes(curve_after)
            curve_layer=next(layer for layer in curve_after_state['layers']
                             if layer['id']==curve_after_state['selected'])
            curve_path=curve_layer['vector_paths'][0]
            curve_segment=curve_path['segments'][0]
            assert curve_segment['kind']=='cubic_to'
            start=curve_path['start']
            end=curve_segment['point']
            assert native_values_near(curve_segment['control1'],
                [start[axis]+(end[axis]-start[axis])/3.0 for axis in range(2)])
            assert native_values_near(curve_segment['control2'],
                [start[axis]+(end[axis]-start[axis])*2.0/3.0 for axis in range(2)])
            assert native_document_tables(curve_after)[1] \
                ==native_document_tables(curve_before)[1], \
                'converting a line to a vector curve must not write raster tiles'
            assert native_document_tables(curve_after)[2] \
                ==native_document_tables(curve_before)[2], \
                'converting a line to a vector curve must preserve editable masks'
            ctrl('z');ctrl('s')
            wait(lambda:imported.read_bytes()==curve_before,
                 'Undo restores the exact pre-conversion line path')
            ctrl('y');ctrl('s')
            wait(lambda:imported.read_bytes()==curve_after,
                 'Redo restores the exact editable cubic segment')

            # Load a checked native fixture containing a cubic segment so the
            # actual canvas handle editor is exercised with XTest, not just its
            # document API or Accessibility tree. The metadata rewrite keeps
            # all raster/mask tables and the existing native v3 checksum valid.
            cubic_state=native_state_bytes(insertion_before)
            cubic_layer=next(layer for layer in cubic_state['layers']
                             if layer['id']==cubic_state['selected'])
            cubic_path=cubic_layer['vector_paths'][0]
            cubic_start=cubic_path['start']
            cubic_end=cubic_path['segments'][0]['point']
            span=[cubic_end[axis]-cubic_start[axis] for axis in range(2)]
            cubic_path['segments'][0]={
                'kind':'cubic_to',
                'control1':[cubic_start[0]+span[0]*0.28,
                            cubic_start[1]-max(18.0,abs(span[0])*0.42)],
                'control2':[cubic_start[0]+span[0]*0.72,
                            cubic_end[1]+max(18.0,abs(span[0])*0.42)],
                'point':cubic_end,
            }
            cubic_fixture=replace_native_state(insertion_before,cubic_state)
            imported.write_bytes(cubic_fixture)
            assert native_document_tables(cubic_fixture)[1] \
                ==native_document_tables(insertion_before)[1]
            assert native_document_tables(cubic_fixture)[2] \
                ==native_document_tables(insertion_before)[2]
            opened_cubic_count=event_marker_count('UI_TEST_PROJECT_OPENED')
            ctrl('o');path_prompt();ctrl('Return')
            wait(lambda:event_marker_count('UI_TEST_PROJECT_OPENED')>opened_cubic_count,
                 'the live workspace reopens a valid native project with cubic geometry')
            ctrl('s')
            wait(lambda:valid_project(imported) and '*' not in title(window),
                 'the application canonicalizes the cubic fixture before handle editing')
            handle_before=imported.read_bytes()
            handle_before_state=native_state_bytes(handle_before)
            handle_layer=next(layer for layer in handle_before_state['layers']
                              if layer['id']==handle_before_state['selected'])
            handle_path_before=handle_layer['vector_paths'][0]
            handle_segment_before=handle_path_before['segments'][0]
            assert handle_segment_before['kind']=='cubic_to'
            handle_raster_before=native_document_tables(handle_before)[1]
            handle_masks_before=native_document_tables(handle_before)[2]

            # Open the path's accessible anchor controls, expand Bezier handles,
            # select the outgoing control and drag its visible canvas handle.
            key('b');key('v')
            path_select_count=event_marker_count('UI_TEST_VECTOR_PATH_SELECTED')
            click_at(585,560)
            selected_cubic=wait(
                lambda:(lambda marker:marker if marker and
                        event_marker_count('UI_TEST_VECTOR_PATH_SELECTED')>path_select_count
                        else None)(event_marker('UI_TEST_VECTOR_PATH_SELECTED')),
                'the reopened cubic path is selectable on the canvas')
            assert int(selected_cubic)==int(handle_path_before['id'])
            # Selecting the path adds contextual controls and can reflow the
            # canvas. A stationary click must not translate the path because
            # PointerUp should keep using the mapping captured at PointerDown.
            ctrl('s')
            wait(lambda:'*' not in title(window),
                 'selecting a vector path leaves the native document settled')
            selected_bytes=imported.read_bytes()
            selected_state=native_state_bytes(selected_bytes)
            selected_layer=next(layer for layer in selected_state['layers']
                                if layer['id']==selected_state['selected'])
            selected_path=next(path for path in selected_layer['vector_paths']
                               if path['id']==handle_path_before['id'])
            assert native_projects_equivalent(handle_before,selected_bytes), \
                'a stationary path-selection click must not translate geometry when contextual controls reflow'
            handle_before=selected_bytes
            handle_path_before=selected_path
            handle_segment_before=handle_path_before['segments'][0]
            handle_raster_before=native_document_tables(handle_before)[1]
            handle_masks_before=native_document_tables(handle_before)[2]
            edit_nodes_count=event_marker_count('UI_TEST_VECTOR_NODE_EDITING')
            edit_nodes=wait(lambda:stable_window_rect_marker('UI_TEST_VECTOR_EDIT_NODES_RECT'),
                            'the selected cubic path exposes its anchor editor')
            click_scaled_rect(edit_nodes)
            wait(lambda:event_marker_count('UI_TEST_VECTOR_NODE_EDITING')>edit_nodes_count
                 and event_marker('UI_TEST_VECTOR_NODE_EDITING')
                 ==f"{handle_path_before['id']} true",
                 'the cubic path enters its real anchor editing mode')
            anchor_menu=wait(vector_anchor_controls_rect,
                             'the cubic anchor editor exposes anchor selection')
            handles_section_count=event_marker_count('UI_TEST_VECTOR_HANDLES_SECTION_RECT')
            handle_selector_count=event_marker_count('UI_TEST_VECTOR_HANDLE_SELECTOR_RECT')
            click_scaled_rect(anchor_menu)
            handles_section=wait(
                lambda:(lambda rect:rect if rect and
                        event_marker_count('UI_TEST_VECTOR_HANDLES_SECTION_RECT')>
                        handles_section_count else None)(vector_handles_section_rect()),
                'the selected cubic anchor exposes its Bezier controls')
            if event_marker_count('UI_TEST_VECTOR_HANDLE_SELECTOR_RECT')<=handle_selector_count:
                click_scaled_rect(handles_section)
            outgoing_rect=wait(
                lambda:(lambda rect:rect if rect and
                        event_marker_count('UI_TEST_VECTOR_HANDLE_SELECTOR_RECT')>
                        handle_selector_count else None)(vector_handle_selector_rect('Outgoing')),
                                'the Bezier editor exposes its outgoing control selector')
            click_scaled_rect(outgoing_rect)
            control=wait(lambda:vector_control_handle('Outgoing',0),
                          'the selected cubic control is drawn as a movable canvas handle')
            capture_window(window,'vector-cubic-handles')
            handle_move_count=event_marker_count('UI_TEST_VECTOR_HANDLE_MOVED')
            hx,hy=round(control[0]*control[2]),round(control[1]*control[2])
            drag_at(hx,hy,hx+18,hy-14)
            wait(lambda:event_marker_count('UI_TEST_VECTOR_HANDLE_MOVED')>handle_move_count,
                 'dragging a Bezier control commits through the document transaction')
            ctrl('s')
            wait(lambda:valid_project(imported) and imported.read_bytes()!=handle_before
                 and '*' not in title(window),
                 'the moved Bezier control is saved in the editable native project')
            handle_after=imported.read_bytes()
            handle_after_state=native_state_bytes(handle_after)
            handle_layer_after=next(layer for layer in handle_after_state['layers']
                                    if layer['id']==handle_after_state['selected'])
            handle_path_after=handle_layer_after['vector_paths'][0]
            handle_segment_after=handle_path_after['segments'][0]
            assert handle_segment_after['kind']=='cubic_to'
            assert handle_segment_after['control1']!=handle_segment_before['control1'], \
                f"outgoing handle did not update control1: before={handle_segment_before} after={handle_segment_after} event={event_marker('UI_TEST_VECTOR_HANDLE_MOVED')}"
            assert native_values_near(handle_segment_after['control2'],
                                      handle_segment_before['control2'])
            assert native_values_near(handle_segment_after['point'],
                                      handle_segment_before['point'])
            assert native_values_near(handle_path_after['start'],handle_path_before['start'])
            assert native_values_near(handle_path_after['segments'][1:],
                                      handle_path_before['segments'][1:])
            assert native_document_tables(handle_after)[1]==handle_raster_before, \
                'editing cubic handles must not write derived geometry into raster tiles'
            assert native_document_tables(handle_after)[2]==handle_masks_before, \
                'editing cubic handles must preserve independent masks'
            ctrl('z');ctrl('s')
            wait(lambda:imported.read_bytes()==handle_before,
                 'Undo restores the exact native cubic geometry before handle movement')
            ctrl('y');ctrl('s')
            wait(lambda:imported.read_bytes()==handle_after,
                 'Redo restores the moved cubic handle exactly')

            # Width editing changes editable path metadata, leaves raster/mask
            # storage untouched, refreshes the layer preview, and commits one
            # undo transaction per gesture.
            def apply_vector_width(start_fraction, end_fraction):
                before=imported.read_bytes()
                before_state=native_state_bytes(before)
                before_layer=next(layer for layer in before_state['layers']
                                  if layer['id']==before_state['selected'])
                before_path=before_layer['vector_paths'][0]
                before_preview=layer_thumbnail_update(before_layer['id'])
                rect=wait(vector_width_rect,
                          'the selected vector exposes its stroke-width control')
                print(f'UI_TEST_VECTOR_STYLE_CONTROL: WIDTH rect={rect}',flush=True)
                before_count=event_marker_count('UI_TEST_VECTOR_STYLE_APPLIED')
                x,y,width,height,scale=rect
                drag_control_with_frame_ack(
                    (x+width*start_fraction)*scale,
                    (y+height*0.5)*scale,
                    (x+width*end_fraction)*scale,
                    (y+height*0.5)*scale,
                    'UI_TEST_VECTOR_SLIDER_POINTER',
                )
                wait(lambda:event_marker_count('UI_TEST_VECTOR_STYLE_APPLIED')>before_count,
                     'a completed width drag commits one vector-style transaction',seconds=30)
                assert event_marker_count('UI_TEST_VECTOR_STYLE_APPLIED')==before_count+1, \
                    'a width drag must create exactly one undo transaction'
                applied=event_marker('UI_TEST_VECTOR_STYLE_APPLIED').split()
                assert int(applied[0])==before_path['id']
                ctrl('s')
                wait(lambda:valid_project(imported) and imported.read_bytes()!=before
                     and '*' not in title(window),
                     'the edited vector width is saved in the native document')
                after=imported.read_bytes()
                after_state=native_state_bytes(after)
                after_layer=next(layer for layer in after_state['layers']
                                 if layer['id']==after_state['selected'])
                after_path=after_layer['vector_paths'][0]
                assert after_path['id']==before_path['id']
                assert after_path['start']==before_path['start']
                assert after_path['segments']==before_path['segments']
                assert native_document_tables(after)[1]==native_document_tables(before)[1], \
                    'vector style editing must not write derived pixels into raster tiles'
                assert native_document_tables(after)[2]==native_document_tables(before)[2], \
                    'vector style editing must preserve independent masks'
                assert after_path['width']!=before_path['width']
                assert abs(after_path['width']-float(applied[1]))<0.02
                wait(lambda:(lambda update:update and before_preview
                             and update[0]!=before_preview[0])(
                                 layer_thumbnail_update(before_layer['id'])),
                     'editing vector width invalidates its cached layer thumbnail')
                ctrl('z');ctrl('s')
                wait(lambda:imported.read_bytes()==before,
                     'Undo restores the exact pre-width vector project')
                ctrl('y');ctrl('s')
                wait(lambda:imported.read_bytes()==after,
                     'Redo restores the saved vector width exactly')
                return after

            # Move within the visible track instead of starting at its edge,
            # where the thumb/label hit area is driver-dependent at high DPI.
            vector_styled=apply_vector_width(0.45,0.65)
            # Confirm the saved editable style is restored by the application's
            # ordinary Open workflow, not merely by its serializer.
            styled_open_count=event_marker_count('UI_TEST_PROJECT_OPENED')
            ctrl('o');path_prompt();ctrl('Return')
            wait(lambda:event_marker_count('UI_TEST_PROJECT_OPENED')>styled_open_count,
                 "the styled vector project reopens through the application's Open workflow")
            reopened_style_state=native_state_bytes(imported.read_bytes())
            reopened_style_layer=next(layer for layer in reopened_style_state['layers']
                                      if layer['id']==reopened_style_state['selected'])
            assert reopened_style_layer['vector_paths'][0]['width'] \
                ==native_state_bytes(vector_styled)['layers'][
                    next(index for index,layer in enumerate(native_state_bytes(vector_styled)['layers'])
                         if layer['id']==reopened_style_state['selected'])]['vector_paths'][0]['width']

            vector_move_before=imported.read_bytes()
            prior_move_count=len(vector_move_markers())
            drag_at(560,570,590,605)
            wait(lambda: len(vector_move_markers())>prior_move_count,
                 'a reopened vector path can be selected and moved again through the canvas')
            ctrl('s')
            wait(lambda: imported.read_bytes()!=vector_move_before and '*' not in title(window),
                 'moving a vector object saves changed editable geometry')
            vector_moved=imported.read_bytes()
            moved_state=native_state_bytes(vector_moved)
            moved_layer=next(layer for layer in moved_state['layers']
                             if layer['id']==moved_state['selected'])
            assert moved_layer['vector_paths'][0]['start']!=vector_layer['vector_paths'][0]['start'], \
                'path movement must change editable coordinates rather than only a preview'
            assert native_document_tables(vector_moved)[1]==vector_raster_before
            assert native_document_tables(vector_moved)[2]==vector_mask_before
            delete_rect=wait(vector_delete_rect,
                             'selected vector path exposes an accessible contextual delete action')
            prior_vector_delete_count=sum(line.startswith('UI_TEST_VECTOR_PATH_DELETED: ')
                                          for line in (directory/'application.log').read_text().splitlines())
            click_at((delete_rect[0]+delete_rect[2]/2)*delete_rect[4],
                     (delete_rect[1]+delete_rect[3]/2)*delete_rect[4])
            wait(lambda: sum(line.startswith('UI_TEST_VECTOR_PATH_DELETED: ')
                             for line in (directory/'application.log').read_text().splitlines())>
                 prior_vector_delete_count,
                 'the contextual delete action invokes the document transaction')
            ctrl('s')
            wait(lambda: not any(layer.get('vector_paths') for layer in native_state(imported)['layers'])
                 and imported.read_bytes()[:8]==b'RPLALP03'
                 and '*' not in title(window),
                 'deleting the final path keeps the v3 allocator record and removes the object')
            deleted_vector=imported.read_bytes()
            ctrl('z');ctrl('s')
            wait(lambda: imported.read_bytes()==vector_moved,
                 'Delete path participates in exact application undo')
            ctrl('y');ctrl('s')
            wait(lambda: imported.read_bytes()==deleted_vector,
                 'Delete path participates in exact application redo')
            ctrl('z');ctrl('s')
            wait(lambda: imported.read_bytes()==vector_moved,
                 'undo restores the editable path before interoperability export')
            ora=directory/'imported-artwork.ora'
            ora.unlink(missing_ok=True)
            ctrl_shift('e')
            wait(lambda: event_marker('UI_TEST_OPENRASTER_VECTORS_FLATTENED')=='true'
                 and ora.exists(),
                 'OpenRaster export discloses that editable vector objects become pixels',
                 seconds=120)
            with zipfile.ZipFile(ora) as archive:
                assert archive.testzip() is None
                layer_pngs=[name for name in archive.namelist()
                            if name.startswith('data/layer') and name.endswith('.png')]
                assert layer_pngs, 'OpenRaster export must include flattened layer pixel images'
                assert archive.read('mergedimage.png').startswith(b'\x89PNG\r\n\x1a\n')
            # Linear and radial gradients use the actual pointer lifecycle,
            # transient GPU preview, document transaction, native save and
            # undo/redo path. Escape must discard only the preview.
            key('g')
            wait(lambda: event_marker('UI_TEST_ACTIVE_TOOL')=='Gradient',
                 'G selects the real document-backed gradient tool')
            wait(lambda: gradient_control_rect('linear') is not None,
                 'gradient controls expose accessible Linear and Radial modes')
            gradient_base=imported.read_bytes()
            cancellation_count=sum(line.startswith('UI_TEST_GRADIENT_CANCELLED: ')
                                   for line in (directory/'application.log').read_text().splitlines())
            move(620,610)
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                capture_window(window,'gradient-preview-before')
                preview_before=(Path(os.environ['RPL_UI_SCREENSHOT_DIR'])/'gradient-preview-before.png').read_bytes()
            move(480,180);click_button(1,True);move(540,300);move(620,610)
            wait(lambda: event_marker('UI_TEST_GRADIENT_PREVIEW') is not None,
                 'active gradient drag creates a transient preview texture')
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                capture_window(window,'gradient-preview-linear')
                preview_after=(Path(os.environ['RPL_UI_SCREENSHOT_DIR'])/'gradient-preview-linear.png').read_bytes()
                visible_gradient_pixels=screenshot_pixel_difference(preview_before,preview_after)
                assert visible_gradient_pixels>=2000, \
                    f'live gradient preview is not visible across the canvas ({visible_gradient_pixels} changed pixels)'
                print(f'UI_TEST_GRADIENT_PREVIEW_PIXELS: {visible_gradient_pixels}', flush=True)
            key('Escape');click_button(1,False)
            wait(lambda: sum(line.startswith('UI_TEST_GRADIENT_CANCELLED: escape')
                             for line in (directory/'application.log').read_text().splitlines())>cancellation_count,
                 'Escape cancels a gradient preview without committing artwork')
            assert imported.read_bytes()==gradient_base, \
                'cancelled gradient preview must leave existing native artwork byte-for-byte unchanged'
            linear_before=imported.read_bytes()
            linear_count=sum(line.startswith('UI_TEST_GRADIENT_COMMITTED: ')
                             for line in (directory/'application.log').read_text().splitlines())
            drag_at(480,180,620,610)
            wait(lambda: sum(line.startswith('UI_TEST_GRADIENT_COMMITTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>linear_count,
                 'canvas drag commits a linear gradient through the document engine')
            linear_marker=(directory/'application.log').read_text().split('UI_TEST_GRADIENT_COMMITTED: ')[-1].splitlines()[0].split()
            assert linear_marker[0]=='Linear', f'linear mode committed a different gradient: {linear_marker}'
            ctrl('s')
            wait(lambda: valid_project(imported) and imported.read_bytes()!=linear_before and '*' not in title(window),
                 'linear gradient pixels are saved to the existing native project')
            sx,sy,ex,ey=map(float,linear_marker[1:])
            sample_x=max(0,min(1799,round((sx+ex)/2)))
            sample_y=max(0,min(2399,round((sy+ey)/2)))
            linear_sample=native_layer_pixel(imported,sample_x,sample_y)
            assert linear_sample[3]>0 and linear_sample!=native_layer_pixel_bytes(linear_before,sample_x,sample_y), \
                'linear gradient changed no visible selected-layer sample'
            linear_bytes=imported.read_bytes()
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                capture_window(window,'gradient-committed-linear')
            ctrl('z');ctrl('s')
            wait(lambda: imported.read_bytes()==linear_before,
                 'linear gradient is one exact undoable transaction')
            radial_rect=wait(lambda: gradient_control_rect('radial'),
                             'Radial selector remains keyboard/touch accessible')
            click_at((radial_rect[0]+radial_rect[2]/2)*radial_rect[4],
                     (radial_rect[1]+radial_rect[3]/2)*radial_rect[4])
            wait(lambda: event_marker('UI_TEST_GRADIENT_MODE')=='Radial',
                 'Radial selector changes the active gradient mode')
            radial_count=sum(line.startswith('UI_TEST_GRADIENT_COMMITTED: ')
                             for line in (directory/'application.log').read_text().splitlines())
            drag_at(480,180,620,610)
            wait(lambda: sum(line.startswith('UI_TEST_GRADIENT_COMMITTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>radial_count,
                 'canvas drag commits a radial gradient through the document engine')
            radial_marker=(directory/'application.log').read_text().split('UI_TEST_GRADIENT_COMMITTED: ')[-1].splitlines()[0].split()
            assert radial_marker[0]=='Radial', f'radial mode committed a different gradient: {radial_marker}'
            ctrl('s')
            wait(lambda: valid_project(imported) and imported.read_bytes()!=linear_before and '*' not in title(window),
                 'radial gradient pixels are saved to the existing native project')
            sx,sy,ex,ey=map(float,radial_marker[1:])
            dx,dy=ex-sx,ey-sy
            length=math.hypot(dx,dy)
            sample_x=max(0,min(1799,round((sx+ex)/2-dy/length*length*0.22)))
            sample_y=max(0,min(2399,round((sy+ey)/2+dx/length*length*0.22)))
            radial_sample=native_layer_pixel(imported,sample_x,sample_y)
            assert radial_sample[3]>0 and radial_sample!=linear_sample, \
                'radial gradient did not produce distinct off-axis raster output'
            radial_bytes=imported.read_bytes()
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                capture_window(window,'gradient-committed-radial')
            ctrl('z');ctrl('s')
            wait(lambda: imported.read_bytes()==linear_before,
                 'radial gradient is one exact undoable transaction')
            ctrl('y');ctrl('s')
            wait(lambda: imported.read_bytes()==radial_bytes and '*' not in title(window),
                 'radial gradient redo restores the exact saved pixels')
            # Exercise Smudge through its real toolbar shortcut, winit input,
            # lifecycle processor, router, document transaction and native
            # persistence. Compare raster tile bytes, not just the window title.
            smudge_base=imported.read_bytes()
            key('s')
            wait(lambda: event_marker('UI_TEST_ACTIVE_TOOL')=='Smudge',
                 'S selects the document-backed Smudge tool')
            smudge_starts=sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                              for line in (directory/'application.log').read_text().splitlines())
            smudge_commits=event_marker_count('UI_TEST_MOUSE_STROKE_COMMITTED')
            drag_at(520,350,600,390)
            wait(lambda: sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>smudge_starts,
                 'Smudge uses the ordinary routed canvas pointer lifecycle')
            wait(lambda:event_marker_count('UI_TEST_MOUSE_STROKE_COMMITTED')>smudge_commits,
                 'the primary-button release commits the Smudge transaction before saving')
            ctrl('s')
            wait(lambda: valid_project(imported) and imported.read_bytes()!=smudge_base
                 and '*' not in title(window),
                 'committed Smudge pixels are saved in the editable native document')
            smudge_bytes=imported.read_bytes()
            base_tiles=native_layer_tiles(smudge_base)
            smudged_tiles=native_layer_tiles(smudge_bytes)
            changed_smudge_pixels=sum(
                old_pixel!=new_pixel
                for tile_key in set(base_tiles)|set(smudged_tiles)
                for old_tile,new_tile in [(base_tiles.get(tile_key,b'\0'*16384),
                                           smudged_tiles.get(tile_key,b'\0'*16384))]
                for old_pixel,new_pixel in zip(
                    (old_tile[i:i+4] for i in range(0,16384,4)),
                    (new_tile[i:i+4] for i in range(0,16384,4)))
            )
            assert changed_smudge_pixels>0, 'Smudge must modify authoritative selected-layer pixels'
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                capture_window(window,'smudge-workspace')
            ctrl('z');ctrl('s')
            wait(lambda: imported.read_bytes()==smudge_base,
                 'Smudge is one exact undoable raster transaction')
            ctrl('y');ctrl('s')
            try:
                wait(lambda: imported.read_bytes()==smudge_bytes and '*' not in title(window),
                     'redo restores the exact committed Smudge result')
            except AssertionError:
                print('UI_TEST_SMUDGE_REDO_MISMATCH: '
                      +native_project_difference_summary(smudge_bytes,imported.read_bytes()),
                      flush=True)
                raise
            # Color Mixer is a separate raster operation: it deposits the
            # selected brush color into existing pixels and uses the ordinary
            # routed input path, but replaces touched layer pixels rather than
            # painting a separate opaque preview on top of the document.
            color_mix_base=imported.read_bytes()
            key('x')
            wait(lambda: event_marker('UI_TEST_ACTIVE_TOOL')=='ColorMix',
                 'X selects the document-backed Color Mixer tool')
            color_mix_starts=sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                                 for line in (directory/'application.log').read_text().splitlines())
            color_mix_commits=event_marker_count('UI_TEST_MOUSE_STROKE_COMMITTED')
            drag_at(520,350,600,390)
            wait(lambda: sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>color_mix_starts,
                 'Color Mixer uses the ordinary routed canvas pointer lifecycle')
            wait(lambda:event_marker_count('UI_TEST_MOUSE_STROKE_COMMITTED')>color_mix_commits,
                 'the primary-button release commits the Color Mixer transaction before saving')
            ctrl('s')
            wait(lambda: valid_project(imported) and imported.read_bytes()!=color_mix_base
                 and '*' not in title(window),
                 'committed color-mixing pixels are saved in the editable native document')
            color_mix_bytes=imported.read_bytes()
            color_base_tiles=native_layer_tiles(color_mix_base)
            color_mixed_tiles=native_layer_tiles(color_mix_bytes)
            changed_mix_pixels=sum(
                old_pixel!=new_pixel
                for tile_key in set(color_base_tiles)|set(color_mixed_tiles)
                for old_tile,new_tile in [(color_base_tiles.get(tile_key,b'\0'*16384),
                                           color_mixed_tiles.get(tile_key,b'\0'*16384))]
                for old_pixel,new_pixel in zip(
                    (old_tile[i:i+4] for i in range(0,16384,4)),
                    (new_tile[i:i+4] for i in range(0,16384,4)))
            )
            assert changed_mix_pixels>0, \
                'Color Mixer must change authoritative selected-layer pixels'
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                capture_window(window,'color-mixer-workspace')
            ctrl('z');ctrl('s')
            wait(lambda: imported.read_bytes()==color_mix_base,
                 'Color Mixer is one exact undoable raster transaction')
            ctrl('y');ctrl('s')
            try:
                wait(lambda: imported.read_bytes()==color_mix_bytes and '*' not in title(window),
                     'redo restores the exact committed Color Mixer result')
            except AssertionError:
                print('UI_TEST_COLOR_MIX_REDO_MISMATCH: '
                      +native_project_difference_summary(color_mix_bytes,imported.read_bytes()),
                      flush=True)
                raise
            # The UI's symmetry panel must affect real routed mouse strokes,
            # save preference separately, and persist reflected pixels as one
            # ordinary editable native raster transaction.
            key('b')
            wait(lambda: event_marker('UI_TEST_ACTIVE_TOOL')=='Brush',
                 'B restores the normal raster painting tool')
            symmetry_base=imported.read_bytes()
            key('y')
            wait(lambda: event_marker('UI_TEST_SYMMETRY_PANEL_TOGGLED')=='true',
                 'Y opens the keyboard-accessible drawing symmetry panel')
            vertical=wait(lambda: symmetry_mode_rect('Vertical'),
                          'symmetry panel exposes a named Vertical mode control')
            click_at((vertical[0]+vertical[2]/2)*vertical[4],
                     (vertical[1]+vertical[3]/2)*vertical[4])
            wait(lambda: (event_marker('UI_TEST_SYMMETRY_CHANGED') or '').startswith('Vertical'),
                 'Vertical mode enables actual mirrored drawing')
            wait(lambda: json.loads(prefs.read_text()).get('symmetry',{}).get('mode')=='vertical',
                 'symmetry mode is persisted in local workspace preferences')
            origin_button=wait(symmetry_origin_button_rect,
                               'symmetry panel exposes an accessible direct-origin control')
            starts_before_origin=sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                                     for line in (directory/'application.log').read_text().splitlines())
            click_at((origin_button[0]+origin_button[2]/2)*origin_button[4],
                     (origin_button[1]+origin_button[3]/2)*origin_button[4])
            wait(lambda: event_marker('UI_TEST_SYMMETRY_ORIGIN_ARMED')=='true',
                 'direct origin placement arms from the visible panel control')
            canvas=wait(symmetry_canvas_bounds,
                        'document mapping exposes physical bounds for pixel-level verification')
            left,top,right,bottom,doc_width,doc_height=canvas
            expected_center=[0.50,0.68]
            origin_client=(left+expected_center[0]*(right-left),
                           top+expected_center[1]*(bottom-top))
            click_at(*origin_client)
            wait(lambda: event_marker('UI_TEST_SYMMETRY_ORIGIN_PLACED') is not None,
                 'mouse placement maps through the current canvas zoom to document origin')
            center=prefs.read_text()
            persisted_center=json.loads(center).get('symmetry',{}).get('center')
            assert persisted_center is not None and all(
                abs(value-target)<0.01 for value,target in zip(persisted_center,expected_center)
            ), f'normalized canvas origin was not persisted: {persisted_center}'
            assert imported.read_bytes()==symmetry_base, \
                'placing the symmetry origin must not change artwork or its saved project'
            assert sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                       for line in (directory/'application.log').read_text().splitlines()) == starts_before_origin, \
                'the click used to place the symmetry origin must never start a painting stroke'
            center_before_cancel=tuple(persisted_center)
            origin_button=wait(symmetry_origin_button_rect,
                               'origin control remains available after a successful placement')
            click_at((origin_button[0]+origin_button[2]/2)*origin_button[4],
                     (origin_button[1]+origin_button[3]/2)*origin_button[4])
            wait(lambda: event_marker('UI_TEST_SYMMETRY_ORIGIN_ARMED')=='true',
                 'origin placement can be armed again')
            key('Escape')
            wait(lambda: event_marker('UI_TEST_SYMMETRY_ORIGIN_CANCELLED')=='escape',
                 'Escape cancels origin placement before the app close action')
            assert json.loads(prefs.read_text()).get('symmetry',{}).get('center')==list(center_before_cancel), \
                'cancelling origin placement must preserve the prior workspace origin'
            assert imported.read_bytes()==symmetry_base, \
                'arming and cancelling origin placement must not mutate artwork'
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                capture_window(window,'symmetry-panel-custom-origin')
            start=(left+0.72*(right-left),top+0.43*(bottom-top))
            end=(left+0.79*(right-left),top+0.48*(bottom-top))
            stroke_starts=sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                              for line in (directory/'application.log').read_text().splitlines())
            drag_at(*start,*end)
            wait(lambda: sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>stroke_starts,
                 'symmetry-painted mouse input traverses Processor and Router')
            ctrl('s')
            wait(lambda: valid_project(imported) and imported.read_bytes()!=symmetry_base
                 and '*' not in title(window),
                 'mirrored stroke pixels are committed and saved in the native artwork')
            symmetry_bytes=imported.read_bytes()
            sample_x=round(0.755*doc_width)
            sample_y=round(0.455*doc_height)
            mirror_x=round(2*expected_center[0]*doc_width-sample_x)
            before_source=native_layer_pixel_bytes(symmetry_base,sample_x,sample_y)
            after_source=native_layer_pixel_bytes(symmetry_bytes,sample_x,sample_y)
            before_mirror=native_layer_pixel_bytes(symmetry_base,mirror_x,sample_y)
            after_mirror=native_layer_pixel_bytes(symmetry_bytes,mirror_x,sample_y)
            assert after_source!=before_source and after_source[3]>0, \
                f'original stroke location did not change its editable layer pixel: {after_source.hex()}'
            assert after_mirror!=before_mirror and after_mirror[3]>0, \
                f'vertical reflection did not change its mirrored layer pixel: {after_mirror.hex()}'
            ctrl('z');ctrl('s')
            wait(lambda: imported.read_bytes()==symmetry_base,
                 'one Undo removes both original and reflected artwork')
            ctrl('y');ctrl('s')
            wait(lambda: imported.read_bytes()==symmetry_bytes and '*' not in title(window),
                 'Redo restores both symmetry copies byte-for-byte')
            key('y')
            wait(lambda: event_marker('UI_TEST_SYMMETRY_PANEL_TOGGLED')=='false',
                 'Y closes the symmetry panel without changing the selected tool')
            # The active process owns a fresh recovery lineage whose .rpl
            # snapshot may not exist until its first completed checkpoint.
            # Do not accidentally test an older process's last snapshot.
            owner=wait(lambda: latest_recovery_owner(recovery_dir),
                       'active document recovery owner is available')
            snapshot=owner.with_suffix('.rpl')
            journal_path=Path(str(snapshot)+'.journal')
            before=snapshot.read_bytes() if snapshot.exists() else None
            journal_before=journal_path.read_bytes() if journal_path.exists() else b''
            journal_checkpoint_before=latest_journal_checkpoint(journal_path)
            completed_before=recovery_complete_count()
            stroke_starts=sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                              for line in (directory/'application.log').read_text().splitlines())
            # Derive the recovery stroke from the live document viewport. The
            # previous fixed endpoint could land over Layers after the workspace
            # reflowed, beginning a stroke without committing any canvas pixels.
            left,top,right,bottom,_,_=wait(symmetry_canvas_bounds,
                                           'recovery test has current canvas bounds')
            start=(left+0.58*(right-left),top+0.45*(bottom-top))
            end=(left+0.70*(right-left),top+0.62*(bottom-top))
            print(f'UI_TEST_RECOVERY_STROKE_TARGET: {start} -> {end}',flush=True)
            key('b')
            wait(lambda: event_marker('UI_TEST_ACTIVE_TOOL')=='Brush',
                 'recovery stroke explicitly selects the paint brush tool')
            drag_at(*start,*end)
            wait(lambda: sum(line.startswith('UI_TEST_MOUSE_STROKE_STARTED: ')
                             for line in (directory/'application.log').read_text().splitlines())>stroke_starts,
                 'real canvas mouse input begins a painting transaction')
            status('*')
            wait(lambda: recovery_complete_count()>completed_before,
                 'active recovery writer completes the committed artwork checkpoint')
            recovered_bytes=wait(
                lambda: (payload if (payload:=latest_journal_checkpoint(journal_path))
                         is not None and payload!=journal_checkpoint_before else None),
                'latest checkpoint is durably journaled before process termination')
            assert recovered_bytes!=before, 'the live recovery lineage must advance after the real stroke'
            wait(lambda: valid_project(snapshot) and snapshot.read_bytes()==recovered_bytes,
                 'active snapshot matches its durable journal checkpoint')
            assert journal_path.read_bytes()!=journal_before, \
                'the recovery journal must append a new record for the stroke'
            preserved_artwork=imported.read_bytes()
            # Reopen the original workspace reference layout in the next
            # process after the canvas-only gestures exercised clear hit testing.
            workspace_preferences=json.loads(prefs.read_text())
            workspace_preferences['references']=reference_layouts_before_canvas
            prefs.write_text(json.dumps(workspace_preferences))
            process.kill();assert process.wait(timeout=10)!=0
            # Exercise recovery from the durable journal without its paired snapshot.
            snapshot.unlink()
            time.sleep(0.2)
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'workspace after forced crash');focus(display,window,2,0);send();time.sleep(0.5)
            ctrl_shift('r');status('*');ctrl('s')
            recovered=directory/'recovered-artwork.rpl'
            wait(lambda: recovered.exists(),
                 'forced-crash journal recovery writes a restorable native project')
            restored_bytes=recovered.read_bytes()
            assert native_projects_equivalent(recovered_bytes,restored_bytes), \
                'recovery preserves native metadata and all raster/mask pixels (f64 tolerance 1e-12)'
            assert imported.read_bytes()==preserved_artwork,'journal recovery leaves existing native artwork unchanged'


            close_application(process)
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
            open_card_x,open_card_y,open_card_width,open_card_height,open_card_scale=wait(
                lambda: window_rect_marker('UI_TEST_HOME_OPEN_CARD_RECT'),
                'the Home open-project tile reports its real bounds',
            )
            click_at((open_card_x+open_card_width/2)*open_card_scale,
                     (open_card_y+96.0)*open_card_scale)
            path_prompt()
            cancel_x,cancel_y,cancel_width,cancel_height,cancel_scale=wait(
                lambda: settled_window_rect_marker('UI_TEST_FILE_PICKER_CANCEL_RECT'),
                'the opened file picker exposes stable cancel-button bounds',
            )
            cancel_marker='UI_TEST_FILE_PICKER_CANCELLED'
            cancel_count=sum(line.startswith(cancel_marker)
                             for line in (directory/'application.log').read_text().splitlines())
            click_at((cancel_x+cancel_width/2)*cancel_scale,
                     (cancel_y+cancel_height/2)*cancel_scale)
            wait(lambda: sum(line.startswith(cancel_marker)
                             for line in (directory/'application.log').read_text().splitlines())>cancel_count,
                 'Cancel click reaches the file-picker action')
            wait(lambda: 'TEST FILE PICKER' not in title(window),
                 'Cancel returns to Home from the open-project tile')
            close_application(process)
            home_preferences=json.loads(prefs.read_text())
            assert any(item['path']==str(reference) for item in home_preferences.get('references',[])), 'reference board path survives an app restart through Home'
            # Exercise the same application windows under dark mode and high
            # contrast. The dark workspace uses the persisted top dock to cover
            # the horizontal toolbar layout. Seed Recovery with a valid .rpl.
            preferences=json.loads(prefs.read_text())
            preferences['appearance_mode']='dark';preferences['dark_theme']=True;preferences['high_contrast']=False
            preferences['palette_dock']='Top'
            for index,reference_layout in enumerate(preferences.get('references',[])):
                reference_layout['position']=[40.0+28.0*index,190.0+24.0*index]
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
            assert_strong_text_contrast('dark')
            resize_window(window,1280,800);capture_window(window,'home-dark-normal')
            resize_window(window,1920,1080);capture_window(window,'home-dark-wide')
            home_dark_bounds=wait(home_content_bounds,'wide dark Home content is measured inside its centered layout')
            assert abs(home_dark_bounds[0]+home_dark_bounds[2]/2-960.0)<=4.0 and home_dark_bounds[2]<=1600.0, \
                f'wide dark Home should stay centered with the light layout: {home_dark_bounds}'
            resize_window(window,640,480);capture_window(window,'home-dark-narrow')
            resize_window(window,1280,800)
            ctrl('n');new_document_prompt()
            wait(lambda: (directory / 'application.log').read_text().count('UI_TEST_NEW_ARTWORK_VISIBLE') >= 2,
                 'dark New Artwork interface has rendered before capture')
            assert_new_document_name_and_create_visible(1280,800)
            time.sleep(0.25);capture_window(window,'new-artwork-dark-normal')
            resize_window(window,640,480)
            assert_new_document_name_and_create_visible(640,480)
            capture_window(window,'new-artwork-dark-narrow')
            hidden_wheel_count=sum(line.startswith('UI_TEST_REFERENCE_WHEEL_IGNORED_HIDDEN')
                                   for line in (directory/'application.log').read_text().splitlines())
            wheel_at(320,430,notches=8);time.sleep(0.25)
            wait(lambda: sum(line.startswith('UI_TEST_REFERENCE_WHEEL_IGNORED_HIDDEN')
                             for line in (directory/'application.log').read_text().splitlines())
                 >hidden_wheel_count,
                 'wheel input at the hidden saved Reference position safely reaches the visible New Artwork dialog')
            assert process.poll() is None, 'scrolling the dark New Artwork dialog must not route through a hidden reference viewport'
            dark_create=wait(lambda: window_rect_marker('UI_TEST_NEW_DOCUMENT_CREATE_RECT'),
                             'dark narrow New Artwork exposes the Create action by scrolling')
            assert dark_create[1]+dark_create[3]<=480/dark_create[4]+1, \
                f'dark narrow Create action is clipped after scrolling: {dark_create}'
            capture_window(window,'new-artwork-dark-narrow-scrolled')
            click_at((dark_create[0]+dark_create[2]/2)*dark_create[4],
                     (dark_create[1]+dark_create[3]/2)*dark_create[4])
            wait(lambda: 'UI_TEST_DOCUMENT_CREATED: 1800 2400 artwork 1' in
                 (directory/'application.log').read_text(),
                 'dark narrow New Artwork Create action creates its document')
            resize_window(window,1280,800);time.sleep(0.4)
            capture_window(window,'canvas-workspace-dark-toolbar-top')
            clearance=wait(top_palette_clearance,'top-docked palette reports its measured app-toolbar clearance')
            assert clearance[0]>=clearance[1]-0.5, f'top palette overlaps contextual options: {clearance}'
            wait(lambda: (directory / 'application.log').read_text().count('UI_TEST_REFERENCE_IMAGE_VISIBLE:') > reference_markers_before_dark,
                 'dark reference screenshot renders its imported image')
            time.sleep(0.35)  # wait until the marked reference frame is presented
            capture_window(window,'reference-board-dark')
            resize_window(window,640,480);capture_window(window,'reference-board-dark-narrow')
            resize_window(window,1280,800)
            ctrl('b');time.sleep(0.35);capture_window(window,'brush-library-dark')
            key('Escape');key('c');time.sleep(0.35);capture_window(window,'color-controls-dark')
            key('Escape');ctrl_shift('n');ctrl_shift('n');time.sleep(0.3)
            wait(lambda: layer_row_rect(2),'dark Layers screenshot includes three actual editable layer entries')
            capture_window(window,'layers-panel-populated-dark')
            saved_panel_geometry=json.loads(prefs.read_text())
            resize_window(window,640,480)
            narrow_layers=wait(layers_window_rect,'compact Layers window reports stable bounds')
            narrow_reference=wait(compact_reference_window_rect,'the active compact reference reports stable bounds')
            narrow_palette=wait(palette_bounds,'compact toolbar reports its bounds beside the panels')
            assert narrow_layers[0]>=0 and narrow_layers[1]>=0 \
                and narrow_layers[0]+narrow_layers[2]<=640/narrow_layers[4]+1 \
                and narrow_layers[1]+narrow_layers[3]<=480/narrow_layers[4]+1, \
                f'compact Layers panel must remain inside 640x480: {narrow_layers}'
            assert narrow_layers[1]+narrow_layers[3]<=408/narrow_layers[4]+1, \
                f'compact Layers panel must not cover the application status bar: {narrow_layers}'
            assert not rects_overlap(narrow_layers[:4],narrow_palette[:4]), \
                f'compact Layers panel overlaps the tool palette: layers={narrow_layers}, palette={narrow_palette}'
            compact_rects=[narrow_layers[:4],narrow_reference[:4]]
            for left_index,left in enumerate(compact_rects):
                assert left[0]>=0 and left[1]>=0 \
                    and left[0]+left[2]<=640/narrow_layers[4]+1 \
                    and left[1]+left[3]<=480/narrow_layers[4]+1, \
                    f'compact workspace panel is clipped at 640x480: {left}'
                for right in compact_rects[left_index+1:]:
                    assert not rects_overlap(left,right), \
                        f'compact Layers/Reference windows overlap: {left} vs {right}'
            wait(lambda: layer_row_rect(2),'compact Layers keeps its populated entries reachable')
            narrow_panel_geometry=json.loads(prefs.read_text())
            assert narrow_panel_geometry['layers_flyout_size']==saved_panel_geometry['layers_flyout_size'], \
                'temporary narrow layout must not replace saved Layers size'
            assert [entry['size'] for entry in narrow_panel_geometry.get('references',[])] == \
                [entry['size'] for entry in saved_panel_geometry.get('references',[])], \
                'temporary narrow layout must not replace saved Reference sizes'
            capture_window(window,'layers-reference-populated-dark-narrow')
            capture_window(window,'layers-panel-populated-dark-narrow')
            resize_window(window,1280,800)
            ctrl('l');time.sleep(0.2)
            ctrl_shift('m');time.sleep(0.35)
            assert any(p.name.startswith('recovery-') and p.suffix=='.rpl' and valid_project(p) for p in recovery_dir.iterdir()), 'dark recovery manager has a valid checkpoint entry'
            wait(lambda: 'UI_TEST_RECOVERY_LABEL_CONTRAST: dark ' in (directory/'application.log').read_text(),
                 'dark recovery entry uses readable text contrast')
            dark_recovery_contrast = next(line for line in reversed((directory/'application.log').read_text().splitlines())
                if line.startswith('UI_TEST_RECOVERY_LABEL_CONTRAST: dark '))
            assert float(dark_recovery_contrast.rsplit(' ',1)[1]) >= 4.5, 'dark recovery label contrast is below 4.5:1'
            dark_hover_count=sum(line.startswith('UI_TEST_RECOVERY_PREVIEW_HOVERED')
                                 for line in (directory/'application.log').read_text().splitlines())
            target=wait(recovery_preview_target,'dark recovery preview target geometry')
            click_x=round((target[0]+target[2]/2)*target[4])
            click_y=None
            for offset in range(-80,81,4):
                candidate=round((target[1]+target[3]/2)*target[4])+offset
                move(click_x,candidate)
                hovered=sum(line.startswith('UI_TEST_RECOVERY_PREVIEW_HOVERED')
                            for line in (directory/'application.log').read_text().splitlines())
                if hovered>dark_hover_count:
                    click_y=candidate
                    break
            assert click_y is not None,'dark recovery preview button did not receive pointer hover'
            visible_count=sum(line.startswith('UI_TEST_RECOVERY_PREVIEW_VISIBLE')
                              for line in (directory/'application.log').read_text().splitlines())
            click_button(1,True);click_button(1,False)
            wait(lambda: sum(line.startswith('UI_TEST_RECOVERY_PREVIEW_VISIBLE')
                             for line in (directory/'application.log').read_text().splitlines())>visible_count,
                 'dark recovery manager renders the requested artwork preview')
            capture_window(window,'recovery-manager-populated-dark')
            resize_window(window,640,480);capture_window(window,'recovery-manager-populated-dark-narrow')
            resize_window(window,1280,800)
            ctrl_shift('m')
            # The preceding New Artwork capture creates a fresh unsaved
            # document. Escape correctly enters the close-save path, so end
            # this screenshot-only process explicitly before reopening Home.
            process.terminate();assert process.wait(timeout=10) is not None
            preferences=json.loads(prefs.read_text());preferences['high_contrast']=True;prefs.write_text(json.dumps(preferences))
            process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=subprocess.STDOUT)
            window=wait(find_window,'high-contrast Home');focus(display,window,2,0);send();time.sleep(0.5)
            capture_window(window,'home-dark-high-contrast')
            close_application(process)
            if os.environ.get('RPL_UI_SCREENSHOT_DIR'):
                required={'home-light-normal','home-light-wide','home-light-narrow','home-preset-created','home-recent-artwork-light','new-artwork-light-normal','new-artwork-light-narrow','new-artwork-light-narrow-scrolled','reference-board-light','reference-board-light-narrow','reference-board-multi-light','compact-reference-first','compact-reference-second','brush-library-light-normal','brush-library-light-narrow','color-controls-light','layers-panel-populated-light','layers-panel-populated-light-narrow','layers-panel-white-mask-settings','layers-panel-mask-editing','layers-panel-painted-mask','recovery-manager-populated-light','recovery-manager-populated-light-narrow','workspace-controls-floating-light-normal','home-dark-normal','home-dark-wide','home-dark-narrow','new-artwork-dark-normal','new-artwork-dark-narrow','new-artwork-dark-narrow-scrolled','canvas-workspace-dark-toolbar-top','reference-board-dark','reference-board-dark-narrow','brush-library-dark','color-controls-dark','layers-panel-populated-dark','layers-panel-populated-dark-narrow','layers-reference-populated-dark-narrow','recovery-manager-populated-dark','recovery-manager-populated-dark-narrow','symmetry-panel-custom-origin','selection-move-canvas','selection-scale-rotate-workspace','quick-shape-workspace','vector-node-editor','vector-node-editor-with-inserted-midpoint','vector-cubic-handles','home-dark-high-contrast'}
                missing=required-set(captured_screenshots)
                assert not missing,f'screenshot suite missed required states: {sorted(missing)}'
                for name,(width,height) in captured_screenshots.items():
                    if name.endswith('narrow'):
                        assert width<=700 and height<=520,f'{name} was not captured at a narrow size: {width}x{height}'
                    if name.endswith('normal'):
                        assert width>=900 and height>=650,f'{name} was not captured at a normal size: {width}x{height}'
                    if name.endswith('wide'):
                        if width < 1800 or height < 1000:
                            print(f'UI_SCREENSHOT_WIDE_SIZE: SKIPPED {name} ({width}x{height}; need 1800x1000)', flush=True)
                        else:
                            assert width>=1800 and height>=1000,f'{name} was not captured at a wide size: {width}x{height}'
                    if name == 'home-recent-artwork-light':
                        if width >= 1800 and height >= 1000:
                            assert width>=1800 and height>=1000, f'{name} must show the featured real artwork at wide size: {width}x{height}'
            print('ALPHA_WORKFLOW_SMOKE: PASSED (real mouse raster paint; vertical symmetry panel through keyboard shortcut, real mirrored layer pixels and exact undo/redo; live linear/radial gradient preview, Escape cancellation and selection-clipped commits; QuickShape line/rectangle/ellipse recognition, incomplete-gesture rejection, cancellation and exact undo/redo; transactional Smudge and Color Mixer strokes with saved tile changes and exact undo/redo; native save/reopen pixels; Home thumbnail and Open-project card after restart; persistent theme/reference board; multi-reference placement; floating workspace panels; saved colors and hue wheel; layer duplicate/reorder/delete/clear; selected-pixel drag move with Escape cancellation, raster-byte mutation and exact undo/redo; canvas scale/rotation handles with cancelled previews, pixel changes, save and exact undo/redo; selected-pixel copy to a new native layer with source-tile integrity and exact undo/redo; real opacity and blend-mode controls with native-save and undo/redo checks; Add/Edit/Enable/Disable/Invert/Clear/Remove editable masks with v1/v2 reopen, compositing, raster-tile integrity and exact undo/redo; reference isolation from exports; save checksum and backup; PNG/OpenRaster export/import; brush preset save/load; rectangle selection/fill/undo; zoom/fit; journal-durable process-kill recovery without paired snapshot; preserved source artwork; clean close)')
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
            mask_trace=[line for line in relevant
                        if line.startswith(('UI_TEST_LAYER_MASK_',
                                            'UI_TEST_LAYER_SETTINGS_HEADER_RECT:',
                                            'UI_TEST_LAYER_OPACITY_SLIDER_RECT:',
                                            'UI_TEST_LAYER_BLEND_COMBO_RECT:'))]
            print('ALPHA_WORKFLOW_MASK_TRACE_BEGIN')
            print('\n'.join(mask_trace[-80:]))
            print('ALPHA_WORKFLOW_MASK_TRACE_END')
            recovery=[line for line in relevant if line.startswith('UI_TEST_RECOVERY_')]
            print('\n'.join(recovery[-24:]))
