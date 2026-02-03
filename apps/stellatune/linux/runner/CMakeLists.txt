cmake_minimum_required(VERSION 3.13)
project(runner LANGUAGES CXX)

# ==============================================================================
# Rust Integration: Build stellatune-ffi automatically
# ==============================================================================

# Define the root path relative to this file
# linux/runner -> linux -> apps -> repo root
set(PROJECT_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../../../")
set(CRATE_DIR "${PROJECT_ROOT}/crates/stellatune-ffi")

# Build profile:
# - Release/Profile -> cargo --release
# - Debug -> cargo build (debug)
if(CMAKE_BUILD_TYPE STREQUAL "Release" OR CMAKE_BUILD_TYPE STREQUAL "Profile")
  set(CARGO_ARGS "--release")
  set(TARGET_DIR "release")
else()
  set(CARGO_ARGS "")
  set(TARGET_DIR "debug")
endif()

set(RUST_LIB_FILENAME "libstellatune_ffi.so")
set(RUST_LIB_SRC "${PROJECT_ROOT}/target/${TARGET_DIR}/${RUST_LIB_FILENAME}")

# Ninja needs to know which file is produced; otherwise it will error before running Cargo.
add_custom_command(
  OUTPUT "${RUST_LIB_SRC}"
  COMMAND cargo build -p stellatune-ffi ${CARGO_ARGS}
  WORKING_DIRECTORY "${PROJECT_ROOT}"
  COMMENT "Building Rust library (${TARGET_DIR})..."
  VERBATIM
)

add_custom_target(stellatune_rust_build ALL
  DEPENDS "${RUST_LIB_SRC}"
)

# Define the application target. To change its name, change BINARY_NAME in the
# top-level CMakeLists.txt, not the value here, or `flutter run` will no longer
# work.
#
# Any new source files that you add to the application should be added here.
add_executable(${BINARY_NAME}
  "main.cc"
  "my_application.cc"
  "${FLUTTER_MANAGED_DIR}/generated_plugin_registrant.cc"
)

# Ensure Cargo runs before linking C++.
add_dependencies(${BINARY_NAME} stellatune_rust_build)

# Define an imported target for the Rust library and link it.
add_library(stellatune_ffi_rust SHARED IMPORTED GLOBAL)
set_target_properties(stellatune_ffi_rust PROPERTIES
  IMPORTED_LOCATION "${RUST_LIB_SRC}"
)
target_link_libraries(${BINARY_NAME} PRIVATE stellatune_ffi_rust)

# Apply the standard set of build settings. This can be removed for applications
# that need different build settings.
apply_standard_settings(${BINARY_NAME})

# Add preprocessor definitions for the application ID.
add_definitions(-DAPPLICATION_ID="${APPLICATION_ID}")

# Add dependency libraries. Add any application-specific dependencies here.
target_link_libraries(${BINARY_NAME} PRIVATE flutter)
target_link_libraries(${BINARY_NAME} PRIVATE PkgConfig::GTK)

target_include_directories(${BINARY_NAME} PRIVATE "${CMAKE_SOURCE_DIR}")

# PREVENT LINKER OPTIMIZATION
# This is crucial for FFI. It forces the linker to include the library.
target_link_options(${BINARY_NAME} PRIVATE "-Wl,--no-as-needed")

# Runtime path (RPATH): look for the .so in the 'lib' folder next to the binary.
set_target_properties(${BINARY_NAME} PROPERTIES
  BUILD_WITH_INSTALL_RPATH TRUE
  INSTALL_RPATH "$ORIGIN/lib"
  BUILD_RPATH   "$ORIGIN/lib"
)

# Copy the .so file to bundle/lib so the app can find it when running locally.
add_custom_command(TARGET ${BINARY_NAME} POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E make_directory "$<TARGET_FILE_DIR:${BINARY_NAME}>/lib"
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
          "${RUST_LIB_SRC}"
          "$<TARGET_FILE_DIR:${BINARY_NAME}>/lib/${RUST_LIB_FILENAME}"
  COMMENT "Copying ${RUST_LIB_FILENAME} to bundle/lib..."
)

# Ensure the .so is included in the final bundle for `flutter build linux`.
install(FILES "${RUST_LIB_SRC}"
  DESTINATION "lib"
  COMPONENT Runtime
)
