#include "mpvhelper.h"

#include "httpclient.h"

#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QUrl>
#include <QtGlobal>

namespace {

QString shinchiroAssetUrl(const QString &namePrefix) {
    const auto resp = HttpClient::get(
        QUrl(QStringLiteral("https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/"
                            "latest")),
        {{QStringLiteral("Accept"), QStringLiteral("application/vnd.github+json")}},
        90000);
    if (!resp.ok()) {
        return {};
    }
    const QJsonDocument doc = QJsonDocument::fromJson(resp.body);
    const QJsonArray assets = doc.object().value(QStringLiteral("assets")).toArray();
    QString fallback;
    for (const QJsonValue &v : assets) {
        const QJsonObject o = v.toObject();
        const QString name = o.value(QStringLiteral("name")).toString();
        if (!name.startsWith(namePrefix) || !name.endsWith(QStringLiteral(".7z"))) {
            continue;
        }
        const QString url = o.value(QStringLiteral("browser_download_url")).toString();
        if (name.contains(QStringLiteral("v3"))) {
            return url;
        }
        if (fallback.isEmpty()) {
            fallback = url;
        }
    }
    return fallback;
}

bool extract7z(const QString &archive, const QString &destDir, QString *error) {
    QString sevenZ;
    for (const QString &cmd : {QStringLiteral("7z"), QStringLiteral("7za")}) {
        const QString p = QStandardPaths::findExecutable(cmd);
        if (!p.isEmpty()) {
            sevenZ = p;
            break;
        }
    }
#if defined(Q_OS_WIN)
    if (sevenZ.isEmpty()) {
        const QString p = QStringLiteral("C:/Program Files/7-Zip/7z.exe");
        if (QFileInfo::exists(p)) {
            sevenZ = p;
        }
    }
#endif
    if (sevenZ.isEmpty()) {
        if (error) {
            *error = QStringLiteral(
                "Нужен 7-Zip (7z) для распаковки mpv. Установите 7-Zip или mpv в PATH.");
        }
        return false;
    }
    QDir().mkpath(destDir);
    QProcess proc;
    proc.setProgram(sevenZ);
    proc.setArguments({QStringLiteral("x"),
                       archive,
                       QStringLiteral("-o%1").arg(destDir),
                       QStringLiteral("-y")});
    proc.start();
    if (!proc.waitForFinished(300000) || proc.exitCode() != 0) {
        if (error) {
            *error = QStringLiteral("Распаковка mpv не удалась (7z код %1)")
                         .arg(proc.exitCode());
        }
        return false;
    }
    return true;
}

QString findMpvInTree(const QString &root) {
#if defined(Q_OS_WIN)
    const QString name = QStringLiteral("mpv.exe");
#else
    const QString name = QStringLiteral("mpv");
#endif
    QDirIterator it(root, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        const QFileInfo fi(it.next());
        if (fi.isFile() && fi.fileName() == name) {
            return fi.absoluteFilePath();
        }
    }
    return {};
}

bool copyDirRecursive(const QString &src, const QString &dst) {
    QDir srcDir(src);
    if (!srcDir.exists()) {
        return false;
    }
    QDir().mkpath(dst);
    for (const QString &entry : srcDir.entryList(QDir::Files | QDir::Dirs | QDir::NoDotAndDotDot)) {
        const QString srcPath = srcDir.absoluteFilePath(entry);
        const QString dstPath = QDir(dst).filePath(entry);
        if (QFileInfo(srcPath).isDir()) {
            if (!copyDirRecursive(srcPath, dstPath)) {
                return false;
            }
        } else {
            if (QFile::exists(dstPath)) {
                QFile::remove(dstPath);
            }
            if (!QFile::copy(srcPath, dstPath)) {
                return false;
            }
        }
    }
    return true;
}

} // namespace

