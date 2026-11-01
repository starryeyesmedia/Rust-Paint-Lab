"""Pair New Artwork geometry emitted during a single egui frame."""

NAME_PREFIX = "UI_TEST_NEW_DOCUMENT_NAME_RECT: "
PORTRAIT_PREFIX = "UI_TEST_FORMAT_CARD_RECT|Portrait|"
CREATE_PREFIX = "UI_TEST_NEW_DOCUMENT_CREATE_RECT: "


def _rect(line, prefix):
    if not line.startswith(prefix):
        return None
    try:
        values = tuple(float(value) for value in line[len(prefix) :].split()[:5])
    except ValueError:
        return None
    return values if len(values) == 5 else None


def stable_new_document_geometry(lines, baseline_name_count):
    """Return two-frame-stable, same-frame name/card/create geometry.

    New Artwork writes its name marker, format-card markers, and Create marker
    synchronously during each UI frame. Pairing within those frame boundaries
    prevents a resize from comparing a current name field with a previous
    frame's still-stable card geometry. `baseline_name_count` excludes frames
    that were already present before the caller began waiting for a layout.
    """
    name_count = 0
    frame = None
    completed = []

    def finish_frame(current):
        if current is None:
            return
        if all(current[key] is not None for key in ("name", "portrait", "create")):
            completed.append((current["name"], current["portrait"], current["create"]))

    for line in lines:
        name = _rect(line, NAME_PREFIX)
        if name is not None:
            finish_frame(frame)
            name_count += 1
            frame = (
                {"name": name, "portrait": None, "create": None}
                if name_count > baseline_name_count
                else None
            )
            continue
        if frame is None:
            continue
        if frame["portrait"] is None:
            frame["portrait"] = _rect(line, PORTRAIT_PREFIX)
        if frame["portrait"] is not None and frame["create"] is None:
            frame["create"] = _rect(line, CREATE_PREFIX)

    finish_frame(frame)
    if frame is None or any(frame[key] is None for key in ("name", "portrait", "create")):
        return None
    if len(completed) < 2 or completed[-1] != completed[-2]:
        return None
    return completed[-1]
