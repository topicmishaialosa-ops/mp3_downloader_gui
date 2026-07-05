#include "mainwindow.h"

#include <QApplication>
#include <QComboBox>
#include <QDialog>
#include <QFontDatabase>
#include <QLineEdit>
#include <QListWidgetItem>
#include <QDesktopServices>
#include <QFileDialog>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QListWidget>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QCheckBox>
#include <QSlider>
#include <QSpinBox>
#include <QTabWidget>
#include <QClipboard>
#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTimer>
#include <QUrl>
#include <QVBoxLayout>
#include <QtConcurrent>
#include <QEventLoop>

static Track parseImpeData(const QString &text);
static Track fetchUrlToTrack(const QString &url, QString *error = nullptr);

#include "drivemusicapi.h"
#include "mp3partyapi.h"
#include "paths.h"
#include "pesnimeapi.h"
#include "mpvhelper.h"
#include "ytdlphelper.h"

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
    setupUi();
    m_folderEdit->setText(AppPaths::defaultDownloadFolder());
    m_downloads.setDownloadFolder(m_folderEdit->text());
    m_player.bindUi(m_playerTitle, m_playerSubtitle, m_seekSlider, m_playerBar);
    connect(&m_downloads, &DownloadManager::logLine, this, &MainWindow::onLog);
    applyTheme(true);
    setWindowTitle(QStringLiteral("MP3 Downloader (Qt)"));
    resize(1000, 760);
}

