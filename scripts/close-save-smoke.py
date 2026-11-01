#!/usr/bin/env python3
"""Exercise close-save, retry/cancel, and recovery through the real Linux UI.

This is a focused XTest integration test for the normal winit/egui application.
It uses temporary local storage and never changes or reads the user's artwork.
"""
import ctypes as c
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time

X11 = c.CDLL("libX11.so.6")
XTEST = c.CDLL("libXtst.so.6")
D, W, I, U = c.c_void_p, c.c_ulong, c.c_int, c.c_uint


def api(lib, name, result, *args):
    function = getattr(lib, name)
    function.restype, function.argtypes = result, args
    return function


open_display = api(X11, "XOpenDisplay", D, c.c_char_p)
root_window = api(X11, "XDefaultRootWindow", W, D)
query_tree = api(X11, "XQueryTree", I, D, W, c.POINTER(W), c.POINTER(W),
                 c.POINTER(c.POINTER(W)), c.POINTER(U))
get_geometry = api(X11, "XGetGeometry", I, D, W, c.POINTER(W), c.POINTER(I),
                   c.POINTER(I), c.POINTER(U), c.POINTER(U), c.POINTER(U),
                   c.POINTER(U))
fetch_name = api(X11, "XFetchName", I, D, W, c.POINTER(c.c_void_p))
free = api(X11, "XFree", I, c.c_void_p)
translate = api(X11, "XTranslateCoordinates", I, D, W, W, I, I,
                c.POINTER(I), c.POINTER(I), c.POINTER(W))
set_focus = api(X11, "XSetInputFocus", I, D, W, I, W)
raise_window = api(X11, "XRaiseWindow", I, D, W)
resize = api(X11, "XResizeWindow", I, D, W, U, U)
move_resize = api(X11, "XMoveResizeWindow", I, D, W, I, I, U, U)
sync = api(X11, "XSync", I, D, I)
keysym = api(X11, "XStringToKeysym", W, c.c_char_p)
keycode = api(X11, "XKeysymToKeycode", U, D, W)
motion = api(XTEST, "XTestFakeMotionEvent", I, D, I, I, I, W)
button = api(XTEST, "XTestFakeButtonEvent", I, D, U, I, W)
key_event = api(XTEST, "XTestFakeKeyEvent", I, D, U, I, W)
display = open_display(None)
if not display:
    raise SystemExit("Xvfb/XTest display is required")
root = root_window(display)
window = 0
process = None
log_path = None
launch_number = 0


def lines():
    if not log_path or not log_path.exists():
        return []
    return log_path.read_text(errors="replace").splitlines()


def count(prefix):
    return sum(line.startswith(prefix) for line in lines())


def marker(prefix):
    return next((line[len(prefix):] for line in reversed(lines())
                 if line.startswith(prefix)), None)


def window_title(candidate):
    pointer = c.c_void_p()
    if not fetch_name(display, candidate, c.byref(pointer)) or not pointer:
        return ""
    try:
        return c.string_at(pointer).decode("utf-8", errors="replace")
    finally:
        free(pointer)


def find_window(token):
    parent, ancestor, children, child_count = W(), W(), c.POINTER(W)(), U()
    if not query_tree(display, root, c.byref(ancestor), c.byref(parent),
                      c.byref(children), c.byref(child_count)):
        return 0
    try:
        return next((children[index] for index in range(child_count.value)
                     if token in window_title(children[index])), 0)
    finally:
        if children:
            free(children)


def resize_window(width, height):
    """Wait for the visible X11 client to accept the requested test size."""
    def actual_size():
        root_return, x_return, y_return = W(), I(), I()
        actual_width, actual_height, border, depth = U(), U(), U(), U()
        if not get_geometry(display, window, c.byref(root_return),
                            c.byref(x_return), c.byref(y_return),
                            c.byref(actual_width), c.byref(actual_height),
                            c.byref(border), c.byref(depth)):
            return None
        return actual_width.value, actual_height.value

    for attempt in range(4):
        if attempt == 0:
            resize(display, window, width, height)
        else:
            # A resize sent before winit's first visible frame can be replaced
            # by its initial size. Reissue after visibility and verify X11's
            # actual geometry instead of assuming the request took effect.
            move_resize(display, window, 24, 24, width, height)
        sync(display, 0)
        time.sleep(0.12)
        if actual_size() == (width, height):
            return
    raise AssertionError(
        f"X11 refused requested window size {width}x{height}; actual size is {actual_size()}"
    )


