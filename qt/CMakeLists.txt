cmake_minimum_required(VERSION 3.16)
project(mp3_downloader_gui_qt VERSION 1.18.3 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

find_package(Qt6 REQUIRED COMPONENTS Core Widgets Network Concurrent Multimedia MultimediaWidgets)

qt_add_executable(mp3_downloader_gui_qt
    src/main.cpp
    src/mainwindow.cpp
    src/mainwindow.h
    src/track.h
    src/downloadsource.h
    src/appconstants.h
    src/paths.h
    src/httpclient.cpp
    src/httpclient.h
    src/mp3partyapi.cpp
    src/mp3partyapi.h
    src/drivemusicapi.cpp
    src/drivemusicapi.h
    src/pesnimeapi.cpp
    src/pesnimeapi.h
    src/ytdlphelper.cpp
    src/ytdlphelper.h
    src/mpvhelper.cpp
    src/mpvhelper.h
    src/downloadmanager.cpp
    src/downloadmanager.h
    src/playercontroller.cpp
    src/playercontroller.h
    src/libraryscanner.cpp
    src/libraryscanner.h
    src/batchqueries.cpp
    src/batchqueries.h
)

target_link_libraries(mp3_downloader_gui_qt PRIVATE
    Qt6::Core
    Qt6::Widgets
    Qt6::Network
    Qt6::Concurrent
    Qt6::Multimedia
    Qt6::MultimediaWidgets
)

if(WIN32)
    set_target_properties(mp3_downloader_gui_qt PROPERTIES WIN32_EXECUTABLE TRUE)
endif()

if(APPLE)
    set_target_properties(mp3_downloader_gui_qt PROPERTIES
        MACOSX_BUNDLE TRUE
        MACOSX_BUNDLE_GUI_IDENTIFIER net.mp3party.downloader.qt
        MACOSX_BUNDLE_BUNDLE_NAME "MP3 Downloader Qt"
        MACOSX_BUNDLE_BUNDLE_VERSION "${PROJECT_VERSION}"
        MACOSX_BUNDLE_SHORT_VERSION_STRING "${PROJECT_VERSION}"
    )
endif()

if(APPLE)
    install(TARGETS mp3_downloader_gui_qt
        BUNDLE DESTINATION .
        RUNTIME DESTINATION bin)
else()
    install(TARGETS mp3_downloader_gui_qt RUNTIME DESTINATION bin)
endif()