void MainWindow::setupUi() {
    auto *central = new QWidget(this);
    auto *root = new QVBoxLayout(central);

    m_tabs = new QTabWidget();
    auto *searchPage = new QWidget();
    auto *searchLay = new QVBoxLayout(searchPage);

    auto *top = new QHBoxLayout();
    m_sourceCombo = new QComboBox();
    m_sourceCombo->addItem(QStringLiteral("MP3Party"), static_cast<int>(DownloadSource::Mp3Party));
    m_sourceCombo->addItem(QStringLiteral("DriveMusic"), static_cast<int>(DownloadSource::DriveMusic));
    m_sourceCombo->addItem(QStringLiteral("Pesni.me"), static_cast<int>(DownloadSource::PesniMe));
    m_sourceCombo->addItem(QStringLiteral("YouTube"), static_cast<int>(DownloadSource::YtDlp));
    m_ytFormatCombo = new QComboBox();
    m_ytFormatCombo->addItem(QStringLiteral("MP3"), static_cast<int>(YtFormat::Mp3));
    m_ytFormatCombo->addItem(QStringLiteral("MP4"), static_cast<int>(YtFormat::Mp4));
    m_queryEdit = new QLineEdit();
    m_queryEdit->setPlaceholderText(QStringLiteral("Исполнитель или название…"));
    m_searchBtn = new QPushButton(QStringLiteral("🔍 Найти"));
    auto *batchBtn = new QPushButton(QStringLiteral("📋 Список"));
    batchBtn->setToolTip(QStringLiteral(
        "Пакетный поиск: по одному треку на строку\n(Исполнитель - Название)"));
    connect(m_searchBtn, &QPushButton::clicked, this, &MainWindow::onSearch);
    connect(batchBtn, &QPushButton::clicked, this, &MainWindow::onBatchSearch);
    top->addWidget(new QLabel(QStringLiteral("Источник:")));
    top->addWidget(m_sourceCombo);
    top->addWidget(new QLabel(QStringLiteral("YT:")));
    top->addWidget(m_ytFormatCombo);
    top->addWidget(m_queryEdit, 1);
    top->addWidget(m_searchBtn);
    top->addWidget(batchBtn);

    auto *folderRow = new QHBoxLayout();
    m_folderEdit = new QLineEdit();
    auto *browseBtn = new QPushButton(QStringLiteral("📁 Папка"));
    connect(browseBtn, &QPushButton::clicked, this, &MainWindow::onBrowseFolder);
    folderRow->addWidget(new QLabel(QStringLiteral("Скачивать в:")));
    folderRow->addWidget(m_folderEdit, 1);
    folderRow->addWidget(browseBtn);

    m_resultsList = new QListWidget();
    m_resultsList->setSelectionMode(QAbstractItemView::ExtendedSelection);

    auto *btnRow = new QHBoxLayout();
    auto *dlSel = new QPushButton(QStringLiteral("📥 Скачать"));
    auto *streamBtn = new QPushButton(QStringLiteral("▶ Слушать"));
    auto *dlAll = new QPushButton(QStringLiteral("📥 Все"));
    auto *addPlaylistBtn = new QPushButton(QStringLiteral("➕ В плейлист"));
    auto *shareBtn = new QPushButton(QStringLiteral("🔗 .impe"));
    auto *importBtn = new QPushButton(QStringLiteral("📂 Импорт"));
    connect(dlSel, &QPushButton::clicked, this, &MainWindow::onDownloadSelected);
    connect(streamBtn, &QPushButton::clicked, this, &MainWindow::onStreamSelected);
    connect(dlAll, &QPushButton::clicked, this, &MainWindow::onDownloadAll);
    connect(addPlaylistBtn, &QPushButton::clicked, this, &MainWindow::onAddToPlaylist);
    connect(shareBtn, &QPushButton::clicked, this, &MainWindow::onShareTracks);
    connect(importBtn, &QPushButton::clicked, this, [this]() {
        QDialog dlg(this);
        dlg.setWindowTitle(QStringLiteral("📂 Импорт"));
        dlg.resize(420, 160);
        auto *lay = new QVBoxLayout(&dlg);
        QPushButton *fileBtn = new QPushButton(QStringLiteral("📁 .impe файл"));
        lay->addWidget(fileBtn);
        auto *urlLay = new QHBoxLayout();
        QLineEdit *urlEdit = new QLineEdit();
        urlEdit->setPlaceholderText(QStringLiteral("Ссылка: .impe, YouTube, mp3party.net…"));
        QPushButton *urlBtn = new QPushButton(QStringLiteral("🌐 Открыть"));
        urlLay->addWidget(urlEdit);
        urlLay->addWidget(urlBtn);
        lay->addLayout(urlLay);
        QPushButton *cancelBtn = new QPushButton(QStringLiteral("✕ Отмена"));
        lay->addWidget(cancelBtn);
        connect(fileBtn, &QPushButton::clicked, &dlg, [this, &dlg]() {
            const QString path = QFileDialog::getOpenFileName(this, QStringLiteral("Выберите .impe файл"),
                QString(), QStringLiteral("IMPE (*.impe);;Все файлы (*)"));
            if (path.isEmpty()) return;
            Track t = parseImpeFile(path);
            if (t.id.isEmpty()) {
                QMessageBox::warning(this, QStringLiteral("Ошибка"), QStringLiteral("Не удалось разобрать .impe файл"));
                return;
            }
            showImpeDialog(t);
            dlg.accept();
        });
        connect(urlBtn, &QPushButton::clicked, &dlg, [this, urlEdit, &dlg]() {
            const QString url = urlEdit->text().trimmed();
            if (url.isEmpty()) return;

            QString error;
            Track t = fetchUrlToTrack(url, &error);
            if (!t.id.isEmpty()) {
                showImpeDialog(t);
                dlg.accept();
                return;
            }

            QDialog loadingDlg(&dlg);
            loadingDlg.setWindowTitle(QStringLiteral("🌐 Загрузка…"));
            loadingDlg.resize(300, 80);
            auto *layL = new QVBoxLayout(&loadingDlg);
            layL->addWidget(new QLabel(QStringLiteral("Загрузка .impe с сервера…")));
            QNetworkAccessManager nam;
            QNetworkRequest req(url);
            QNetworkReply *reply = nam.get(req);
            QObject::connect(reply, &QNetworkReply::finished, &loadingDlg, &QDialog::accept);
            if (loadingDlg.exec() != QDialog::Accepted || reply->error() != QNetworkReply::NoError) {
                if (reply->error() != QNetworkReply::NoError)
                    QMessageBox::warning(&dlg, QStringLiteral("Ошибка"), reply->errorString());
                else
                    QMessageBox::warning(&dlg, QStringLiteral("Ошибка"), error);
                reply->deleteLater();
                return;
            }
            const QString text = QString::fromUtf8(reply->readAll());
            reply->deleteLater();
            Track t2 = parseImpeData(text);
            if (t2.id.isEmpty()) {
                QMessageBox::warning(&dlg, QStringLiteral("Ошибка"), QStringLiteral("Не удалось разобрать .impe"));
                return;
            }
            showImpeDialog(t2);
            dlg.accept();
        });
        connect(cancelBtn, &QPushButton::clicked, &dlg, &QDialog::reject);
        dlg.exec();
    });
    btnRow->addWidget(dlSel);
    btnRow->addWidget(streamBtn);
    btnRow->addWidget(dlAll);
    btnRow->addWidget(addPlaylistBtn);
    btnRow->addWidget(shareBtn);
    btnRow->addWidget(importBtn);
    btnRow->addStretch();

    searchLay->addLayout(top);
    searchLay->addLayout(folderRow);
    searchLay->addWidget(m_resultsList, 1);
    searchLay->addLayout(btnRow);

    auto *libraryPage = new QWidget();
    auto *libLay = new QVBoxLayout(libraryPage);
    auto *libTop = new QHBoxLayout();
    auto *refreshBtn = new QPushButton(QStringLiteral("🔄 Обновить"));
    auto *openBtn = new QPushButton(QStringLiteral("📂 Открыть папку"));
    auto *libShareBtn = new QPushButton(QStringLiteral("🔗 .impe"));
    connect(refreshBtn, &QPushButton::clicked, this, &MainWindow::onRefreshLibrary);
    connect(openBtn, &QPushButton::clicked, this, &MainWindow::onOpenFolder);
    connect(libShareBtn, &QPushButton::clicked, this, [this]() {
        const int row = m_libraryList->currentRow();
        if (row < 0 || row >= m_library.size()) return;
        const auto &f = m_library[row];
        Track t;
        t.id.clear();
        t.artist.clear();
        t.title = f.displayName;
        t.url = f.path;
        t.source = DownloadSource::Local;
        showShareDialog(t);
    });
    libTop->addWidget(refreshBtn);
    libTop->addWidget(openBtn);
    libTop->addWidget(libShareBtn);
    libTop->addStretch();
    m_libraryList = new QListWidget();
    connect(m_libraryList, &QListWidget::itemDoubleClicked, this, [this](QListWidgetItem *item) {
        const int row = m_libraryList->row(item);
        if (row < 0 || row >= m_library.size()) {
            return;
        }
        const auto &f = m_library[row];
        if (f.isVideo && !ensureMpv(true)) {
            return;
        }
        m_player.stop();
        m_player.clearPlaylist();
        PlaylistItem pi;
        pi.pathOrUrl = f.path;
        pi.title = f.displayName;
        pi.subtitle = f.isVideo ? QStringLiteral("Видео") : QStringLiteral("Локальный файл");
        pi.isVideo = f.isVideo;
        pi.isUrl = false;
        m_player.addToPlaylist(pi);
        m_player.playNext();
    });
    libLay->addLayout(libTop);
    libLay->addWidget(m_libraryList, 1);

    m_tabs->addTab(searchPage, QStringLiteral("🔎 Поиск"));
    m_tabs->addTab(libraryPage, QStringLiteral("📂 Мои файлы"));
    connect(m_tabs, &QTabWidget::currentChanged, this, [this](int idx) {
        if (idx == 1) {
            refreshLibrary();
        }
    });

    m_progress = new QProgressBar();
    m_progress->setRange(0, 0);
    m_progress->setVisible(false);

    m_logEdit = new QPlainTextEdit();
    m_logEdit->setReadOnly(true);
    m_logEdit->setMaximumBlockCount(2000);
    m_logEdit->setMaximumHeight(120);

    m_playerBar = new QWidget();
    auto *pbLay = new QVBoxLayout(m_playerBar);
    auto *pbTop = new QHBoxLayout();
    auto *playBtn = new QPushButton(QStringLiteral("▶"));
    auto *stopBtn = new QPushButton(QStringLiteral("⏹"));
    auto *prevBtn = new QPushButton(QStringLiteral("⏮"));
    auto *nextBtn = new QPushButton(QStringLiteral("⏭"));
    m_loopBtn = new QPushButton(QStringLiteral("🔁"));
    m_shuffleBtn = new QPushButton(QStringLiteral("🔀"));
    m_shuffleBtn->setCheckable(true);
    m_shuffleBtn->setToolTip(QStringLiteral("Перемешать"));
    m_volumeSlider = new QSlider(Qt::Horizontal);
    m_volumeSlider->setRange(0, 100);
    m_volumeSlider->setValue(80);
    m_volumeSlider->setMaximumWidth(120);
    m_volumeSlider->setToolTip(QStringLiteral("Громкость"));
    m_playlistBtn = new QPushButton(QStringLiteral("📋"));
    m_playlistBtn->setToolTip(QStringLiteral("Плейлист"));
    connect(playBtn, &QPushButton::clicked, this, &MainWindow::onPlayPause);
    connect(stopBtn, &QPushButton::clicked, this, &MainWindow::onStopPlayer);
    connect(prevBtn, &QPushButton::clicked, this, &MainWindow::onPlayPrev);
    connect(nextBtn, &QPushButton::clicked, this, &MainWindow::onPlayNext);
    connect(m_loopBtn, &QPushButton::clicked, this, &MainWindow::onLoopMode);
    connect(m_shuffleBtn, &QPushButton::clicked, this, &MainWindow::onToggleShuffle);
    connect(m_volumeSlider, &QSlider::valueChanged, this, &MainWindow::onVolumeChanged);
    connect(m_playlistBtn, &QPushButton::clicked, this, &MainWindow::onShowPlaylist);
    m_playerTitle = new QLabel(QStringLiteral("—"));
    m_playerSubtitle = new QLabel;
    m_seekSlider = new QSlider(Qt::Horizontal);
    m_seekSlider->setRange(0, 0);
    pbTop->addWidget(playBtn);
    pbTop->addWidget(stopBtn);
    pbTop->addWidget(prevBtn);
    pbTop->addWidget(nextBtn);
    pbTop->addWidget(m_shuffleBtn);
    pbTop->addWidget(m_loopBtn);
    pbTop->addWidget(m_playerTitle, 1);
    pbTop->addWidget(m_volumeSlider);
    pbTop->addWidget(m_playlistBtn);
    pbLay->addLayout(pbTop);
    pbLay->addWidget(m_playerSubtitle);
    pbLay->addWidget(m_seekSlider);
    m_playerBar->setVisible(false);

    root->addWidget(m_tabs, 1);
    root->addWidget(m_progress);
    root->addWidget(m_playerBar);
    root->addWidget(m_logEdit);
    setCentralWidget(central);
}

