# React Native's default app library, as the default app build makes it (ReactAndroid/cmake-utils/default-app-setup).
# It runs in a subdirectory of ubrn's project, where CMAKE_PROJECT_NAME still names that project, so the name is set here.
cmake_minimum_required(VERSION 3.13)
project(appmodules)
set(CMAKE_PROJECT_NAME appmodules)
include(${REACT_ANDROID_DIR}/cmake-utils/ReactNative-application.cmake)
