"""Integrator-owned wiring. python -m brain from sidecar/ dir."""
import os

from .distill import GitWatcher
from .feed import CardStore
from .server import make_app

repo = os.environ.get("SIDECAR_REPO", os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..")))
addr = os.environ.get("SIDECAR_ADDR", "127.0.0.1")
port = int(os.environ.get("SIDECAR_PORT", "7717"))

store = CardStore()
store.add(kind="real", title="sidecar online", body=f"watching {repo}",
          justification="startup")
GitWatcher(repo, store).start()
make_app(store, repo).run(host=addr, port=port, threaded=True)