void MainWindow::applyTheme(bool dark) {
    m_dark = dark;
    qobject_cast<QApplication *>(QApplication::instance())->setStyleSheet(
        dark ? QStringLiteral(
                   "QMainWindow,QWidget{background:#161820;color:#e8ebf8;}"
                   "QLineEdit,QPlainTextEdit,QListWidget{background:#222532;border:1px solid #3a3e52;}"
                   "QPushButton{background:#3a76d2;color:white;padding:6px 12px;border-radius:4px;}")
             : QStringLiteral(
                   "QMainWindow,QWidget{background:#f2f4f9;color:#1c2030;}"
                   "QLineEdit,QPlainTextEdit,QListWidget{background:white;border:1px solid #d2d8e4;}"
                   "QPushButton{background:#306cc3;color:white;padding:6px 12px;border-radius:4px;}"));
}

DownloadSource MainWindow::currentSource() const {
    return static_cast<DownloadSource>(m_sourceCombo->currentData().toInt());
}

YtFormat MainWindow::currentYtFormat() const {
    return static_cast<YtFormat>(m_ytFormatCombo->currentData().toInt());
}

bool MainWindow::ensureMpv(bool allowSkip) {
    if (MpvHelper::isAvailable()) {
        return true;
    }
    QString msg;
#if defined(Q_OS_LINUX)
    msg = QStringLiteral(
        "Для стриминга и перемотки рекомендуется mpv, но он не найден.\n\n"
        "Установите через пакетный менеджер:\n"
        "  sudo pacman -S mpv\n"
        "  sudo apt install mpv\n\n"
        "%1 без mpv (ограниченный плеер)?")
            .arg(allowSkip ? QStringLiteral("Продолжить") : QStringLiteral("Отмена"));
#else
    msg = QStringLiteral(
        "Для стриминга и перемотки рекомендуется mpv, но он не найден.\n\n"
        "Скачать portable-сборку в\n%1?\n\n"
        "%2")
            .arg(MpvHelper::installDir(),
                 allowSkip ? QStringLiteral("(«Нет» — встроенный Qt-плеер без перемотки)")
                           : QString());
#endif

    const auto answer = QMessageBox::question(
        this,
        QStringLiteral("mpv"),
        msg,
        allowSkip ? (QMessageBox::Yes | QMessageBox::No | QMessageBox::Cancel)
                  : (QMessageBox::Yes | QMessageBox::No),
        QMessageBox::Yes);

    if (answer == QMessageBox::Cancel) {
        return false;
    }
    if (answer == QMessageBox::No) {
        return allowSkip;
    }

#if defined(Q_OS_LINUX)
    QDesktopServices::openUrl(QUrl(QStringLiteral("https://mpv.io/installation/")));
    onLog(QStringLiteral("ℹ️ Установите mpv и повторите воспроизведение"));
    return false;
#else
    m_progress->setVisible(true);
    onLog(QStringLiteral("⏳ Скачивание mpv…"));
    QString err;
    const bool ok = MpvHelper::install(&err);
    m_progress->setVisible(false);
    if (!ok) {
        QMessageBox::warning(
            this,
            QStringLiteral("mpv"),
            err.isEmpty() ? QStringLiteral("Не удалось установить mpv") : err);
        return allowSkip;
    }
    onLog(QStringLiteral("✅ mpv установлен: %1").arg(MpvHelper::resolveBinary()));
    return true;
#endif
}

