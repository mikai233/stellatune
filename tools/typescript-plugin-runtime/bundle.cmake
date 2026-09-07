# Included after the Flutter bundle install directory has been defined.
get_filename_component(runtime_tools "${CMAKE_CURRENT_LIST_FILE}" DIRECTORY)
if(WIN32)
  set(node_os win)
  set(node_arch "${CMAKE_GENERATOR_PLATFORM}")
else()
  set(node_os linux)
  set(node_arch "${CMAKE_SYSTEM_PROCESSOR}")
endif()
string(TOLOWER "${node_arch}" node_arch)
if(node_arch MATCHES "^(arm64|aarch64)$")
  set(node_arch arm64)
elseif(node_arch MATCHES "^(x64|x86_64|amd64)$")
  set(node_arch x64)
else()
  message(FATAL_ERROR "Unsupported plugin runtime architecture: ${node_arch}")
endif()
set(node_stage "${CMAKE_BINARY_DIR}/plugin-runtime")
execute_process(COMMAND "${CMAKE_COMMAND}"
  "-DNODE_TARGET=${node_os}-${node_arch}"
  "-DNODE_CACHE=${runtime_tools}/../../target/node-runtime-cache"
  "-DNODE_DEST=${node_stage}"
  -P "${runtime_tools}/prepare.cmake"
  RESULT_VARIABLE prepare_result)
if(NOT prepare_result EQUAL 0)
  message(FATAL_ERROR "Failed to prepare bundled plugin runtime")
endif()
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
  "${runtime_tools}/prepare.cmake" "${runtime_tools}/runner.mjs"
  "${runtime_tools}/host-client.mjs" "${runtime_tools}/ui-server.mjs")
install(DIRECTORY "${node_stage}/" DESTINATION "${CMAKE_INSTALL_PREFIX}/plugin-runtime"
  USE_SOURCE_PERMISSIONS COMPONENT Runtime)
