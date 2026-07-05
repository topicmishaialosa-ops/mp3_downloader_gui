#include <QApplication>
#include <QMessageBox>

#include "mainwindow.h"

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("mp3_downloader_gui_qt"));
    QApplication::setOrganizationName(QStringLiteral("MP3Party"));

    MainWindow window;
    window.show();

    for (int i = 1; i < argc; ++i) {
        const QString arg = QString::fromLocal8Bit(argv[i]);
        if (arg.endsWith(QLatin1String(".impe"))) {
            Track t = MainWindow::parseImpeFile(arg);
            if (!t.id.isEmpty()) {
                window.showImpeDialog(t);
            }
            break;
        }
    }

    return app.exec();
}