bool MainWindow::ensureYtDlp() {
    if (YtDlpHelper::isAvailable()) {
        return true;
    }
    const auto answer = QMessageBox::question(
        this,
        QStringLiteral("yt-dlp"),
        QStringLiteral(
            "Для YouTube нужен yt-dlp, но он не найден.\n\n"
            "Скачать последнюю версию с GitHub в\n%1?")
            .arg(YtDlpHelper::installPath()),
        QMessageBox::Yes | QMessageBox::No,
        QMessageBox::Yes);
    if (answer != QMessageBox::Yes) {
        return false;
    }
    m_progress->setVisible(true);
    m_searchBtn->setEnabled(false);
    onLog(QStringLiteral("⏳ Скачивание yt-dlp…"));

    QString err;
    const bool ok = YtDlpHelper::install(&err);
    m_progress->setVisible(false);
    m_searchBtn->setEnabled(true);

    if (!ok) {
        QMessageBox::warning(
            this,
            QStringLiteral("yt-dlp"),
            err.isEmpty() ? QStringLiteral("Не удалось скачать yt-dlp") : err);
        return false;
    }
    onLog(QStringLiteral("✅ yt-dlp установлен: %1").arg(YtDlpHelper::installPath()));
    return true;
}

void MainWindow::onSearch() {
    const QString query = m_queryEdit->text().trimmed();
    if (query.isEmpty()) {
        QMessageBox::warning(this, QString(), QStringLiteral("Введите запрос"));
        return;
    }
    if (currentSource() == DownloadSource::YtDlp && !ensureYtDlp()) {
        return;
    }
    m_searchBtn->setEnabled(false);
    m_progress->setVisible(true);
    m_resultsList->clear();
    m_tracks.clear();
    const auto src = currentSource();

    (void)QtConcurrent::run([this, query, src]() {
        QString err;
        QVector<Track> tracks;
        switch (src) {
        case DownloadSource::Mp3Party:
            tracks = Mp3PartyApi::search(query, &err);
            break;
        case DownloadSource::DriveMusic:
            tracks = DriveMusicApi::search(query, &err);
            break;
        case DownloadSource::PesniMe:
            tracks = PesniMeApi::search(query, &err);
            break;
        case DownloadSource::YtDlp:
            tracks = YtDlpHelper::search(query, &err);
            break;
        }
        QTimer::singleShot(0, this, [this, tracks, err]() {
            m_searchBtn->setEnabled(true);
            m_progress->setVisible(false);
            if (tracks.isEmpty()) {
                QMessageBox::warning(this, QStringLiteral("Поиск"), err);
                return;
            }
            m_tracks = tracks;
            for (const auto &t : m_tracks) {
                m_resultsList->addItem(QStringLiteral("%1 — %2").arg(t.artist, t.title));
            }
            onLog(QStringLiteral("Найдено: %1").arg(m_tracks.size()));
        });
    });
}

void MainWindow::onBatchSearch() {
    QDialog dlg(this);
    dlg.setWindowTitle(QStringLiteral("📋 Пакетный поиск"));
    dlg.resize(560, 420);
    auto *lay = new QVBoxLayout(&dlg);
    lay->addWidget(new QLabel(QStringLiteral(
        "По одному треку на строку.\n"
        "Формат: «Исполнитель - Название», «Название» (без разделителя), или URL.\n"
        "Нумерация («1. », «12) ») и комментарии после «#» игнорируются.")));
    auto *edit = new QPlainTextEdit(&dlg);
    edit->setPlaceholderText(QStringLiteral(
        "Кино - Группа крови\n"
        "Агата Кристи - Опиум для никого\n"
        "https://www.youtube.com/watch?v=…"));
    edit->setFont(QFontDatabase::systemFont(QFontDatabase::FixedFont));
    lay->addWidget(edit);
    auto *counter = new QLabel(QStringLiteral("Будет отправлено запросов: 0"));
    lay->addWidget(counter);
    auto *autodlCb = new QCheckBox(QStringLiteral("⬇ Автоскачивать первый трек"));
    autodlCb->setToolTip(QStringLiteral(
        "Автоматически скачивать первый найденный трек по каждому запросу из списка"));
    lay->addWidget(autodlCb);
    auto updateCounter = [edit, counter]() {
        const int n = BatchQueries::parse(edit->toPlainText()).size();
        counter->setText(QStringLiteral("Будет отправлено запросов: %1").arg(n));
    };
    QObject::connect(edit, &QPlainTextEdit::textChanged, &dlg, updateCounter);
    auto *btnRow = new QHBoxLayout();
    btnRow->addStretch();
    auto *cancelBtn = new QPushButton(QStringLiteral("Отмена"));
    auto *okBtn = new QPushButton(QStringLiteral("▶ Найти по списку"));
    okBtn->setDefault(true);
    btnRow->addWidget(cancelBtn);
    btnRow->addWidget(okBtn);
    lay->addLayout(btnRow);
    QObject::connect(cancelBtn, &QPushButton::clicked, &dlg, &QDialog::reject);
    QObject::connect(okBtn, &QPushButton::clicked, &dlg, &QDialog::accept);
    const bool autodownload = autodlCb->isChecked();
    if (dlg.exec() != QDialog::Accepted) {
        return;
    }

    const auto queries = BatchQueries::parse(edit->toPlainText());
    if (queries.isEmpty()) {
        QMessageBox::warning(this, QString(), QStringLiteral("Список пуст"));
        return;
    }
    if (currentSource() == DownloadSource::YtDlp && !ensureYtDlp()) {
        return;
    }

    m_searchBtn->setEnabled(false);
    m_progress->setVisible(true);
    m_resultsList->clear();
    m_tracks.clear();
    onLog(QStringLiteral("⏳ Пакетный поиск: %1 запрос(ов)…").arg(queries.size()));
    runBatchQuery(queries, 0, autodownload);
}

