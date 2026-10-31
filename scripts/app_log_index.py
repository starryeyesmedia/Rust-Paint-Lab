"""Incremental index for the verbose GUI smoke-test application log."""

from pathlib import Path


class ApplicationLogIndex:
    """Read appended log bytes once and index markers without repeated splits."""

    def __init__(self):
        self._path = None
        self._offset = 0
        self._partial = b""
        self.lines = []
        self.counts = {}
        self.latest = {}
        self.previous = {}
        self._prefix_counts = {}
        self._text_counts = {}

    def _reset(self, path):
        self._path = path
        self._offset = 0
        self._partial = b""
        self.lines.clear()
        self.counts.clear()
        self.latest.clear()
        self.previous.clear()
        self._prefix_counts.clear()
        self._text_counts.clear()

    def refresh(self, path):
        path = Path(path)
        try:
            size = path.stat().st_size
        except OSError:
            return self.lines
        if self._path != path or size < self._offset:
            self._reset(path)
        try:
            with path.open("rb") as log:
                log.seek(self._offset)
                appended = log.read()
        except OSError:
            return self.lines
        self._offset += len(appended)
        if not appended:
            return self.lines

        records = (self._partial + appended).split(b"\n")
        self._partial = records.pop() if not appended.endswith(b"\n") else b""
        if not self._partial and records and records[-1] == b"":
            records.pop()
        for record in records:
            line = record.decode("utf-8", errors="replace").removesuffix("\r")
            self.lines.append(line)
            for prefix in self._prefix_counts:
                if line.startswith(prefix):
                    self._prefix_counts[prefix] += 1
            for text in self._text_counts:
                self._text_counts[text] += line.count(text)
            marker, separator, value = line.partition(": ")
            if separator:
                self.counts[marker] = self.counts.get(marker, 0) + 1
                self.previous[marker] = self.latest.get(marker)
                self.latest[marker] = value
        return self.lines

    def count(self, marker):
        return self.counts.get(marker, 0)

    def value(self, marker):
        return self.latest.get(marker)

    def stable_value(self, marker):
        latest = self.latest.get(marker)
        if latest is not None and latest == self.previous.get(marker):
            return latest
        return None

    def count_prefix(self, prefix):
        if prefix not in self._prefix_counts:
            self._prefix_counts[prefix] = sum(
                line.startswith(prefix) for line in self.lines
            )
        return self._prefix_counts[prefix]

    def count_text(self, text):
        if text not in self._text_counts:
            self._text_counts[text] = sum(line.count(text) for line in self.lines)
        return self._text_counts[text]