def wait(predicate, description, timeout=15):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        if process and process.poll() is not None:
            raise AssertionError(
                f"application exited {process.returncode} while waiting for {description}:\n"
                + "\n".join(lines()[-40:])
            )
        last = predicate()
        if last:
            return last
        time.sleep(0.04)
    diagnostic_prefixes = (
        "CLOSE_SAVE_", "RECOVERY_CHECKPOINT_", "TEST_PATH_DIALOG:",
        "UI_TEST_DOCUMENT_", "UI_TEST_HOME_OPEN_PROJECT_CLICKED",
        "UI_TEST_MOUSE_STROKE_", "UI_TEST_OPEN_PROJECT_FAILED:",
        "UI_TEST_CLOSE_SAVE_", "UI_TEST_RECOVERY_",
        "UI_TEST_SAVED_DRAFTS_PAGE:", "UI_TEST_SAVED_DRAFTS_NEXT_RECT:",
        "UI_TEST_SAVED_DRAFTS_NEXT_HOVERED:", "UI_TEST_SAVED_DRAFTS_NEXT_CLICKED:",
    )
    recent_log = "\n".join(line for line in lines()
                            if line.startswith(diagnostic_prefixes))
    raise AssertionError(
        f"timeout waiting for {description}; last value={last!r}\n"
        f"recent application log:\n{recent_log}"
    )


def send():
    sync(display, 0)
    time.sleep(0.06)


def move(x, y):
    dx, dy, child = I(), I(), W()
    assert translate(display, window, root, 0, 0, c.byref(dx), c.byref(dy), c.byref(child))
    assert motion(display, -1, dx.value + round(x), dy.value + round(y), 0)
    send()


def click(x, y):
    move(x, y)
    assert button(display, 1, 1, 0) and button(display, 1, 0, 0)
    send()


def scroll_wheel(down, steps=1):
    code = 5 if down else 4
    for _ in range(steps):
        assert button(display, code, 1, 0) and button(display, code, 0, 0)
    send()


def press(name):
    code = keycode(display, keysym(name.encode("ascii")))
    assert code and key_event(display, code, 1, 0) and key_event(display, code, 0, 0)
    send()


def chord(modifier, name):
    modifier_code = keycode(display, keysym(modifier.encode("ascii")))
    assert modifier_code and key_event(display, modifier_code, 1, 0)
    press(name)
    assert key_event(display, modifier_code, 0, 0)
    send()


def type_text(value):
    for character in value:
        shifted = character in "_+:<>?"
        key_name = {
            " ": "space", "/": "slash", "-": "minus", ".": "period", "_": "minus",
            ":": "semicolon", "+": "equal", "<": "comma", ">": "period", "?": "slash",
        }.get(character, character)
        code = keycode(display, keysym(key_name.encode("ascii")))
        shift_code = keycode(display, keysym(b"Shift_L")) if shifted else 0
        if shift_code:
            assert key_event(display, shift_code, 1, 0)
        assert code and key_event(display, code, 1, 0) and key_event(display, code, 0, 0)
        if shift_code:
            assert key_event(display, shift_code, 0, 0)
        send()


def stable_rect(name):
    prefix = name + ": "
    entries = [line[len(prefix):] for line in lines() if line.startswith(prefix)]
    if len(entries) < 2 or entries[-1] != entries[-2]:
        return None
    return tuple(map(float, entries[-1].split()))


