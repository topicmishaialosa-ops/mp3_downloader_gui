#include <QApplication>

#include "mainwindow.h"

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("mp3_downloader_gui_qt"));
    QApplication::setOrganizationName(QStringLiteral("MP3Party"));

    MainWindow window;
    window.show();
    return app.exec();
}