QString MpvHelper::installDir() {
#if defined(Q_OS_WIN)
    return QDir(QDir::homePath()).filePath(QStringLiteral("mpv-util/windows"));
#elif defined(Q_OS_MACOS)
    return QDir(QDir::homePath()).filePath(QStringLiteral("mpv-util/macos"));
#else
    return QDir(QDir::homePath()).filePath(QStringLiteral("mpv-util/bin"));
#endif
}

QString MpvHelper::resolveBinary(QString *error) {
    QStringList candidates;
#if defined(Q_OS_WIN)
    candidates << QDir(installDir()).filePath(QStringLiteral("mpv.exe"));
#else
    candidates << QDir(installDir()).filePath(QStringLiteral("mpv"));
#endif
    candidates << QStandardPaths::findExecutable(QStringLiteral("mpv"));
    candidates << QStandardPaths::findExecutable(QStringLiteral("mpv.exe"));
    for (const QString &p : candidates) {
        if (!p.isEmpty() && QFileInfo::exists(p)) {
            return p;
        }
    }
    if (error) {
#if defined(Q_OS_LINUX)
        *error = QStringLiteral(
            "mpv не найден. Установите: sudo pacman -S mpv  или  sudo apt install mpv");
#else
        *error = QStringLiteral(
            "mpv не найден. Установите в PATH или скачайте через приложение в %1")
                     .arg(installDir());
#endif
    }
    return {};
}

bool MpvHelper::isAvailable() {
    return !resolveBinary(nullptr).isEmpty();
}

bool MpvHelper::install(QString *error) {
#if defined(Q_OS_LINUX)
    if (error) {
        *error = QStringLiteral(
            "На Linux установите mpv через пакетный менеджер:\n"
            "  sudo pacman -S mpv\n"
            "  sudo apt install mpv\n"
            "Подробнее: https://mpv.io/installation/");
    }
    return false;
#else
    QString url;
#if defined(Q_OS_WIN)
    url = shinchiroAssetUrl(QStringLiteral("mpv-x86_64"));
#elif defined(Q_OS_MACOS)
#if defined(Q_PROCESSOR_ARM) || defined(__aarch64__)
    url = shinchiroAssetUrl(QStringLiteral("mpv-aarch64"));
#else
    if (error) {
        *error = QStringLiteral(
            "Автоустановка mpv для Intel Mac недоступна. Установите: brew install mpv");
    }
    return false;
#endif
#endif

    if (url.isEmpty()) {
        if (error) {
            *error = QStringLiteral("Не удалось найти сборку mpv на GitHub");
        }
        return false;
    }

    QTemporaryDir tmp;
    if (!tmp.isValid()) {
        if (error) {
            *error = QStringLiteral("Временная папка недоступна");
        }
        return false;
    }

    const QString archive = QDir(tmp.path()).filePath(QStringLiteral("mpv.7z"));
    const auto dl = HttpClient::downloadToFile(QUrl(url), archive, {});
    if (!dl.ok()) {
        if (error) {
            *error = dl.error.isEmpty() ? QStringLiteral("Скачивание mpv: HTTP %1").arg(dl.status)
                                        : dl.error;
        }
        return false;
    }

    const QString extractRoot = QDir(tmp.path()).filePath(QStringLiteral("extract"));
    if (!extract7z(archive, extractRoot, error)) {
        return false;
    }

    const QString found = findMpvInTree(extractRoot);
    if (found.isEmpty()) {
        if (error) {
            *error = QStringLiteral("mpv не найден в архиве");
        }
        return false;
    }

    const QString srcDir = QFileInfo(found).absolutePath();
    QDir(installDir()).removeRecursively();
    if (!copyDirRecursive(srcDir, installDir())) {
        if (error) {
            *error = QStringLiteral("Не удалось скопировать mpv в %1").arg(installDir());
        }
        return false;
    }

#if !defined(Q_OS_WIN)
    QFile f(resolveBinary());
    if (f.exists()) {
        f.setPermissions(QFile::ExeUser | QFile::ReadUser | QFile::WriteUser | QFile::ReadGroup
                         | QFile::ExeGroup | QFile::ReadOther | QFile::ExeOther);
    }
#endif

    return isAvailable();
#endif
}
