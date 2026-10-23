import React from 'react';
import { AppRegistry, LogBox, ScrollView, Text } from 'react-native';
import { name as appName } from './app.json';

// Temporary workaround for problem with nested text
// not working currently.
LogBox.ignoreAllLogs();

// A release build that fails while loading would close without a word;
// say what went wrong on screen instead.
let Root;
try {
  Root = require('./src/App').App;
} catch (error) {
  const message = String((error && error.stack) || error);
  Root = () =>
    React.createElement(
      ScrollView,
      { style: { flex: 1, backgroundColor: '#0f1012' }, contentContainerStyle: { padding: 96 } },
      React.createElement(Text, { style: { color: '#e5534b', fontSize: 36, fontWeight: '700', marginBottom: 24 } }, "Nuvora couldn't start"),
      React.createElement(Text, { style: { color: '#f2f2f2', fontSize: 22 } }, message),
    );
}

AppRegistry.registerComponent(appName, () => Root);
