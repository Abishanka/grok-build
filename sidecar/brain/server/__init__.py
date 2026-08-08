"""Lane A: the contract surface. Flask, sync, boring."""
from flask import Flask, jsonify, request

from ..distill import current_context


def make_app(store, repo: str) -> Flask:
    app = Flask("sidecar")

    @app.get("/state")
    def state():
        # A2 upgrades this to a real wake heuristic
        return jsonify({"mode": "waiting", "context": current_context(repo)})

    @app.get("/cards")
    def cards():
        since = int(request.args.get("since", 0))
        return jsonify({"cards": [c.to_dict() for c in store.since(since)]})

    @app.post("/action")
    def action():
        payload = request.get_json(force=True, silent=True) or {}
        if payload.get("action") not in ("save", "dismiss", "approve_post"):
            return jsonify({"ok": False, "error": "bad action"}), 400
        # approve_post routes through brain/post once C3 lands
        return jsonify({"ok": True})

    @app.get("/briefing.wav")
    def briefing():
        return "voice not built yet", 503

    return app
