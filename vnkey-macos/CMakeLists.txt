cmake_minimum_required(VERSION 3.20)
project(vnkey-macos LANGUAGES C OBJC)

set(CMAKE_C_STANDARD 11)
set(CMAKE_OBJC_STANDARD 11)

# ==================== vnkey-engine (Rust static lib) ====================
set(ENGINE_DIR "${CMAKE_SOURCE_DIR}/../vnkey-engine")
set(ENGINE_LIB "${ENGINE_DIR}/target/release/libvnkey_engine.a")

add_custom_command(
    OUTPUT "${ENGINE_LIB}"
    COMMAND cargo build --release
    WORKING_DIRECTORY "${ENGINE_DIR}"
    COMMENT "Building vnkey-engine (Rust)"
)
add_custom_target(vnkey_engine_build DEPENDS "${ENGINE_LIB}")

add_library(vnkey_engine STATIC IMPORTED)
set_target_properties(vnkey_engine PROPERTIES IMPORTED_LOCATION "${ENGINE_LIB}")
add_dependencies(vnkey_engine vnkey_engine_build)

# ==================== VnKey.app (Input Method) ====================
set(APP_NAME "VnKey")
set(BUNDLE_ID "dev.marix.inputmethod.VnKey")

add_executable(${APP_NAME} MACOSX_BUNDLE
    src/main.m
    src/VnKeyInputController.m
    src/VnKeyAppDelegate.m
    src/VnKeyPreferences.m
)

target_include_directories(${APP_NAME} PRIVATE
    src
    "${ENGINE_DIR}/target/release"
)

# Frameworks
find_library(COCOA_FW Cocoa REQUIRED)
find_library(IMKIT_FW InputMethodKit REQUIRED)
find_library(CARBON_FW Carbon REQUIRED)

target_link_libraries(${APP_NAME} PRIVATE
    vnkey_engine
    ${COCOA_FW}
    ${IMKIT_FW}
    ${CARBON_FW}
    "-framework Security"
    "-framework SystemConfiguration"
    resolv
)

# Info.plist
set_target_properties(${APP_NAME} PROPERTIES
    MACOSX_BUNDLE TRUE
    MACOSX_BUNDLE_INFO_PLIST "${CMAKE_SOURCE_DIR}/resources/Info.plist"
    MACOSX_BUNDLE_BUNDLE_NAME "${APP_NAME}"
    MACOSX_BUNDLE_GUI_IDENTIFIER "${BUNDLE_ID}"
    MACOSX_BUNDLE_BUNDLE_VERSION "1.0.1"
    MACOSX_BUNDLE_SHORT_VERSION_STRING "1.0.1"
    XCODE_ATTRIBUTE_CODE_SIGN_IDENTITY "-"
)

# Copy icon vào bundle
set(ICON_FILE "${CMAKE_SOURCE_DIR}/resources/VnKey.icns")
if(EXISTS "${ICON_FILE}")
    target_sources(${APP_NAME} PRIVATE "${ICON_FILE}")
    set_source_files_properties("${ICON_FILE}" PROPERTIES
        MACOSX_PACKAGE_LOCATION "Resources"
    )
endif()

# Install: copy vào ~/Library/Input Methods/
install(TARGETS ${APP_NAME}
    BUNDLE DESTINATION "$ENV{HOME}/Library/Input Methods"
)
