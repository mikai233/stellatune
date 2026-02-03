cmake_minimum_required(VERSION 3.14)
project(runner LANGUAGES CXX)

# Define the application target. To change its name, change BINARY_NAME in the
# top-level CMakeLists.txt, not the value here, or `flutter run` will no longer
# work.
#
# Any new source files that you add to the application should be added here.
add_executable(${BINARY_NAME} WIN32
  "flutter_window.cpp"
  "main.cpp"
  "utils.cpp"
  "win32_window.cpp"
  "${FLUTTER_MANAGED_DIR}/generated_plugin_registrant.cc"
  "Runner.rc"
  "runner.exe.manifest"
)

# Apply the standard set of build settings. This can be removed for applications
# that need different build settings.
apply_standard_settings(${BINARY_NAME})

# Add preprocessor definitions for the build version.
target_compile_definitions(${BINARY_NAME} PRIVATE "FLUTTER_VERSION=\"${FLUTTER_VERSION}\"")
target_compile_definitions(${BINARY_NAME} PRIVATE "FLUTTER_VERSION_MAJOR=${FLUTTER_VERSION_MAJOR}")
target_compile_definitions(${BINARY_NAME} PRIVATE "FLUTTER_VERSION_MINOR=${FLUTTER_VERSION_MINOR}")
target_compile_definitions(${BINARY_NAME} PRIVATE "FLUTTER_VERSION_PATCH=${FLUTTER_VERSION_PATCH}")
target_compile_definitions(${BINARY_NAME} PRIVATE "FLUTTER_VERSION_BUILD=${FLUTTER_VERSION_BUILD}")

# Disable Windows macros that collide with C++ standard library functions.
target_compile_definitions(${BINARY_NAME} PRIVATE "NOMINMAX")

# Add dependency libraries and include directories. Add any application-specific
# dependencies here.
target_link_libraries(${BINARY_NAME} PRIVATE flutter flutter_wrapper_app)
target_link_libraries(${BINARY_NAME} PRIVATE "dwmapi.lib")
target_include_directories(${BINARY_NAME} PRIVATE "${CMAKE_SOURCE_DIR}")

# Run the Flutter tool portions of the build. This must not be removed.
add_dependencies(${BINARY_NAME} flutter_assemble)


# =========================================================
# Rust Integration: Build stellatune_ffi.dll automatically
# =========================================================

# Assumes this file is at: <repo>/apps/stellatune/windows/runner/CMakeLists.txt
get_filename_component(STELLATUNE_WORKSPACE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../.." ABSOLUTE)

# Cargo package name (kebab-case) and output DLL stem (snake_case).
set(RUST_PACKAGE_NAME "stellatune-ffi")
set(RUST_LIB_NAME "stellatune_ffi")

# Detect target architecture for Cargo.
if(CMAKE_VS_PLATFORM_NAME STREQUAL "ARM64")
  set(RUST_TARGET "aarch64-pc-windows-msvc")
else()
  set(RUST_TARGET "x86_64-pc-windows-msvc")
endif()

# Per-config staging directory to avoid Debug/Release collisions.
# Note: Visual Studio link steps don't reliably expand generator expressions for
# imported target properties (e.g. IMPORTED_IMPLIB). We therefore stage into
# per-config folders but set IMPORTED_*_<CONFIG> properties explicitly below.
set(RUST_STAGING_ROOT "${CMAKE_CURRENT_BINARY_DIR}/rust")
set(RUST_STAGING_DIR "${RUST_STAGING_ROOT}/$<CONFIG>")
set(RUST_DLL_STAGING "${RUST_STAGING_DIR}/${RUST_LIB_NAME}.dll")
set(RUST_IMPLIB_STAGING "${RUST_STAGING_DIR}/${RUST_LIB_NAME}.dll.lib")

set(RUST_DLL_DEBUG "${RUST_STAGING_ROOT}/Debug/${RUST_LIB_NAME}.dll")
set(RUST_IMPLIB_DEBUG "${RUST_STAGING_ROOT}/Debug/${RUST_LIB_NAME}.dll.lib")
set(RUST_DLL_RELEASE "${RUST_STAGING_ROOT}/Release/${RUST_LIB_NAME}.dll")
set(RUST_IMPLIB_RELEASE "${RUST_STAGING_ROOT}/Release/${RUST_LIB_NAME}.dll.lib")
set(RUST_DLL_PROFILE "${RUST_STAGING_ROOT}/Profile/${RUST_LIB_NAME}.dll")
set(RUST_IMPLIB_PROFILE "${RUST_STAGING_ROOT}/Profile/${RUST_LIB_NAME}.dll.lib")
set(BUILD_SCRIPT "${CMAKE_CURRENT_BINARY_DIR}/cargo_build_runner.bat")

file(WRITE "${BUILD_SCRIPT}" "
@echo off
setlocal enabledelayedexpansion

:: Arguments passed from CMake
set PACKAGE_NAME=%1
set TARGET_ARCH=%2
set BUILD_MODE=%3
set OUTPUT_DLL_DEST=%4
set OUTPUT_IMPLIB_DEST=%5

:: --- Environment Setup ---
:: Clear potential conflicts from VS environment variables to ensure a clean Rust build.
set LIB=
set LIBPATH=
set INCLUDE=
set VSINSTALLDIR=
set VCINSTALLDIR=
set PATH=C:\\Windows\\system32;C:\\Windows;C:\\Windows\\System32\\Wbem;C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\;%USERPROFILE%\\.cargo\\bin

:: --- Determine Cargo Profile ---
if \"%BUILD_MODE%\"==\"release\" (
    echo [Rust] Build Profile: RELEASE
    set CARGO_ARGS=--release
    set SUBDIR=release
) else (
    echo [Rust] Build Profile: DEBUG
    set CARGO_ARGS=
    set SUBDIR=debug
)

:: --- Execute Cargo Build ---
echo [Rust] Running: cargo build -p %PACKAGE_NAME% --target %TARGET_ARCH% %CARGO_ARGS%
cargo build -p %PACKAGE_NAME% --target %TARGET_ARCH% %CARGO_ARGS%

if %errorlevel% neq 0 (
    echo [Rust] Fatal Error: Cargo build failed with code %errorlevel%
    exit /b %errorlevel%
)

:: --- Retrieve Artifact ---
set SRC_DLL=target\\%TARGET_ARCH%\\!SUBDIR!\\${RUST_LIB_NAME}.dll
set SRC_IMPLIB=target\\%TARGET_ARCH%\\!SUBDIR!\\${RUST_LIB_NAME}.dll.lib

if not exist \"!SRC_DLL!\" (
    echo [Rust] Fatal Error: DLL artifact not found at: !SRC_DLL!
    exit /b 1
)

