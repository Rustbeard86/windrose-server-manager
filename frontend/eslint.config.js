import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
    rules: {
      // Data-fetching callbacks invoked from effects is a standard React pattern.
      // The async state updates happen after the effect body returns, so there is
      // no synchronous cascade risk. Disabling to allow the fetch-in-effect idiom.
      'react-hooks/set-state-in-effect': 'off',
      // Updating a ref's .current outside of effects is a well-established React
      // pattern for keeping a "latest callback" ref in sync across renders.
      'react-hooks/refs': 'off',
    },
  },
])
