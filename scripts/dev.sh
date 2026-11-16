#!/usr/bin/env bash
# Starts the backend and the web app together. Ctrl-C stops both.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

pids=()

stop() {
	trap - INT TERM EXIT
	for pid in "${pids[@]:-}"; do
		[ -n "$pid" ] && kill "$pid" 2>/dev/null || true
	done
	wait 2>/dev/null || true
}
trap stop INT TERM EXIT

if [ ! -f "$ROOT/backend/.env" ]; then
	echo "backend/.env is missing. Copy backend/.env.example to backend/.env and edit it first." >&2
	exit 1
fi

# Refuse to start a second copy: say plainly what is already running and how to stop it.
for port in 8787 5174; do
	if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
		echo "Something is already listening on port $port, most likely Margincite from an earlier run." >&2
		echo "Stop it with:  kill \$(lsof -ti tcp:$port -sTCP:LISTEN)   then run this script again." >&2
		exit 1
	fi
done

echo "Backend:  http://127.0.0.1:8787"
echo "Web app:  http://localhost:5174  (sign in with the token from backend/.env)"

# The backend reads its settings from the environment, so load backend/.env for it.
(cd "$ROOT/backend" && set -a && . ./.env && set +a && exec cargo run) &
pids+=("$!")
(cd "$ROOT/web" && exec pnpm dev) &
pids+=("$!")

# macOS ships bash 3.2, which has no `wait -n`, so poll: if either half exits, stop the other.
while kill -0 "${pids[0]}" 2>/dev/null && kill -0 "${pids[1]}" 2>/dev/null; do
	sleep 1
done
echo "One of the two processes ended. Stopping the other." >&2