echo [Rust] Copying artifact to staging area...
copy /Y \"!SRC_DLL!\" \"%OUTPUT_DLL_DEST%\" >nul
if %errorlevel% neq 0 (
    echo [Rust] Fatal Error: Failed to copy DLL.
    exit /b %errorlevel%
)

if not exist \"!SRC_IMPLIB!\" (
    echo [Rust] Fatal Error: Import library not found at: !SRC_IMPLIB!
    exit /b 1
)

echo [Rust] Copying import library to staging area...
copy /Y \"!SRC_IMPLIB!\" \"%OUTPUT_IMPLIB_DEST%\" >nul
if %errorlevel% neq 0 (
    echo [Rust] Fatal Error: Failed to copy import library.
    exit /b %errorlevel%
)

echo [Rust] Build sequence completed successfully.
")

set(MODE_GENEX "$<IF:$<OR:$<CONFIG:Release>,$<CONFIG:Profile>>,release,debug>")

add_custom_target(stellatune_rust_build ALL
  COMMAND ${CMAKE_COMMAND} -E make_directory "${RUST_STAGING_DIR}"
  COMMAND "${BUILD_SCRIPT}"
          "${RUST_PACKAGE_NAME}"
          "${RUST_TARGET}"
          "${MODE_GENEX}"
          "${RUST_DLL_STAGING}"
          "${RUST_IMPLIB_STAGING}"
  WORKING_DIRECTORY "${STELLATUNE_WORKSPACE_DIR}"
  COMMENT "Compiling Rust backend (Mode: ${MODE_GENEX})..."
  VERBATIM
)

add_dependencies(${BINARY_NAME} stellatune_rust_build)

# Link to the import library so the DLL is loaded into the current process.
add_library(stellatune_ffi_rust SHARED IMPORTED GLOBAL)
set_target_properties(stellatune_ffi_rust PROPERTIES
  IMPORTED_CONFIGURATIONS "Debug;Release;Profile"
  IMPORTED_LOCATION_DEBUG "${RUST_DLL_DEBUG}"
  IMPORTED_IMPLIB_DEBUG "${RUST_IMPLIB_DEBUG}"
  IMPORTED_LOCATION_RELEASE "${RUST_DLL_RELEASE}"
  IMPORTED_IMPLIB_RELEASE "${RUST_IMPLIB_RELEASE}"
  IMPORTED_LOCATION_PROFILE "${RUST_DLL_PROFILE}"
  IMPORTED_IMPLIB_PROFILE "${RUST_IMPLIB_PROFILE}"
)
target_link_libraries(${BINARY_NAME} PRIVATE stellatune_ffi_rust)

# Prevent the linker from dropping the import library even if no Rust symbols
# are referenced from C++ (we load via `ExternalLibrary.process()` in Dart).
if(MSVC)
  target_link_options(${BINARY_NAME} PRIVATE "/INCLUDE:frb_get_rust_content_hash")
endif()

# Copy the staged DLL to the final executable directory alongside the Flutter Runner.
add_custom_command(TARGET ${BINARY_NAME} POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
          "${RUST_DLL_STAGING}"
          "$<TARGET_FILE_DIR:${BINARY_NAME}>/${RUST_LIB_NAME}.dll"
  VERBATIM
)