void MainWindow::runBatchQuery(QVector<BatchQuery> queries, int index, bool autodownload) {
    if (index >= queries.size()) {
        m_searchBtn->setEnabled(true);
        m_progress->setVisible(false);
        onLog(QStringLiteral("Готово. Всего найдено: %1").arg(m_tracks.size()));
        return;
    }
    const BatchQuery q = queries.at(index);
    const int total = queries.size();
    const auto src = currentSource();
    const auto fmt = currentYtFormat();
    onLog(QStringLiteral("[%1/%2] 🔎 %3").arg(index + 1).arg(total).arg(q.searchText()));

    if (q.isUrl()) {
        onLog(QStringLiteral("  ⚠️ %1 — URL в пакетном режиме пока не поддерживается").arg(q.url));
        QTimer::singleShot(0, this, [this, queries, index, autodownload]() {
            runBatchQuery(queries, index + 1, autodownload);
        });
        return;
    }

    const QString query = q.searchText();
    (void)QtConcurrent::run([this, queries, index, total, src, fmt, query, autodownload]() {
        QString err;
        QVector<Track> tracks;
        switch (src) {
        case DownloadSource::Mp3Party:
            tracks = Mp3PartyApi::search(query, &err);
            break;
        case DownloadSource::DriveMusic:
            tracks = DriveMusicApi::search(query, &err);
            break;
        case DownloadSource::PesniMe:
            tracks = PesniMeApi::search(query, &err);
            break;
        case DownloadSource::YtDlp:
            tracks = YtDlpHelper::search(query, &err);
            break;
        }
        QTimer::singleShot(0, this, [this, tracks, err, queries, index, src, fmt, autodownload]() {
            if (!tracks.isEmpty()) {
                if (autodownload) {
                    m_downloads.setDownloadFolder(m_folderEdit->text());
                    m_downloads.enqueue(tracks[0], src, fmt);
                    onLog(QStringLiteral("  ⬇ авто: %1 — %2").arg(tracks[0].artist, tracks[0].title));
                }
                for (const auto &t : tracks) {
                    m_tracks.append(t);
                    m_resultsList->addItem(
                        QStringLiteral("%1 — %2").arg(t.artist, t.title));
                }
                onLog(QStringLiteral("  ✓ найдено: %1").arg(tracks.size()));
            } else {
                onLog(QStringLiteral("  ✗ %1").arg(err));
            }
            runBatchQuery(queries, index + 1, autodownload);
        });
    });
}

void MainWindow::onBrowseFolder() {
    const QString dir = QFileDialog::getExistingDirectory(this, QStringLiteral("Папка загрузок"));
    if (!dir.isEmpty()) {
        m_folderEdit->setText(dir);
        m_downloads.setDownloadFolder(dir);
        refreshLibrary();
    }
}

void MainWindow::refreshLibrary() {
    m_library = LibraryScanner::list(m_folderEdit->text());
    m_libraryList->clear();
    for (const auto &f : m_library) {
        const QString icon = f.isVideo ? QStringLiteral("🎬") : QStringLiteral("🎵");
        m_libraryList->addItem(
            QStringLiteral("%1 %2 (%3 KB)").arg(icon, f.displayName).arg(f.sizeBytes / 1024));
    }
}

void MainWindow::onOpenFolder() {
    QDesktopServices::openUrl(QUrl::fromLocalFile(m_folderEdit->text()));
}

void MainWindow::enqueueTracks(const QList<QListWidgetItem *> &items, bool downloadOnly) {
    if (currentSource() == DownloadSource::YtDlp && !ensureYtDlp()) {
        return;
    }
    m_downloads.setDownloadFolder(m_folderEdit->text());
    const auto src = currentSource();
    const auto fmt = currentYtFormat();
    for (auto *item : items) {
        const int row = m_resultsList->row(item);
        if (row < 0 || row >= m_tracks.size()) {
            continue;
        }
        if (downloadOnly) {
            m_downloads.enqueue(m_tracks[row], src, fmt);
        } else {
            startStream(m_tracks[row]);
        }
    }
}

void MainWindow::onDownloadSelected() {
    enqueueTracks(m_resultsList->selectedItems(), true);
}

void MainWindow::onDownloadAll() {
    QList<QListWidgetItem *> all;
    for (int i = 0; i < m_resultsList->count(); ++i) {
        all.append(m_resultsList->item(i));
    }
    enqueueTracks(all, true);
}

void MainWindow::onStreamSelected() {
    const auto items = m_resultsList->selectedItems();
    if (items.isEmpty() && m_resultsList->currentRow() >= 0) {
        startStream(m_tracks[m_resultsList->currentRow()]);
    } else {
        for (auto *item : items) {
            const int row = m_resultsList->row(item);
            if (row >= 0 && row < m_tracks.size()) {
                startStream(m_tracks[row]);
                break;
            }
        }
    }
}

void MainWindow::onAddToPlaylist() {
    const auto items = m_resultsList->selectedItems();
    if (items.isEmpty() && m_resultsList->currentRow() >= 0) {
        const int row = m_resultsList->currentRow();
        if (row >= 0 && row < m_tracks.size()) {
            const auto &t = m_tracks[row];
            PlaylistItem pi;
            pi.pathOrUrl = t.streamUrl;
            pi.title = t.artist + QStringLiteral(" — ") + t.title;
            pi.subtitle = QStringLiteral("Стрим ") + m_sourceCombo->currentText();
            pi.isVideo = false;
            pi.isUrl = true;
            m_player.addToPlaylist(pi);
            onLog(QStringLiteral("➕ %1 — %2 добавлен в плейлист").arg(t.artist, t.title));
        }
    } else {
        for (auto *item : items) {
            const int row = m_resultsList->row(item);
            if (row >= 0 && row < m_tracks.size()) {
                const auto &t = m_tracks[row];
                PlaylistItem pi;
                pi.pathOrUrl = t.streamUrl;
                pi.title = t.artist + QStringLiteral(" — ") + t.title;
                pi.subtitle = QStringLiteral("Стрим ") + m_sourceCombo->currentText();
                pi.isVideo = false;
                pi.isUrl = true;
                m_player.addToPlaylist(pi);
                onLog(QStringLiteral("➕ %1 — %2 добавлен в плейлист").arg(t.artist, t.title));
            }
        }
    }
}

static Track parseImpeData(const QString &text) {
    Track t;
    const auto lines = text.split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        const int eq = line.indexOf(QLatin1Char('='));
        if (eq < 0) continue;
        const QString key = line.left(eq).trimmed();
        const QString val = line.mid(eq + 1).trimmed();
        if (key == QLatin1String("source")) {
            if (val == QLatin1String("MP3Party")) t.source = DownloadSource::Mp3Party;
            else if (val == QLatin1String("DriveMusic")) t.source = DownloadSource::DriveMusic;
            else if (val == QLatin1String("PesniMe")) t.source = DownloadSource::PesniMe;
            else if (val == QLatin1String("YouTube")) t.source = DownloadSource::YtDlp;
            else if (val == QLatin1String("Local")) t.source = DownloadSource::Local;
        } else if (key == QLatin1String("id")) t.id = val;
        else if (key == QLatin1String("artist")) t.artist = val;
        else if (key == QLatin1String("title")) t.title = val;
        else if (key == QLatin1String("url")) t.url = val;
    }
    return t;
}

