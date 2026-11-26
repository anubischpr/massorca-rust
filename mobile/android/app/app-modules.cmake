# Passed to CMake as CMAKE_PROJECT_INCLUDE (mobile/android/app/build.gradle), so it runs after each project() call, the
# first being the project() of ubrn's CMakeLists.txt, which builds the Rust binding library. This adds React Native's app
# library, libappmodules.so: its JNI_OnLoad registers the core native modules (PlatformConstants among them) and the
# Fabric components. The default app build makes it; this app's explicit CMake path had replaced that build.
if(NOT PROJECT_NAME STREQUAL "appmodules")
  get_filename_component(MASSORCA_APP_DIR "${CMAKE_CURRENT_LIST_FILE}" DIRECTORY)
  add_subdirectory("${MASSORCA_APP_DIR}/app-modules" "${CMAKE_BINARY_DIR}/appmodules")
endif()
