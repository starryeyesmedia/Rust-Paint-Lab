"""Small deterministic helpers shared by Linux GUI smoke workflows."""


class QuietValue:
    """Return observations only after their value stays unchanged for a span."""

    def __init__(self, quiet_seconds):
        if quiet_seconds <= 0:
            raise ValueError("quiet_seconds must be positive")
        self.quiet_seconds = quiet_seconds
        self._value = None
        self._unchanged_since = None

    def observe(self, value, now):
        if value is None:
            self._value = None
            self._unchanged_since = None
            return None
        if value != self._value:
            self._value = value
            self._unchanged_since = now
            return None
        if self._unchanged_since is not None and now - self._unchanged_since >= self.quiet_seconds:
            return value
        return None