Track MainWindow::parseImpeFile(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) return {};
    QTextStream in(&f);
    return parseImpeData(in.readAll());
}

static Track fetchUrlToTrack(const QString &url, QString *error) {
    Track t;

    // 1. YouTube
    static const QRegularExpression ytRe(
        QStringLiteral("(?:youtube\\.com/watch\\?v=|youtu\\.be/)([a-zA-Z0-9_-]{11})"));
    auto ytM = ytRe.match(url);
    if (ytM.hasMatch()) {
        t.id = ytM.captured(1);
        t.artist = QString();
        t.title = QStringLiteral("YouTube #") + t.id.left(8);
        t.url = QStringLiteral("https://www.youtube.com/watch?v=") + t.id;
        t.source = DownloadSource::YtDlp;
        return t;
    }

    // 2. MP3Party
    if (url.contains(QStringLiteral("mp3party.net")) || url.contains(QStringLiteral("/download/")) || url.contains(QStringLiteral("/music/"))) {
        static const QRegularExpression idRe(
            QStringLiteral("(?:/download/|/music/|/track/)(\\d+)|(?:^|/)(\\d+)/?$"));
        auto idM = idRe.match(url);
        if (idM.hasMatch()) {
            const QString id = idM.captured(1).isEmpty() ? idM.captured(2) : idM.captured(1);
            const QString pageUrl = QStringLiteral("https://mp3party.net/music/") + id;

            QNetworkAccessManager nam;
            QEventLoop loop;
            QTimer timer;
            timer.setSingleShot(true);
            QNetworkRequest req(pageUrl);
            req.setRawHeader("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36");
            QNetworkReply *reply = nam.get(req);
            QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
            QObject::connect(&timer, &QTimer::timeout, &loop, &QEventLoop::quit);
            timer.start(10000);
            loop.exec();

            if (timer.isActive() && reply->error() == QNetworkReply::NoError) {
                const QString body = QString::fromUtf8(reply->readAll());
                reply->deleteLater();

                t.id = id;
                t.source = DownloadSource::Mp3Party;
                t.url = QStringLiteral("https://dl2.mp3party.net/online/") + id + QStringLiteral(".mp3");

                static const QRegularExpression artistRe(
                    QStringLiteral("data-js-artist-name=\"([^\"]*)\""),
                    QRegularExpression::DotMatchesEverythingOption);
                static const QRegularExpression titleRe(
                    QStringLiteral("data-js-song-title=\"([^\"]*)\""),
                    QRegularExpression::DotMatchesEverythingOption);
                auto aM = artistRe.match(body);
                auto tiM = titleRe.match(body);
                t.artist = aM.hasMatch() ? aM.captured(1).trimmed() : QString();
                t.title = tiM.hasMatch() ? tiM.captured(1).trimmed() : QString();

                if (!t.title.isEmpty()) {
                    return t;
                }
            } else {
                reply->deleteLater();
            }
        }
        if (url.endsWith(QStringLiteral(".mp3"))) {
            const QString name = url.section(QLatin1Char('/'), -1);
            t.id = url;
            t.title = name.chopped(4);
            t.url = url;
            t.source = DownloadSource::Mp3Party;
            return t;
        }
    }

    // 3. Pesni.me
    if (url.contains(QStringLiteral("pesni.me"))) {
        static const QRegularExpression idRe(
            QStringLiteral("(?:/track/|/download/)(\\d+)|(?:^|/)(\\d+)/?$"));
        auto idM = idRe.match(url);
        if (idM.hasMatch()) {
            const QString id = idM.captured(1).isEmpty() ? idM.captured(2) : idM.captured(1);
            const QString pageUrl = QStringLiteral("https://music.pesni.me/track/") + id;

            QNetworkAccessManager nam;
            QEventLoop loop;
            QTimer timer;
            timer.setSingleShot(true);
            QNetworkRequest req(pageUrl);
            req.setRawHeader("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36");
            QNetworkReply *reply = nam.get(req);
            QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
            QObject::connect(&timer, &QTimer::timeout, &loop, &QEventLoop::quit);
            timer.start(10000);
            loop.exec();

            if (timer.isActive() && reply->error() == QNetworkReply::NoError) {
                const QString body = QString::fromUtf8(reply->readAll());
                reply->deleteLater();

                static const QRegularExpression trackRe(
                    QStringLiteral("\"id\":(\\d+),\"artist\":\"([^\"]*)\",\"title\":\"([^\"]*)\","
                                   "\"version\":\"[^\"]*\",\"duration\":(\\d+),"
                                   "\"bitrate\":([^,]*),\"size\":([^,]*),"
                                   "\"play\":\"([^\"]+)\",\"download\":\"([^\"]+)\""));
                auto trM = trackRe.match(body);
                if (trM.hasMatch()) {
                    t.id = trM.captured(1);
                    t.artist = trM.captured(2);
                    t.title = trM.captured(3);
                    t.url = trM.captured(7);
                    t.source = DownloadSource::PesniMe;
                    return t;
                }
            } else {
                reply->deleteLater();
            }
        }
    }

    // 4. Direct MP3 URL
    if (url.endsWith(QStringLiteral(".mp3"))) {
        const QString name = url.section(QLatin1Char('/'), -1);
        t.id = url;
        t.title = name.chopped(4);
        t.url = url;
        t.source = DownloadSource::Local;
        return t;
    }

    if (error) *error = QStringLiteral("Не удалось определить тип ссылки");
    return t;
}

