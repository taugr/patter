# Patter public information pages

Static homepage and privacy policy for Google OAuth branding. No JavaScript, external fonts, analytics or credentials. All assets are relative so the pages can be served at a domain root or under a subdirectory. The VitePress guide is built separately into `docs/.vitepress/dist/` and published at `/docs/`.

Preview from the repository root:

```sh
python3 -m http.server 1421 --bind 127.0.0.1 --directory website
```

Hosted on GitHub Pages at `https://patter.labs.tau.gr/`, with the privacy policy at `https://patter.labs.tau.gr/privacy.html` and the guide at `https://patter.labs.tau.gr/docs/`. The Pages workflow copies this directory to the site root and the VitePress output to `/docs/`. Cloudflare DNS points this hostname to `taugr.github.io` without proxying. GitHub Pages provides HTTPS. Verify the homepage and privacy URLs before using them in Google OAuth branding.

Only deploy the assembled site artifact. Never deploy the repository root or `.secrets/`. Publishing these pages does not publish an application release.
