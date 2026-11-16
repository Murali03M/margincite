# Contributing

Thanks for wanting to help. This page covers how the project is laid out, how to run it, and the rules a change has to meet.

## Layout

| Folder | What it is |
|---|---|
| `backend/` | The Rust service: API, crawler, checks, integrations, background jobs |
| `web/` | The web app (React, TypeScript, Vite) |
| `landing/` | The public landing page |
| `shared/` | Styles shared by `web/` and `landing/` |
| `scripts/` | `dev.sh` starts the backend and the web app together |
| `docs/` | Design notes and build plans |

## Running it

You need Rust (the version is pinned in `backend/rust-toolchain.toml`), PostgreSQL 14 or later, Node 20 or later, and pnpm.

```bash
createdb margincite_dev
cp backend/.env.example backend/.env    # then set DATABASE_URL for your machine
pnpm --dir web install
bash scripts/dev.sh
```

Open http://localhost:5174. `backend/README.md` and `web/README.md` have the details.

## Tests

```bash
# backend
cd backend
cargo test -- --test-threads=4
cargo clippy --all-targets -- -D warnings

# web app
pnpm --dir web exec tsc --noEmit
pnpm --dir web exec vitest run tests/unit
pnpm --dir web build && pnpm --dir web exec playwright test
```

Backend tests need a local PostgreSQL. They create and remove their own temporary databases.

## Rules for a change

- **Write the test first.** A change comes with a test that failed before it and passes after it. Line coverage stays at 80% or higher for `backend/src` and `web/src`.
- **Tests never call a real outside service or website.** Google, Bing, the AI providers and crawled sites are always stood in for by a local mock server. Every outside client takes its base address as an argument for this reason.
- **No secrets, anywhere.** No real keys, tokens, personal email addresses or machine paths in code, tests, examples, commit messages or issues.
- **Stored secrets are write-only.** An endpoint may say a secret is set. It never returns it, and it never reaches a log.
- **Every stored row belongs to a workspace**, and every query filters by it. Another workspace's data is reported as "not found".
- **Crawled and AI-generated text is untrusted.** Store it and show it as text. Never run it or render it as markup.
- **Copy is plain and honest.** Say what was found and how to fix it. Never promise a ranking or a citation. Say where a number comes from.
- **Keep files small:** under 800 lines, functions under 50, organised by feature.
- **New dependencies need a permissive licence** (MIT, Apache-2.0, BSD, ISC, MPL-2.0 or similar).

## Commit messages

```
<type>: <description>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`. No attribution or co-author lines.

## Licence

Margincite is licensed under the GNU Affero General Public License, version 3 (see `LICENSE`). By contributing, you agree that your contribution is licensed under the same terms.

In short: anyone may use, change and share Margincite, and anyone who runs a changed version as a service for others must make their changes available under the same licence.

The landing page began from an MIT-licensed design, credited in `landing/NOTICE.md`. Keep that notice.

## Reporting a security problem

See `SECURITY.md`. Please do not open a public issue for it.
