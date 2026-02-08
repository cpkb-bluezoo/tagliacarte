cmake_minimum_required(VERSION 3.16)
project(tagliacarte_ui LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)

# Qt 6 (Widgets). Set CMAKE_PREFIX_PATH to Qt install if needed.
find_package(Qt6 REQUIRED COMPONENTS Core Widgets Svg)

# Tagliacarte FFI: Rust cdylib built with cargo build -p tagliacarte_ffi
# Default: ../target/release (or Debug when building Debug)
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  set(TAGLIACARTE_FFI_DIR "${CMAKE_SOURCE_DIR}/../target/debug")
else()
  set(TAGLIACARTE_FFI_DIR "${CMAKE_SOURCE_DIR}/../target/release")
endif()
# Allow override: cmake -DTAGLIACARTE_FFI_DIR=/path/to/dir
if(DEFINED ENV{TAGLIACARTE_FFI_DIR})
  set(TAGLIACARTE_FFI_DIR "$ENV{TAGLIACARTE_FFI_DIR}")
endif()

# App icon (macOS): run 'make icons' to generate icons/icon.icns from icons/app-icon.svg
set(APP_ICON_SOURCES main.cpp EventBridge.cpp)
if(APPLE AND EXISTS "${CMAKE_SOURCE_DIR}/../icons/icon.icns")
  set(APP_ICON_FILE "${CMAKE_SOURCE_DIR}/../icons/icon.icns")
  set_source_files_properties(${APP_ICON_FILE} PROPERTIES MACOSX_PACKAGE_LOCATION "Resources")
  list(APPEND APP_ICON_SOURCES ${APP_ICON_FILE})
endif()

# Executable
qt_add_executable(tagliacarte_ui ${APP_ICON_SOURCES})

# UI icons (toolbar, sidebar) from ui/icons/ — use :/icons/cog.svg, :/icons/quill.svg
qt_add_resources(tagliacarte_ui "icons" PREFIX "/"
  FILES icons.qrc
)

# EventBridge uses Q_OBJECT; generate moc and link it
set(MOC_EVENTBRIDGE_OUT ${CMAKE_CURRENT_BINARY_DIR}/moc_EventBridge.cpp)
qt_generate_moc(${CMAKE_SOURCE_DIR}/EventBridge.h ${MOC_EVENTBRIDGE_OUT} TARGET tagliacarte_ui)
target_sources(tagliacarte_ui PRIVATE ${MOC_EVENTBRIDGE_OUT})
if(APPLE)
  set_target_properties(tagliacarte_ui PROPERTIES MACOSX_BUNDLE TRUE)
  if(EXISTS "${CMAKE_SOURCE_DIR}/../icons/icon.icns")
    set_target_properties(tagliacarte_ui PROPERTIES MACOSX_BUNDLE_ICON_FILE icon)
  endif()
endif()

target_include_directories(tagliacarte_ui PRIVATE
  ${CMAKE_SOURCE_DIR}
  ${CMAKE_SOURCE_DIR}/../ffi/include
)
target_link_directories(tagliacarte_ui PRIVATE ${TAGLIACARTE_FFI_DIR})
target_link_libraries(tagliacarte_ui PRIVATE
  Qt6::Widgets
  Qt6::Svg
  tagliacarte_ffi
)

# Copy the Rust dylib next to the executable so it can load at runtime (e.g. macOS)
# Alternatively use RPATH. For macOS .app bundle we'd copy into .app/Contents/libs.
if(APPLE)
  add_custom_command(TARGET tagliacarte_ui POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
      "${TAGLIACARTE_FFI_DIR}/libtagliacarte_ffi.dylib"
      $<TARGET_FILE_DIR:tagliacarte_ui>
    COMMENT "Copy libtagliacarte_ffi.dylib next to executable"
  )
endif()
if(UNIX AND NOT APPLE)
  add_custom_command(TARGET tagliacarte_ui POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
      "${TAGLIACARTE_FFI_DIR}/libtagliacarte_ffi.so"
      $<TARGET_FILE_DIR:tagliacarte_ui>
    COMMENT "Copy libtagliacarte_ffi.so next to executable"
  )
endif()