def scroll_rect_into_view(name, width=1280, height=800):
    for _ in range(40):
        rect = wait(lambda: stable_rect(name), f"stable {name} geometry")
        x, y, rect_width, rect_height, scale = rect
        viewport_height = height / scale
        viewport_width = width / scale
        if x < 0 or x + rect_width > viewport_width - 8:
            raise AssertionError(
                f"{name} is horizontally clipped: rect={rect}, "
                f"window={width}x{height} at scale={scale}"
            )
        if y >= 0 and y + rect_height <= viewport_height - 8:
            return rect
        scroll_wheel(down=y + rect_height > viewport_height, steps=1)
    raise AssertionError(f"could not scroll {name} into view; last rect={rect}")


def canvas_bounds():
    prefix = "UI_TEST_SYMMETRY_CANVAS: "
    entries = [line.partition(": ")[2] for line in lines()
               if line.startswith(prefix)]
    if len(entries) < 2 or entries[-1] != entries[-2]:
        return None
    return tuple(map(float, entries[-1].split()))


def document_pixels(path):
    data = path.read_bytes()
    assert len(data) >= 48 and data[:8] in (b"RPLALP01", b"RPLALP02", b"RPLALP03", b"RPLALP04")
    assert hashlib.sha256(data[:-32]).digest() == data[-32:], f"invalid checksum: {path}"
    metadata_size = struct.unpack_from("<I", data, 8)[0]
    state = json.loads(data[12:12 + metadata_size])
    offset = 12 + metadata_size
    count_tiles = struct.unpack_from("<I", data, offset)[0]
    offset += 4
    has_opaque_pixel = False
    for _ in range(count_tiles):
        offset += 16
        tile = data[offset:offset + 64 * 64 * 4]
        assert len(tile) == 64 * 64 * 4
        has_opaque_pixel |= any(tile[3::4])
        offset += len(tile)
    return state, count_tiles, has_opaque_pixel


