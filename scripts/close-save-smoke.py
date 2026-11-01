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
fetch_name = api(X11, "XFetchName", I, D, W, c.POINTER(c.c_void_p))
free = api(X11, "XFree", I, c.c_void_p)
translate = api(X11, "XTranslateCoordinates", I, D, W, W, I, I,
                c.POINTER(I), c.POINTER(I), c.POINTER(W))
set_focus = api(X11, "XSetInputFocus", I, D, W, I, W)
resize = api(X11, "XResizeWindow", I, D, W, U, U)
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
    raise AssertionError(f"timeout waiting for {description}; last value={last!r}")


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
            "/": "slash", "-": "minus", ".": "period", "_": "minus",
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


def last_rect(name):
    prefix = name + ": "
    entries = [line[len(prefix):] for line in lines() if line.startswith(prefix)]
    return tuple(map(float, entries[-1].split())) if entries else None


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
    global process, log_path, window
    log_path = root_dir / "logs" / "canvas-demo.log"
    token = f"RPL_CLOSE_SAVE_{name}_{os.getpid()}"
    env = {
        **os.environ,
        "RPL_TEST_PATH_DIALOG": "1",
        "RPL_TEST_DOCUMENT_NAME": name,
        "RPL_UI_TEST_WINDOW_TOKEN": token,
        "RPL_LOG_DIR": str(log_path.parent),
        "XDG_STATE_HOME": str(root_dir / "state"),
    }
    executable = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/canvas-demo").resolve()
    process = subprocess.Popen([str(executable)], cwd=root_dir, env=env,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    window = wait(lambda: find_window(token), "visible application window")
    set_focus(display, window, 2, 0)
    resize(display, window, 1280, 800)
    send()


def create_and_paint():
    chord("Control_L", "n")
    wait(lambda: "TEST NEW DOCUMENT" in window_title(window), "New Artwork dialog")
    create = wait(lambda: last_rect("UI_TEST_NEW_DOCUMENT_CREATE_RECT"),
                  "stable Create document button")
    click((create[0] + create[2] / 2) * create[4],
          (create[1] + create[3] / 2) * create[4])
    wait(lambda: any(line.startswith("UI_TEST_DOCUMENT_CREATED:") for line in lines()),
         "document creation")
    # These are established on-canvas coordinates from the full alpha XTest
    # workflow at 1280x800; file pixel checks below verify they painted.
    move(520, 540)
    assert button(display, 1, 1, 0)
    for x, y in ((550, 550), (580, 565), (610, 575)):
        move(x, y)
    assert button(display, 1, 0, 0)
    send()


def request_close():
    press("Escape")


def finish_successfully():
    assert process.wait(timeout=20) == 0, "application did not close cleanly after a successful save"
    assert "CLOSE_SAVE_COMPLETE:" in "\n".join(lines()), "close save completion was not recorded"


def inject_non_directory_drafts_path(state_home):
    state_root = state_home / "rust-paint-lab"
    state_root.mkdir(parents=True, exist_ok=True)
    bad_path = state_root / "drafts"
    bad_path.write_text("intentionally blocks draft directory creation")
    return bad_path


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
        chord("Control_L", "o")
        path_rect = wait(lambda: last_rect("UI_TEST_FILE_PICKER_PATH_RECT")
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
        move(570, 520)
        assert button(display, 1, 1, 0)
        for x, y in ((590, 520), (615, 530), (640, 540)):
            move(x, y)
        assert button(display, 1, 0, 0)
        send()
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

        # A file where the draft directory should live reproduces an actual
        # local storage failure deterministically. Keep the application open,
        # confirm the reason is visible, then repair the directory and retry.
        retry_state = root_dir / "retry-state"
        retry_state.mkdir()
        bad_path = inject_non_directory_drafts_path(retry_state / "state")
        launch(retry_state, "retry draft")
        create_and_paint()
        request_close()
        error = wait_close_error()
        assert "drafts" in error and ("File exists" in error or "Not a directory" in error), error
        assert process.poll() is None, "failed close discarded the artwork instead of keeping the app open"
        retry = wait(lambda: last_rect("UI_TEST_CLOSE_SAVE_RETRY_RECT"), "Retry save target")
        recover_drafts_path(bad_path)
        click((retry[0] + retry[2] / 2) * retry[4],
              (retry[1] + retry[3] / 2) * retry[4])
        finish_successfully()
        retry_drafts = list(bad_path.glob("draft-*.rpl"))
        assert len(retry_drafts) == 1 and document_pixels(retry_drafts[0])[2], \
            "Retry save after storage recovery did not preserve painted artwork"
        print("CLOSE_SAVE_RETRY: PASSED (filesystem failure surfaced; artwork retained; Retry saved after storage repair)", flush=True)

        cancel_state = root_dir / "cancel-state"
        cancel_state.mkdir()
        cancel_bad_path = inject_non_directory_drafts_path(cancel_state / "state")
        launch(cancel_state, "cancel close")
        create_and_paint()
        request_close()
        wait_close_error()
        cancel = wait(lambda: last_rect("UI_TEST_CLOSE_SAVE_CANCEL_RECT"), "Cancel close target")
        click((cancel[0] + cancel[2] / 2) * cancel[4],
              (cancel[1] + cancel[3] / 2) * cancel[4])
        wait(lambda: count("UI_TEST_CLOSE_SAVE_CANCELLED:") > 0,
             "Cancel leaves the failed close dialog and artwork open")
        assert process.poll() is None, "Cancel closed the application after a failed save"
        move(540, 610)
        assert button(display, 1, 1, 0)
        move(565, 620)
        move(590, 630)
        assert button(display, 1, 0, 0)
        send()
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
        save_as = wait(lambda: last_rect("UI_TEST_CLOSE_SAVE_AS_RECT"), "Save As close target")
        click((save_as[0] + save_as[2] / 2) * save_as[4],
              (save_as[1] + save_as[3] / 2) * save_as[4])
        path_rect = wait(lambda: last_rect("UI_TEST_FILE_PICKER_PATH_RECT"),
                         "Save As project path field")
        save_path = save_as_state / "artist-selected-location.rpl"
        click((path_rect[0] + path_rect[2] / 2) * path_rect[4],
              (path_rect[1] + path_rect[3] / 2) * path_rect[4])
        chord("Control_L", "a")
        type_text(str(save_path))
        press("Return")
        finish_successfully()
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
