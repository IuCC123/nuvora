const path = require('path');
const {getDefaultConfig, mergeConfig} = require('@react-native/metro-config');

// The data layer and design tokens are shared with the web and Fire TV apps,
// used straight from their TypeScript sources. Their own dependencies
// (thumbhash) come from this app's node_modules: the packages' Bun-installed
// node_modules are hidden, so the build doesn't depend on a Bun install.
//
// No custom resolveRequest: Vega's build supplies its own, which maps
// `react-native` to Vega's React Native, and a project one would replace it.
const packages = path.resolve(__dirname, '../../packages');
const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const config = {
  watchFolders: [path.join(packages, 'core'), path.join(packages, 'tokens')],
  resolver: {
    extraNodeModules: {
      '@nuvora/core': path.join(packages, 'core/src'),
      '@nuvora/tokens': path.join(packages, 'tokens/src'),
    },
    nodeModulesPaths: [path.join(__dirname, 'node_modules')],
    blockList: [new RegExp(`^${escape(packages)}/[^/]+/node_modules/.*`)],
  },
};

module.exports = mergeConfig(getDefaultConfig(__dirname), config);
