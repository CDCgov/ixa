// Self-contained Conventional Commits rules.
// We avoid `extends: ['@commitlint/config-conventional']` because with mise-managed
// npm tools the config package may not be resolvable from the repo root.

const TYPES = [
  'chore',
  'ci',
  'dependencies',
  'docs',
  'feat',
  'fix',
  'perf',
  'refactor',
  'revert',
  'release',
  'style',
  'test',
];

module.exports = {
  // Parse: type(scope)!: subject
  parserPreset: {
    parserOpts: {
      headerPattern: /^(\w+)(?:\(([^)]+)\))?(!)?: (.+)$/,
      headerCorrespondence: ['type', 'scope', 'breaking', 'subject'],
    },
  },

  rules: {
    'type-empty': [2, 'never'],
    'type-enum': [2, 'always', TYPES],
    'scope-empty': [0],
    'subject-empty': [2, 'never'],
    'subject-full-stop': [2, 'never', '.'],
    'header-max-length': [2, 'always', 100],
    // Conventional Commits suggests: don't use Sentence/Pascal/Start/UPPER case.
    'subject-case': [2, 'never', ['sentence-case', 'start-case', 'pascal-case', 'upper-case']],
  },
};
