import coreWebVitals from 'eslint-config-next/core-web-vitals'
import typescript from 'eslint-config-next/typescript'
import prettier from 'eslint-config-prettier'

/** @type {import('eslint').Linter.Config[]} */
const eslintConfig = [
  {
    ignores: ['.next/**', 'out/**', 'node_modules/**', 'src-tauri/**', 'next-env.d.ts'],
  },
  ...coreWebVitals,
  ...typescript,
  // Desliga as regras de estilo: formatação é assunto do Prettier.
  prettier,
  {
    rules: {
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
    },
  },
]

export default eslintConfig
