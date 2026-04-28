import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const site = 'https://docs.refact.ai/';

// https://astro.build/config
export default defineConfig({
  integrations: [
    starlight({
      title: 'Refact Documentation',
      components: {
        Search: './src/components/Search.astro',
        Head: './src/components/Head.astro'
      },
      logo: {
        light: '/src/assets/logo-light.svg',
        dark: '/src/assets/logo-dark.svg',
        replacesTitle: true,
      },
      social: {
        github: 'https://github.com/smallcloudai'
      },
      head: [
        {
          tag: 'meta',
          attrs: { property: 'og:image', content: site + 'og.jpg' }
        },
        {
          tag: 'meta',
          attrs: { property: 'twitter:image', content: site + 'og.jpg' }
        }
      ],
      sidebar: [
        {
          label: 'Introduction',
          collapsed: true,
          items: [
            { 
              label: 'Quickstart', 
              link: '/introduction/quickstart/',
              attrs: {
                'aria-label': 'Get started with Refact'
              }
            },
            {
              label: 'Installation',
              collapsed: true,
              items: [
                { 
                  label: 'Installation Hub', 
                  link: '/installation/installation-hub/',
                  attrs: {
                    'aria-label': 'Browse Installation Options'
                  }
                },
                { 
                  label: 'VS Code', 
                  link: '/installation/vs-code/',
                  attrs: {
                    'aria-label': 'Install Refact for VS Code'
                  }
                },
                { 
                  label: 'JetBrains IDEs', 
                  link: '/installation/jetbrains/',
                  attrs: {
                    'aria-label': 'Install Refact for JetBrains IDEs'
                  }
                },
              ] 
            },
            {
              label: 'Features',
              collapsed: true,
              items: [
                { 
                  label: 'AI Chat', 
                  link: '/features/ai-chat/',
                  attrs: {
                    'aria-label': 'Learn about AI Chat Feature'
                  }
                },
                { 
                  label: 'AI Toolbox', 
                  link: '/features/ai-toolbox/',
                  attrs: {
                    'aria-label': 'Explore AI Toolbox Features'
                  }
                },
                { 
                  label: 'Code Completion', 
                  link: '/features/code-completion/',
                  attrs: {
                    'aria-label': 'Learn about Code Completion'
                  }
                },
                { 
                  label: 'Context', 
                  link: '/features/context/',
                  attrs: {
                    'aria-label': 'Understanding Context Features'
                  }
                },
              ]
            },
          ],
        },
        {
          label: 'Autonomous Agent',
          collapsed: true,
          items: [
            { label: 'Getting Started', link: '/features/autonomous-agent/getting-started/' },
            { label: 'Overview', link: '/features/autonomous-agent/overview/' },
            { 
              label: 'Tools', 
              link: '/features/autonomous-agent/tools/',
              attrs: {
                'aria-label': 'Learn about Agent Tools'
              }
            },
            { 
              label: 'Rollback', 
              link: '/features/autonomous-agent/rollback/',
              attrs: {
                'aria-label': 'Learn about Agent Rollback Feature'
              }
            },
            { 
              label: 'Integrations', 
              collapsed: true,
              items: [
                { label: 'Overview', link: '/features/autonomous-agent/integrations/' },
                // Development Tools
    		{ label: 'Chrome', link: '/features/autonomous-agent/integrations/chrome/' },
                { label: 'Shell Commands', link: '/features/autonomous-agent/integrations/shell-commands/' },
                { label: 'Command Line Tool', link: '/features/autonomous-agent/integrations/command-line-tool/' },
                { label: 'Command Line Service', link: '/features/autonomous-agent/integrations/command-line-service/' },
                { label: 'MCP Server', link: '/features/autonomous-agent/integrations/mcp/', attrs: { 'aria-label': 'Connect to Model Context Protocol servers' } },
                // Version Control
                { label: 'GitHub', link: '/features/autonomous-agent/integrations/github/' },
                { label: 'GitLab', link: '/features/autonomous-agent/integrations/gitlab/' },
                // Container Management
                { label: 'Docker', link: '/features/autonomous-agent/integrations/docker/' },
                // Databases
                { label: 'PostgreSQL', link: '/features/autonomous-agent/integrations/postgresql/' },
                { label: 'MySQL', link: '/features/autonomous-agent/integrations/mysql/' },
                // Debugging
                { label: 'PDB', link: '/features/autonomous-agent/integrations/pdb/' },
              ] 
            },
          ]
        },
        {
          label: 'Guides',
          collapsed: true,
          items: [
            {
              label: 'Plugins',
              collapsed: true,
              items: [
                { 
                  label: 'JetBrains IDEs', 
                  collapsed: true,
                  items: [
                    { 
                      label: 'Troubleshooting', 
                      link: '/guides/plugins/jetbrains/troubleshooting/',
                      attrs: {
                        'aria-label': 'JetBrains IDEs Troubleshooting Guide'
                      }
                    },
                  ]
                },
              ]
            },
          ]
        },
        {
          label: 'Supported Models',
          link: '/supported-models/',
          attrs: {
            'aria-label': 'View Supported AI Models'
          }
        },
        {
          label: 'Configure Providers (BYOK)',
          link: '/byok/',
          attrs: {
            'aria-label': 'Configure Providers (BYOK) documentation'
          }
        },
        {
          label: 'FAQ',
          link: '/faq/',
          attrs: {
            'aria-label': 'Frequently Asked Questions'
          }
        },
        {
          label: 'Contributing',
          link: '/contributing/',
          attrs: {
            'aria-label': 'Learn how to contribute to Refact'
          }
        },
      ],
      customCss: [
        // Main CSS entry point that imports all other CSS files
        './src/styles/index.css',
      ],
      editLink: {
        baseUrl: 'https://github.com/smallcloudai/refact/edit/main/docs/',
      },
      lastUpdated: true,
    }),
  ],
});
