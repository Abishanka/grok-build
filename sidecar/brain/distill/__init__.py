"""Lane A: context extraction. M0 = git watcher, zero AI, never regresses."""
import subprocess
import threading
import time


def _git(repo: str, *args: str) -> str:
    try:
        out = subprocess.run(
            ["git", "-C", repo, *args],
            capture_output=True, text=True, timeout=10,
        )
        return out.stdout.strip()
    except Exception:
        return ""


class GitWatcher:
    """Polls repo state; emits one card per change of the diff fingerprint."""

    def __init__(self, repo: str, store, interval: float = 5.0):
        self.repo = repo
        self.store = store
        self.interval = interval
        self._last_fingerprint = ""

    def poll_once(self) -> bool:
        stat = _git(self.repo, "diff", "--stat", "HEAD")
        status = _git(self.repo, "status", "--porcelain")
        fingerprint = stat + "\n" + status
        if fingerprint == self._last_fingerprint or not fingerprint.strip():
            return False
        self._last_fingerprint = fingerprint
        last_commit = _git(self.repo, "log", "-1", "--format=%s")
        top_line = stat.splitlines()[0].strip() if stat else status.splitlines()[0].strip()
        self.store.add(
            kind="real",
            title=f"working on: {top_line}",
            body=(stat or status)[:800] + (f"\nlast commit: {last_commit}" if last_commit else ""),
            justification="change detected in your working tree",
        )
        return True

    def run_forever(self):
        while True:
            self.poll_once()
            time.sleep(self.interval)

    def start(self) -> threading.Thread:
        t = threading.Thread(target=self.run_forever, daemon=True)
        t.start()
        return t


def current_context(repo: str) -> str:
    stat = _git(repo, "diff", "--stat", "HEAD")
    top = stat.splitlines()[0].strip() if stat else "clean tree"
    return f"working on {top}"
