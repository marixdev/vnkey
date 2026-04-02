cmake_minimum_required(VERSION 3.10)
project(vnkey-ibus VERSION 1.0.3 LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

include(GNUInstallDirs)

# ==================== Find IBus ====================
find_package(PkgConfig REQUIRED)
pkg_check_modules(IBUS REQUIRED ibus-1.0>=1.5)
pkg_check_modules(GLIB REQUIRED glib-2.0)

# ==================== vnkey-engine (Rust static library) ====================
# Build first: cd ../vnkey-engine && cargo build --release
set(VNKEY_ENGINE_LIB_DIR "${CMAKE_SOURCE_DIR}/../vnkey-engine/target/release"
    CACHE PATH "Path to vnkey-engine library directory")

add_library(vnkey_engine STATIC IMPORTED)
set_target_properties(vnkey_engine PROPERTIES
    IMPORTED_LOCATION "${VNKEY_ENGINE_LIB_DIR}/libvnkey_engine.a"
)

# ==================== IBus engine executable ====================
add_executable(ibus-engine-vnkey src/vnkey-ibus.c src/glibc-compat.c)
target_include_directories(ibus-engine-vnkey PRIVATE
    src
    ${IBUS_INCLUDE_DIRS}
    ${GLIB_INCLUDE_DIRS}
)
target_link_libraries(ibus-engine-vnkey
    ${IBUS_LIBRARIES}
    ${GLIB_LIBRARIES}
    vnkey_engine
    pthread dl m
)
target_compile_options(ibus-engine-vnkey PRIVATE
    ${IBUS_CFLAGS_OTHER}
    ${GLIB_CFLAGS_OTHER}
)

# Static link GCC runtime for portability
target_link_options(ibus-engine-vnkey PRIVATE -static-libgcc)

# ==================== Install rules ====================

# Engine install dir — always use absolute path under install prefix
# (pkg-config --variable=libexecdir ibus-1.0 returns empty on Debian 11)
set(IBUS_LIBEXECDIR "${CMAKE_INSTALL_PREFIX}/libexec"
    CACHE PATH "Where to install ibus-engine-vnkey")

# IBus component XML dir — /usr/share/ibus/component
set(IBUS_COMPONENT_DIR "${CMAKE_INSTALL_PREFIX}/share/ibus/component"
    CACHE PATH "IBus component XML directory")

# Engine executable
install(TARGETS ibus-engine-vnkey
    DESTINATION "${IBUS_LIBEXECDIR}")

# Component XML — configure with actual install path
configure_file(
    "${CMAKE_SOURCE_DIR}/data/vnkey.xml.in"
    "${CMAKE_BINARY_DIR}/vnkey.xml"
    @ONLY
)
install(FILES "${CMAKE_BINARY_DIR}/vnkey.xml"
    DESTINATION "${IBUS_COMPONENT_DIR}")

# Icon (scalable SVG)
install(FILES data/vnkey.svg
    DESTINATION "${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps"
    RENAME ibus-vnkey.svg)

# ==================== CPack — .deb & .rpm ====================

set(CPACK_PACKAGE_NAME "vnkey-ibus")
set(CPACK_PACKAGE_VERSION_MAJOR ${PROJECT_VERSION_MAJOR})
set(CPACK_PACKAGE_VERSION_MINOR ${PROJECT_VERSION_MINOR})
set(CPACK_PACKAGE_VERSION_PATCH ${PROJECT_VERSION_PATCH})
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY
    "VnKey Vietnamese Input Method for IBus")
set(CPACK_PACKAGE_DESCRIPTION
    "Vietnamese input method engine (Telex, VNI, VIQR) for IBus framework, powered by vnkey-engine (Rust). Supports all major Vietnamese charsets.")
set(CPACK_PACKAGE_CONTACT "hi@vnkey.app")
set(CPACK_PACKAGE_VENDOR "VnKey Project")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_SOURCE_DIR}/LICENSE")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/marixdev/vnkey")
set(CPACK_STRIP_FILES ON)

# ---------- DEB ----------
set(CPACK_DEBIAN_PACKAGE_MAINTAINER "VnKey Developers <hi@vnkey.app>")
set(CPACK_DEBIAN_PACKAGE_SECTION "utils")
set(CPACK_DEBIAN_PACKAGE_PRIORITY "optional")
set(CPACK_DEBIAN_PACKAGE_DEPENDS "ibus (>= 1.5)")
set(CPACK_DEBIAN_PACKAGE_RECOMMENDS
    "ibus-gtk, ibus-gtk3, im-config | im-chooser")
set(CPACK_DEBIAN_PACKAGE_CONTROL_EXTRA
    "${CMAKE_SOURCE_DIR}/scripts/postinst;${CMAKE_SOURCE_DIR}/scripts/prerm")
set(CPACK_DEBIAN_FILE_NAME DEB-DEFAULT)

# ---------- RPM ----------
set(CPACK_RPM_PACKAGE_LICENSE "GPL-3.0-or-later")
set(CPACK_RPM_PACKAGE_GROUP "System Environment/Libraries")
set(CPACK_RPM_PACKAGE_REQUIRES "ibus >= 1.5")
set(CPACK_RPM_PACKAGE_RECOMMENDS "ibus-gtk2, ibus-gtk3")
set(CPACK_RPM_POST_INSTALL_SCRIPT_FILE
    "${CMAKE_SOURCE_DIR}/scripts/postinst")
set(CPACK_RPM_PRE_UNINSTALL_SCRIPT_FILE
    "${CMAKE_SOURCE_DIR}/scripts/prerm")
set(CPACK_RPM_FILE_NAME RPM-DEFAULT)
set(CPACK_RPM_PACKAGE_AUTOREQ OFF)
set(CPACK_RPM_PACKAGE_AUTOPROV OFF)

# Enable both generators
set(CPACK_GENERATOR "DEB;RPM")

include(CPack)
