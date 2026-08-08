"""M0: git watcher emits a card on change, dedupes on no-change."""
import subprocess

from brain.distill import GitWatcher
from brain.feed import CardStore


def _run(cwd, *args):
    subprocess.run(args, cwd=cwd, check=True, capture_output=True)


def test_watcher_emits_and_dedupes(tmp_path):
    repo = str(tmp_path)
    _run(repo, "git", "init", "-q")
    _run(repo, "git", "commit", "-q", "--allow-empty", "-m", "init")
    (tmp_path / "a.py").write_text("x = 1\n")

    store = CardStore()
    watcher = GitWatcher(repo, store)
    assert watcher.poll_once() is True
    assert watcher.poll_once() is False  # same fingerprint -> no duplicate card

    # untracked-file EDITS are invisible to the fingerprint (known M0 limit,
    # improved in lane A) — track the file so the diff registers:
    _run(repo, "git", "add", "a.py")
    (tmp_path / "a.py").write_text("x = 2\ny = 3\n")
    assert watcher.poll_once() is True

    cards = store.since(0)
    assert len(cards) == 2
    assert all(c.kind == "real" for c in cards)
    assert "a.py" in cards[0].title or "a.py" in cards[0].body