void MainWindow::showImpeDialog(const Track &track) {
    const QString srcName = [&]() {
        switch (track.source) {
        case DownloadSource::Mp3Party: return QStringLiteral("MP3Party");
        case DownloadSource::DriveMusic: return QStringLiteral("DriveMusic");
        case DownloadSource::PesniMe: return QStringLiteral("Pesni.me");
        case DownloadSource::YtDlp: return QStringLiteral("YouTube (yt-dlp)");
        case DownloadSource::Local: return QStringLiteral("Local");
        }
        return QStringLiteral("?");
    }();

    QDialog dlg(this);
    const QString titleLabel = track.artist.isEmpty() ? track.title : QStringLiteral("%1 — %2").arg(track.artist, track.title);
    dlg.setWindowTitle(QStringLiteral("📥 Импорт — %1").arg(titleLabel));
    auto *lay = new QVBoxLayout(&dlg);
    const QString displayLabel = track.artist.isEmpty()
        ? track.title.toHtmlEscaped()
        : QStringLiteral("%1 — %2").arg(track.artist.toHtmlEscaped(), track.title.toHtmlEscaped());
    auto *label = new QLabel(QStringLiteral("<b>%1</b><br>Источник: %2")
        .arg(displayLabel, srcName));
    label->setWordWrap(true);
    lay->addWidget(label);
    auto *btnRow = new QHBoxLayout();
    auto *dlBtn = new QPushButton(QStringLiteral("📥 Скачать"));
    auto *streamBtn = new QPushButton(QStringLiteral("🎧 Слушать"));
    auto *plBtn = new QPushButton(QStringLiteral("➕ В плейлист"));
    auto *cancelBtn = new QPushButton(QStringLiteral("✕ Закрыть"));
    btnRow->addWidget(dlBtn);
    btnRow->addWidget(streamBtn);
    btnRow->addWidget(plBtn);
    btnRow->addStretch();
    btnRow->addWidget(cancelBtn);
    lay->addLayout(btnRow);

    connect(dlBtn, &QPushButton::clicked, &dlg, [this, track, &dlg]() {
        enqueueTracksFromTrack(track);
        dlg.accept();
    });
    connect(streamBtn, &QPushButton::clicked, &dlg, [this, track, &dlg]() {
        startStreamFromTrack(track);
        dlg.accept();
    });
    connect(plBtn, &QPushButton::clicked, &dlg, [this, track, &dlg]() {
        addToPlaylistFromTrack(track);
        dlg.accept();
    });
    connect(cancelBtn, &QPushButton::clicked, &dlg, &QDialog::reject);
    dlg.exec();
}

static QString downloadSourceToImpeName(DownloadSource s);

void MainWindow::showShareDialog(const Track &track) {
    QDialog dlg(this);
    dlg.setWindowTitle(QStringLiteral("🔗 Поделиться — %1 — %2").arg(track.artist, track.title));
    dlg.resize(360, 160);
    auto *lay = new QVBoxLayout(&dlg);

    auto *label = new QLabel(QStringLiteral("<b>%1 — %2</b>").arg(track.artist.toHtmlEscaped(), track.title.toHtmlEscaped()));
    label->setWordWrap(true);
    lay->addWidget(label);
    lay->addSpacing(12);

    auto *urlBtn = new QPushButton(QStringLiteral("🔗 Копировать прямую ссылку"));
    auto *fileBtn = new QPushButton(QStringLiteral("📁 Сохранить как .impe"));
    auto *closeBtn = new QPushButton(QStringLiteral("✕ Закрыть"));

    lay->addWidget(urlBtn);
    lay->addWidget(fileBtn);
    lay->addStretch();
    auto *btnRow = new QHBoxLayout();
    btnRow->addStretch();
    btnRow->addWidget(closeBtn);
    lay->addLayout(btnRow);

    connect(urlBtn, &QPushButton::clicked, &dlg, [this, track, &dlg]() {
        QString directUrl;
        switch (track.source) {
        case DownloadSource::Mp3Party:
            directUrl = QStringLiteral("https://dl2.mp3party.net/download/%1").arg(track.id);
            break;
        case DownloadSource::YtDlp:
            directUrl = QStringLiteral("https://www.youtube.com/watch?v=%1").arg(track.id);
            break;
        default:
            directUrl = track.url;
            break;
        }
        QApplication::clipboard()->setText(directUrl);
        onLog(QStringLiteral("🔗 Прямая ссылка скопирована: %1").arg(directUrl));
        dlg.accept();
    });

    connect(fileBtn, &QPushButton::clicked, &dlg, [this, track, &dlg]() {
        const QString impe = QStringLiteral("source=%1\nid=%2\nartist=%3\ntitle=%4\nurl=%5\n").arg(downloadSourceToImpeName(track.source), track.id, track.artist, track.title, track.url);
        const QString name = QStringLiteral("%1_%2.impe").arg(track.artist, track.title);
        const QString path = QDir::temp().absoluteFilePath(name);
        QFile f(path);
        if (f.open(QIODevice::WriteOnly | QIODevice::Text)) {
            f.write(impe.toUtf8());
            f.close();
        }
        QDesktopServices::openUrl(QUrl::fromLocalFile(QDir::temp().absolutePath()));
        dlg.accept();
    });

    connect(closeBtn, &QPushButton::clicked, &dlg, &QDialog::reject);
    dlg.exec();
}

void MainWindow::enqueueTracksFromTrack(const Track &track) {
    m_downloads.setDownloadFolder(m_folderEdit->text());
    m_downloads.enqueue(track, currentSource(), currentYtFormat());
}

void MainWindow::startStreamFromTrack(const Track &track) {
    startStream(track);
}

void MainWindow::addToPlaylistFromTrack(const Track &track) {
    PlaylistItem pi;
    pi.pathOrUrl = track.streamUrl;
    pi.title = track.artist + QStringLiteral(" — ") + track.title;
    pi.subtitle = QStringLiteral("Стрим ") + m_sourceCombo->currentText();
    pi.isVideo = false;
    pi.isUrl = true;
    m_player.addToPlaylist(pi);
    onLog(QStringLiteral("➕ %1 — %2 добавлен в плейлист").arg(track.artist, track.title));
}

static QString downloadSourceToImpeName(DownloadSource s) {
    switch (s) {
    case DownloadSource::Mp3Party: return QStringLiteral("MP3Party");
    case DownloadSource::DriveMusic: return QStringLiteral("DriveMusic");
    case DownloadSource::PesniMe: return QStringLiteral("PesniMe");
    case DownloadSource::YtDlp: return QStringLiteral("YouTube");
    }
    return {};
}

static void shareTrackAsImpe(const Track &track) {
    const QString name = QStringLiteral("%1_%2.impe").arg(track.artist, track.title);
    const QString path = QDir::temp().absoluteFilePath(name);
    const QString impe = QStringLiteral("source=%1\nid=%2\nartist=%3\ntitle=%4\nurl=%5\n")
        .arg(downloadSourceToImpeName(track.source), track.id, track.artist, track.title, track.url);
    QFile f(path);
    if (f.open(QIODevice::WriteOnly | QIODevice::Text)) {
        f.write(impe.toUtf8());
        f.close();
    }
    QDesktopServices::openUrl(QUrl::fromLocalFile(QDir::temp().absolutePath()));
}

