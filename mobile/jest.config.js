module.exports = {
  preset: '@react-native/jest-preset',
  // Navigation and the screen libraries ship ES modules, which Jest must transform like React Native itself.
  transformIgnorePatterns: [
    'node_modules/(?!((jest-)?react-native|@react-native(-community)?|@react-navigation|react-native-screens|react-native-safe-area-context)/)',
  ],
};
