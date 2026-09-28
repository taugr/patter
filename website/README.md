# Patter public information pages

Static homepage and privacy policy for Google OAuth branding. No build step, JavaScript, external fonts, analytics or credentials. All assets are relative so the pages can be served at a domain root or under a subdirectory.

Preview from the repository root:

```sh
python3 -m http.server 1421 --bind 127.0.0.1 --directory website
```

Hosted on GitHub Pages from the repository's `gh-pages` branch at `https://patter.labs.tau.gr/`, with the privacy policy at `https://patter.labs.tau.gr/privacy.html`. Cloudflare DNS points this hostname to `taugr.github.io` without proxying. GitHub Pages provides HTTPS. Verify both URLs before using them in Google OAuth branding.

Only deploy this directory. Never deploy the repository root or `.secrets/`. Publishing these pages must not publish an application release or the other pending feature changes.