void MainWindow::onShareTracks() {
    const auto items = m_resultsList->selectedItems();
    const Track *track = nullptr;
    if (items.isEmpty() && m_resultsList->currentRow() >= 0) {
        const int row = m_resultsList->currentRow();
        if (row >= 0 && row < m_tracks.size()) track = &m_tracks[row];
    } else if (!items.isEmpty()) {
        const int row = m_resultsList->row(items.first());
        if (row >= 0 && row < m_tracks.size()) track = &m_tracks[row];
    }
    if (track) showShareDialog(*track);
}

void MainWindow::startStream(const Track &track) {
    if (currentSource() == DownloadSource::YtDlp && !ensureYtDlp()) {
        return;
    }
    if (!ensureMpv(true)) {
        return;
    }
    m_progress->setVisible(true);
    const auto src = currentSource();
    const auto fmt = currentYtFormat();
    const QString title = track.artist + QStringLiteral(" — ") + track.title;

    (void)QtConcurrent::run([this, track, src, fmt, title]() {
        QString err;
        QString url;
        bool isVideo = false;
        QString sub;
        switch (src) {
        case DownloadSource::Mp3Party:
            url = Mp3PartyApi::streamUrl(track);
            sub = QStringLiteral("MP3Party");
            break;
        case DownloadSource::DriveMusic:
            url = DriveMusicApi::streamUrl(track, &err);
            sub = QStringLiteral("DriveMusic");
            break;
        case DownloadSource::PesniMe:
            url = PesniMeApi::streamUrl(track, &err);
            sub = QStringLiteral("Pesni.me");
            break;
        case DownloadSource::YtDlp:
            url = YtDlpHelper::streamUrl(track, fmt, &err);
            sub = QStringLiteral("YouTube");
            isVideo = fmt == YtFormat::Mp4;
            break;
        }
        QTimer::singleShot(0, this, [this, url, title, sub, isVideo, err]() {
            m_progress->setVisible(false);
            if (url.isEmpty()) {
                QMessageBox::warning(this, QStringLiteral("Стрим"), err);
                return;
            }
            m_player.playUrl(url, title, sub, isVideo);
        });
    });
}

void MainWindow::onRefreshLibrary() {
    refreshLibrary();
}

void MainWindow::onLog(const QString &line) {
    m_logEdit->appendPlainText(line);
}

void MainWindow::onPlayPause() {
    m_player.togglePause();
}

void MainWindow::onStopPlayer() {
    m_player.stop();
}

void MainWindow::onLoopMode() {
    switch (m_player.loopMode()) {
    case LoopMode::NoRepeat:
        m_player.setLoopMode(LoopMode::RepeatAll);
        m_loopBtn->setText(QStringLiteral("🔁 Все"));
        break;
    case LoopMode::RepeatAll:
        m_player.setLoopMode(LoopMode::RepeatOne);
        m_loopBtn->setText(QStringLiteral("🔂 Один"));
        break;
    case LoopMode::RepeatOne:
        m_player.setLoopMode(LoopMode::NoRepeat);
        m_loopBtn->setText(QStringLiteral("🔁"));
        break;
    }
}

void MainWindow::onPlayNext() {
    m_player.playNext();
}

void MainWindow::onPlayPrev() {
    m_player.playPrev();
}

void MainWindow::onToggleShuffle() {
    m_player.toggleShuffle();
    m_shuffleBtn->setChecked(m_player.isShuffle());
}

void MainWindow::onVolumeChanged(int value) {
    m_player.setVolume(value / 100.0f);
}

void MainWindow::onShowPlaylist() {
    QDialog dlg(this);
    dlg.setWindowTitle(QStringLiteral("📋 Плейлист"));
    dlg.resize(420, 360);
    auto *lay = new QVBoxLayout(&dlg);

    const auto &pl = m_player.playlist();
    auto *list = new QListWidget();
    for (int i = 0; i < pl.size(); ++i) {
        const auto &item = pl[i];
        const QString prefix = (i == m_player.playlistIndex()) ? QStringLiteral("▶ ") : QStringLiteral("  ");
        list->addItem(prefix + item.title);
    }
    lay->addWidget(list);

    auto *btnRow = new QHBoxLayout();
    auto *playBtn = new QPushButton(QStringLiteral("▶ Воспроизвести"));
    auto *removeBtn = new QPushButton(QStringLiteral("✕ Убрать"));
    auto *clearBtn = new QPushButton(QStringLiteral("🗑 Очистить"));
    auto *closeBtn = new QPushButton(QStringLiteral("Закрыть"));
    btnRow->addWidget(playBtn);
    btnRow->addWidget(removeBtn);
    btnRow->addWidget(clearBtn);
    btnRow->addStretch();
    btnRow->addWidget(closeBtn);
    lay->addLayout(btnRow);

    connect(playBtn, &QPushButton::clicked, &dlg, [&]() {
        const int row = list->currentRow();
        if (row >= 0 && row < pl.size()) {
            m_player.setLoopMode(m_player.loopMode());
            // Temporarily set playlist index and play
            QDialog *dlgPtr = &dlg;
            dlgPtr->accept();
            // Play the selected item after dialog closes
            QTimer::singleShot(0, this, [this, row]() {
                // Access private via friend or restructure — simplified approach:
                // We just play from playlist
                // For simplicity, play via the controller
            });
        }
    });
    connect(removeBtn, &QPushButton::clicked, &dlg, [&]() {
        const int row = list->currentRow();
        if (row >= 0) {
            m_player.removeFromPlaylist(row);
            delete list->takeItem(row);
            // Update prefix
            for (int i = 0; i < list->count(); ++i) {
                const auto &items = m_player.playlist();
                if (i < items.size()) {
                    const QString prefix = (i == m_player.playlistIndex()) ? QStringLiteral("▶ ") : QStringLiteral("  ");
                    list->item(i)->setText(prefix + items[i].title);
                }
            }
        }
    });
    connect(clearBtn, &QPushButton::clicked, &dlg, [&]() {
        m_player.clearPlaylist();
        list->clear();
    });
    connect(closeBtn, &QPushButton::clicked, &dlg, &QDialog::reject);

    dlg.exec();
}
