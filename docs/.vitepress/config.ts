import { defineConfig } from 'vitepress';
import pkg from '../../package.json';

const siteUrl = 'https://patter.labs.tau.gr/docs/';
const description = 'Install and use Patter, a local-first meeting notebook for Mac.';

export default defineConfig({
  title: 'Patter',
  description,
  base: '/docs/',
  cleanUrls: true,
  sitemap: { hostname: siteUrl },
  srcExclude: ['**/*-plan.md'],
  head: [
    ['link', { rel: 'icon', href: '/docs/patter-icon.svg', type: 'image/svg+xml' }],
    ['meta', { name: 'theme-color', content: '#fbf7ef' }],
    ['meta', { property: 'og:title', content: 'Patter guide' }],
    ['meta', { property: 'og:description', content: description }],
    ['meta', { property: 'og:url', content: siteUrl }],
  ],
  themeConfig: {
    logo: '/patter-icon.svg',
    siteTitle: 'Patter guide',
    search: { provider: 'local' },
    nav: [
      { text: 'Home', link: 'https://patter.labs.tau.gr/' },
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Download', link: 'https://github.com/taugr/patter/releases/latest' },
      { text: `v${pkg.version}`, link: 'https://github.com/taugr/patter/releases' },
    ],
    sidebar: [
      {
        text: 'Start here',
        items: [
          { text: 'Get started', link: '/guide/getting-started' },
          { text: 'Install and update', link: '/guide/install' },
          { text: 'Use Patter', link: '/guide/using-patter' },
        ],
      },
      {
        text: 'Set up Patter',
        items: [
          { text: 'Permissions', link: '/guide/permissions' },
          { text: 'Calendar and meetings', link: '/guide/calendar' },
          { text: 'Local models and templates', link: '/guide/models' },
        ],
      },
      {
        text: 'Your library',
        items: [
          { text: 'Storage and Anarlog import', link: '/guide/library' },
          { text: 'Google Drive backup', link: '/google-drive-backup' },
          { text: 'Connect an agent', link: '/agent-access' },
        ],
      },
    ],
    socialLinks: [{ icon: 'github', link: 'https://github.com/taugr/patter' }],
    footer: {
      message: 'A personal meeting notebook for Mac.',
      copyright: '© 2026 Thomas Auger',
    },
  },
});
