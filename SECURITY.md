# Security

## Reporting a problem

Please do not open a public issue for a security problem.

Use the repository's private reporting instead: on GitHub, open the **Security** tab and choose **Report a vulnerability**. Describe what you found, how to reproduce it, and what an attacker could do with it. You will get a reply as soon as the maintainers can look at it.

## What Margincite handles that needs care

Margincite fetches websites that people type in, stores credentials for outside services, and sends text to AI providers. These are the parts most worth testing.

- **The crawler.** It refuses private, loopback, link-local and cloud-metadata addresses, checks every redirect, and resolves names through a guarded resolver so a name cannot be swapped to an internal address after it was checked. A way to make the server fetch an internal address is a vulnerability.
- **JavaScript rendering.** It is off by default. Turning it on (`CHROME_PATH` together with `RENDER_SITES_I_TRUST=yes`) lets the pages being crawled make the machine request other addresses, and that guard is only partial. Turn it on only on your own machine, for sites you own.
- **Stored credentials.** Provider keys, Google tokens and other secrets are encrypted before they are stored. No API endpoint returns them. A way to read one back is a vulnerability.
- **Workspace separation.** Every stored row belongs to a workspace, and one workspace must never read or change another's data.
- **Crawled and AI-generated text.** It is untrusted. The web app shows it as plain text and only links to `http` and `https` addresses.

## What is not ready for the open internet yet

- **Sign-in is a development token.** `DEV_API_TOKENS` is meant for running Margincite on your own machine. Do not expose an instance that uses it to the internet.
- **Rate limits are per process and per address.** Behind a reverse proxy, every caller shares the proxy's address.

## Keeping your own instance safe

- Never commit `backend/.env` or `backend/.data/`. Both are ignored by git.
- Losing or changing the encryption key makes every stored credential unreadable, so they have to be entered again.
- Use provider keys with the smallest permissions that work, and set spending limits with each provider.