def launch(root_dir, name):
    global process, log_path, window, launch_number
    launch_number += 1
    log_name = "".join(character.lower() if character.isalnum() else "-"
                       for character in name).strip("-")
    # diagnostics.rs truncates canvas-demo.log on every process startup. Give
    # each app launch its own directory so marker observations cannot reset or
    # leak across the close/reopen boundaries this workflow exercises.
    log_directory = root_dir / "logs" / f"{launch_number:02d}-{log_name}"
    log_directory.mkdir(parents=True, exist_ok=False)
    log_path = log_directory / "canvas-demo.log"
    token = f"RPL_CLOSE_SAVE_{name}_{os.getpid()}"
    env = {
        **os.environ,
        "RPL_TEST_PATH_DIALOG": "1",
        "RPL_TEST_DOCUMENT_NAME": name,
        "RPL_UI_TEST_WINDOW_TOKEN": token,
        "RPL_LOG_DIR": str(log_directory),
        "XDG_STATE_HOME": str(root_dir / "state"),
    }
    executable = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/canvas-demo").resolve()
    process = subprocess.Popen([str(executable)], cwd=root_dir, env=env,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    window = wait(lambda: find_window(token), "visible application window")
    wait(lambda: any(line.startswith("UI_TEST_HOME_HEADLINE_CONTRAST:")
                     for line in lines()), "first visible Home frame")
    raise_window(display, window)
    home_rects_before_resize = count("UI_TEST_HOME_OPEN_PROJECT_BUTTON_RECT:")
    resize_window(1280, 800)
    set_focus(display, window, 2, 0)
    wait(lambda: count("UI_TEST_HOME_OPEN_PROJECT_BUTTON_RECT:") > home_rects_before_resize
         and stable_rect("UI_TEST_HOME_OPEN_PROJECT_BUTTON_RECT"),
         "stable Home action geometry after resize")
    send()


def draw_stroke(points):
    before = count("UI_TEST_MOUSE_STROKE_COMMITTED:")
    move(*points[0])
    assert button(display, 1, 1, 0)
    for x, y in points[1:]:
        move(x, y)
    assert button(display, 1, 0, 0)
    send()
    wait(lambda: count("UI_TEST_MOUSE_STROKE_COMMITTED:") > before,
         "actual routed and committed mouse stroke")


def create_and_paint():
    create_rects_before = count("UI_TEST_NEW_DOCUMENT_CREATE_RECT:")
    chord("Control_L", "n")
    wait(lambda: "TEST NEW DOCUMENT" in window_title(window), "New Artwork dialog")
    create = wait(lambda: count("UI_TEST_NEW_DOCUMENT_CREATE_RECT:") > create_rects_before
                  and stable_rect("UI_TEST_NEW_DOCUMENT_CREATE_RECT"),
                  "Create document button")
    click((create[0] + create[2] / 2) * create[4],
          (create[1] + create[3] / 2) * create[4])
    wait(lambda: any(line.startswith("UI_TEST_DOCUMENT_CREATED:") for line in lines()),
         "document creation")
    left, top, right, bottom, _, _ = wait(
        canvas_bounds, "stable visible document bounds after creation")
    start = (left + 0.36 * (right - left), top + 0.48 * (bottom - top))
    end = (left + 0.43 * (right - left), top + 0.56 * (bottom - top))
    draw_stroke((start, ((start[0] + end[0]) / 2, (start[1] + end[1]) / 2), end))


def request_close():
    press("Escape")


def finish_successfully(saved_as=False):
    assert process.wait(timeout=20) == 0, "application did not close cleanly after a successful save"
    expected_marker = "UI_TEST_PROJECT_SAVE: complete" if saved_as else "CLOSE_SAVE_COMPLETE:"
    assert expected_marker in "\n".join(lines()), \
        f"successful close path did not record {expected_marker}"


def inject_non_directory_drafts_path(state_home):
    state_root = state_home / "rust-paint-lab"
    state_root.mkdir(parents=True, exist_ok=True)
    bad_path = state_root / "drafts"
    bad_path.write_text("intentionally blocks draft directory creation")
    return bad_path


def inject_permission_denied_drafts_path(state_home):
    state_root = state_home / "rust-paint-lab"
    state_root.mkdir(parents=True, exist_ok=True)
    drafts_path = state_root / "drafts"
    drafts_path.mkdir()
    drafts_path.chmod(0o555)
    return drafts_path


def wait_close_error():
    return wait(lambda: marker("UI_TEST_CLOSE_SAVE_ERROR_VISIBLE"),
                "diagnostic close-save error in the open application")


def recover_drafts_path(path):
    path.unlink()
    path.mkdir()


try:
    if not display:
        raise SystemExit("Xvfb/XTest display is required")
    with tempfile.TemporaryDirectory(prefix="bitchpixel-close-save-") as temporary:
        root_dir = Path(temporary)
        state_home = root_dir / "normal-state"
        state_home.mkdir()
        launch(state_home, "close round trip")
        recovery_complete_before = count("RECOVERY_CHECKPOINT_COMPLETE:")
        create_and_paint()

        recovery_dir = state_home / "state" / "rust-paint-lab" / "recovery"
        wait(lambda: count("RECOVERY_CHECKPOINT_COMPLETE:") > recovery_complete_before,
             "periodic recovery checkpoint after a real painted stroke", timeout=20)
        checkpoints = sorted(recovery_dir.glob("recovery-*.rpl"))
        assert checkpoints, "periodic recovery checkpoint file was not written"
        _checkpoint_state, checkpoint_tiles, checkpoint_opaque = document_pixels(checkpoints[-1])
        assert checkpoint_tiles and checkpoint_opaque, "recovery checkpoint omitted the painted pixels"

        request_close()
        finish_successfully()
        drafts_dir = state_home / "state" / "rust-paint-lab" / "drafts"
        drafts = sorted(drafts_dir.glob("draft-*.rpl"))
        assert len(drafts) == 1, f"untitled close should preserve one editable draft: {drafts}"
        draft_state, draft_tiles, draft_opaque = document_pixels(drafts[0])
        assert draft_state["width"] > 0 and draft_tiles and draft_opaque, \
            "draft did not preserve actual raster artwork"
        initial_bytes = drafts[0].read_bytes()

        launch(state_home, "reopened draft")
        picker_before = count("UI_TEST_FILE_PICKER_PATH_RECT:")
        open_clicked_before = count("UI_TEST_HOME_OPEN_PROJECT_CLICKED")
        open_button = wait(lambda: stable_rect("UI_TEST_HOME_OPEN_PROJECT_BUTTON_RECT"),
                           "Home's Open project action")
        click((open_button[0] + open_button[2] / 2) * open_button[4],
              (open_button[1] + open_button[3] / 2) * open_button[4])
        wait(lambda: count("UI_TEST_HOME_OPEN_PROJECT_CLICKED") > open_clicked_before,
             "Home Open project pointer action")
        path_rect = wait(lambda: stable_rect("UI_TEST_FILE_PICKER_PATH_RECT")
                         if count("UI_TEST_FILE_PICKER_PATH_RECT:") > picker_before else None,
                         "real Open project path field")
        click((path_rect[0] + path_rect[2] / 2) * path_rect[4],
              (path_rect[1] + path_rect[3] / 2) * path_rect[4])
        chord("Control_L", "a")
        type_text(str(drafts[0]))
        press("Return")
        wait(lambda: any(line.startswith("UI_TEST_PROJECT_OPENED: " + str(drafts[0]))
                         for line in lines()), "application reopening the saved untitled draft")
        complete_before = count("RECOVERY_CHECKPOINT_COMPLETE:")
        left, top, right, bottom, _, _ = wait(
            canvas_bounds, "stable document bounds after reopening the draft")
        start = (left + 0.44 * (right - left), top + 0.50 * (bottom - top))
        end = (left + 0.52 * (right - left), top + 0.58 * (bottom - top))
        draw_stroke((start, ((start[0] + end[0]) / 2, (start[1] + end[1]) / 2), end))
        wait(lambda: count("RECOVERY_CHECKPOINT_COMPLETE:") > complete_before,
             "periodic recovery checkpoint after editing the reopened draft", timeout=20)
        request_close()
        finish_successfully()
        final_bytes = drafts[0].read_bytes()
        final_state, final_tiles, final_opaque = document_pixels(drafts[0])
        assert final_bytes != initial_bytes and final_tiles and final_opaque, \
            "named close failed to persist the later edit to the reopened draft"
        assert final_state["width"] == draft_state["width"]
        print("CLOSE_SAVE_ROUND_TRIP: PASSED (paint, periodic checkpoint, untitled close draft, app reopen, edit and named close)", flush=True)

        # More than eight existing drafts must neither block close-time saving
        # nor hide older drafts from the paged Home browser.
        many_state = root_dir / "many-drafts-state"
        many_state.mkdir()
        many_drafts_dir = many_state / "state" / "rust-paint-lab" / "drafts"
        many_drafts_dir.mkdir(parents=True)
        seeded_drafts = {}
        for index in range(100):
            path = many_drafts_dir / f"draft-{index:019d}-1234-{index}-seed-{index}.rpl"
            path.write_bytes(initial_bytes)
            seeded_drafts[path] = initial_bytes

        launch(many_state, "many drafts")
        create_and_paint()
        request_close()
        finish_successfully()
        many_drafts = list(many_drafts_dir.glob("draft-*.rpl"))
        assert len(many_drafts) == 101, \
            f"close with 100 prior drafts should create one additional draft: {len(many_drafts)}"
        for path, original in seeded_drafts.items():
            assert path.read_bytes() == original, f"existing draft was modified: {path}"
        created_drafts = [path for path in many_drafts if path not in seeded_drafts]
        assert len(created_drafts) == 1 and document_pixels(created_drafts[0])[2], \
            "close-time save with 100 existing drafts did not preserve the newly painted artwork"
        print("CLOSE_SAVE_MANY_DRAFTS: PASSED (100 existing files preserved; new artwork saved)", flush=True)

        page_marker_prefix = "UI_TEST_SAVED_DRAFTS_PAGE:"
        next_rect_prefix = "UI_TEST_SAVED_DRAFTS_NEXT_RECT:"
        next_hover_prefix = "UI_TEST_SAVED_DRAFTS_NEXT_HOVERED:"
        next_click_prefix = "UI_TEST_SAVED_DRAFTS_NEXT_CLICKED:"
        launch(many_state, "reopen many drafts")
        wait(lambda: "page=1 pages=13" in (marker(page_marker_prefix) or ""),
             "new application rendering the first saved-draft page")
        wait(lambda: stable_rect("UI_TEST_SAVED_DRAFTS_NEXT_RECT"),
             "new application rendering the saved-draft Next control")
        page = 0
        while page < 13:
            marker_text = f"page={page + 1} pages=13"
            wait(lambda: marker_text in (marker(page_marker_prefix) or ""),
                 f"saved-draft browser reaching {marker_text}")
            if page == 12:
                assert "range=96..101 total=101" in "\n".join(lines()), \
                    "the final bounded batch did not expose the oldest drafts"
                break
            next_button = scroll_rect_into_view("UI_TEST_SAVED_DRAFTS_NEXT_RECT")
            page_markers_before_click = count(page_marker_prefix)
            next_hovers_before = count(next_hover_prefix)
            next_clicks_before = count(next_click_prefix)
            move((next_button[0] + next_button[2] / 2) * next_button[4],
                 (next_button[1] + next_button[3] / 2) * next_button[4])
            wait(lambda: count(next_hover_prefix) > next_hovers_before,
                 "the real pointer hovering the saved-draft Next button")
            assert button(display, 1, 1, 0)
            send()
            assert button(display, 1, 0, 0)
            send()
            wait(lambda: count(next_click_prefix) > next_clicks_before,
                 "the real pointer click activating saved-draft Next")
            wait(lambda: count(page_marker_prefix) > page_markers_before_click
                 and marker_text in (marker(page_marker_prefix) or ""),
                 f"saved-draft browser advancing to page={page + 2} pages=13")
            page += 1

        open_clicked_before = count("UI_TEST_HOME_OPEN_PROJECT_CLICKED")
        open_button = scroll_rect_into_view("UI_TEST_HOME_OPEN_PROJECT_BUTTON_RECT")
        click((open_button[0] + open_button[2] / 2) * open_button[4],
              (open_button[1] + open_button[3] / 2) * open_button[4])
        wait(lambda: count("UI_TEST_HOME_OPEN_PROJECT_CLICKED") > open_clicked_before,
             "Home Open project action after browsing all saved-draft pages")
        path_rect = wait(lambda: stable_rect("UI_TEST_FILE_PICKER_PATH_RECT"),
                         "Open project path field for the new draft")
        click((path_rect[0] + path_rect[2] / 2) * path_rect[4],
              (path_rect[1] + path_rect[3] / 2) * path_rect[4])
        chord("Control_L", "a")
        type_text(str(created_drafts[0]))
        press("Return")
        wait(lambda: any(line.startswith("UI_TEST_PROJECT_OPENED: " + str(created_drafts[0]))
                         for line in lines()),
             "application reopening the new draft with 100 prior drafts")
        reopened_state, reopened_tiles, reopened_opaque = document_pixels(created_drafts[0])
        assert reopened_state["width"] > 0 and reopened_tiles and reopened_opaque, \
            "reopened draft with many prior files lost the saved artwork"
        print("DRAFT_BROWSER_100: PASSED (13 bounded pages expose all 101 drafts; new artwork reopens)", flush=True)
        request_close()
        finish_successfully()

        if hasattr(os, "geteuid") and os.geteuid() != 0:
            permission_state = root_dir / "permission-state"
            permission_state.mkdir()
            permission_drafts_dir = inject_permission_denied_drafts_path(
                permission_state / "state")
            launch(permission_state, "permission recovery")
            create_and_paint()
            request_close()
            permission_error = wait_close_error()
            assert "permission denied" in permission_error.lower(), \
                f"close-time save should expose the real permission error: {permission_error}"
            assert process.poll() is None, \
                "permission failure closed the app and risked discarding the artwork"
            permission_drafts_dir.chmod(0o755)
            retry = wait(lambda: stable_rect("UI_TEST_CLOSE_SAVE_RETRY_RECT"),
                         "Retry save after restoring draft-directory permissions")
            click((retry[0] + retry[2] / 2) * retry[4],
                  (retry[1] + retry[3] / 2) * retry[4])
            finish_successfully()
            permission_saved = list(permission_drafts_dir.glob("draft-*.rpl"))
            assert len(permission_saved) == 1 and document_pixels(permission_saved[0])[2], \
                "Retry after a genuine permission failure did not preserve painted artwork"
            print("CLOSE_SAVE_PERMISSION: PASSED (real permission error shown; Retry preserved artwork)", flush=True)
        else:
            print("CLOSE_SAVE_PERMISSION: SKIPPED (root bypasses directory permissions)", flush=True)

        cancel_state = root_dir / "cancel-state"
        cancel_state.mkdir()
        cancel_bad_path = inject_non_directory_drafts_path(cancel_state / "state")
        launch(cancel_state, "cancel close")
        create_and_paint()
        request_close()
        wait_close_error()
        cancel = wait(lambda: stable_rect("UI_TEST_CLOSE_SAVE_CANCEL_RECT"),
                      "Cancel close target")
        cancelled_before = count("UI_TEST_CLOSE_SAVE_CANCELLED")
        click((cancel[0] + cancel[2] / 2) * cancel[4],
              (cancel[1] + cancel[3] / 2) * cancel[4])
        wait(lambda: count("UI_TEST_CLOSE_SAVE_CANCELLED") > cancelled_before,
             "Cancel leaves the failed close dialog and artwork open")
        assert process.poll() is None, "Cancel closed the application after a failed save"
        draw_stroke(((540, 610), (565, 620), (590, 630)))
        recover_drafts_path(cancel_bad_path)
        request_close()
        finish_successfully()
        cancel_drafts = list(cancel_bad_path.glob("draft-*.rpl"))
        assert len(cancel_drafts) == 1 and document_pixels(cancel_drafts[0])[2], \
            "artwork was not editable and preservable after Cancel"
        print("CLOSE_SAVE_CANCEL: PASSED (Cancel kept the app open; subsequent close preserved artwork)", flush=True)

        save_as_state = root_dir / "save-as-state"
        save_as_state.mkdir()
        inject_non_directory_drafts_path(save_as_state / "state")
        launch(save_as_state, "save as after failure")
        create_and_paint()
        request_close()
        wait_close_error()
        save_as = wait(lambda: stable_rect("UI_TEST_CLOSE_SAVE_AS_RECT"),
                       "Save As close target")
        click((save_as[0] + save_as[2] / 2) * save_as[4],
              (save_as[1] + save_as[3] / 2) * save_as[4])
        path_rect = wait(lambda: stable_rect("UI_TEST_FILE_PICKER_PATH_RECT"),
                         "Save As project path field")
        save_path = save_as_state / "artist-selected-location.rpl"
        click((path_rect[0] + path_rect[2] / 2) * path_rect[4],
              (path_rect[1] + path_rect[3] / 2) * path_rect[4])
        chord("Control_L", "a")
        type_text(str(save_path))
        press("Return")
        finish_successfully(saved_as=True)
        saved_state, saved_tiles, saved_opaque = document_pixels(save_path)
        assert saved_state["width"] > 0 and saved_tiles and saved_opaque, \
            "Save As did not preserve painted artwork after draft storage failed"
        print("CLOSE_SAVE_AS: PASSED (manual project destination saved actual artwork and closed safely)", flush=True)
finally:
    if process and process.poll() is None:
        process.terminate()
        process.wait(timeout=10)
    if display:
        api(X11, "XCloseDisplay", I, D)(display)
