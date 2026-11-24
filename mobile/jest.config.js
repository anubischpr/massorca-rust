module.exports = {
  preset: '@react-native/jest-preset',
  // Navigation and the screen libraries ship ES modules, which Jest must transform like React Native itself.
  transformIgnorePatterns: [
    'node_modules/(?!((jest-)?react-native|@react-native(-community)?|@react-navigation|react-native-screens|react-native-safe-area-context)/)',
  ],
  // The terminal's WebView needs the device: every test gets a double that records the scripts the app injects.
  moduleNameMapper: {
    '^react-native-webview$': '<rootDir>/src/terminal/test-webview.tsx',
  },
};
