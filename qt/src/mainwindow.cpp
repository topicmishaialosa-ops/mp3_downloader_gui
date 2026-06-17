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
#include <QSlider>
#include <QTabWidget>
#include <QTimer>
#include <QUrl>
#include <QVBoxLayout>
#include <QtConcurrent>

#include "drivemusicapi.h"
#include "mp3partyapi.h"
#include "paths.h"
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
    connect(dlSel, &QPushButton::clicked, this, &MainWindow::onDownloadSelected);
    connect(streamBtn, &QPushButton::clicked, this, &MainWindow::onStreamSelected);
    connect(dlAll, &QPushButton::clicked, this, &MainWindow::onDownloadAll);
    connect(addPlaylistBtn, &QPushButton::clicked, this, &MainWindow::onAddToPlaylist);
    btnRow->addWidget(dlSel);
    btnRow->addWidget(streamBtn);
    btnRow->addWidget(dlAll);
    btnRow->addWidget(addPlaylistBtn);
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
    connect(refreshBtn, &QPushButton::clicked, this, &MainWindow::onRefreshLibrary);
    connect(openBtn, &QPushButton::clicked, this, &MainWindow::onOpenFolder);
    libTop->addWidget(refreshBtn);
    libTop->addWidget(openBtn);
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
    connect(playBtn, &QPushButton::clicked, this, &MainWindow::onPlayPause);
    connect(stopBtn, &QPushButton::clicked, this, &MainWindow::onStopPlayer);
    connect(prevBtn, &QPushButton::clicked, this, &MainWindow::onPlayPrev);
    connect(nextBtn, &QPushButton::clicked, this, &MainWindow::onPlayNext);
    connect(m_loopBtn, &QPushButton::clicked, this, &MainWindow::onLoopMode);
    m_playerTitle = new QLabel(QStringLiteral("—"));
    m_playerSubtitle = new QLabel;
    m_seekSlider = new QSlider(Qt::Horizontal);
    m_seekSlider->setRange(0, 0);
    pbTop->addWidget(playBtn);
    pbTop->addWidget(stopBtn);
    pbTop->addWidget(prevBtn);
    pbTop->addWidget(nextBtn);
    pbTop->addWidget(m_loopBtn);
    pbTop->addWidget(m_playerTitle, 1);
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
    runBatchQuery(queries, 0);
}

void MainWindow::runBatchQuery(QVector<BatchQuery> queries, int index) {
    if (index >= queries.size()) {
        m_searchBtn->setEnabled(true);
        m_progress->setVisible(false);
        onLog(QStringLiteral("Готово. Всего найдено: %1").arg(m_tracks.size()));
        return;
    }
    const BatchQuery q = queries.at(index);
    const int total = queries.size();
    const auto src = currentSource();
    onLog(QStringLiteral("[%1/%2] 🔎 %3").arg(index + 1).arg(total).arg(q.searchText()));

    if (q.isUrl()) {
        onLog(QStringLiteral("  ⚠️ %1 — URL в пакетном режиме пока не поддерживается").arg(q.url));
        QTimer::singleShot(0, this, [this, queries, index]() {
            runBatchQuery(queries, index + 1);
        });
        return;
    }

    const QString query = q.searchText();
    (void)QtConcurrent::run([this, queries, index, total, src, query]() {
        QString err;
        QVector<Track> tracks;
        switch (src) {
        case DownloadSource::Mp3Party:
            tracks = Mp3PartyApi::search(query, &err);
            break;
        case DownloadSource::DriveMusic:
            tracks = DriveMusicApi::search(query, &err);
            break;
        case DownloadSource::YtDlp:
            tracks = YtDlpHelper::search(query, &err);
            break;
        }
        QTimer::singleShot(0, this, [this, tracks, err, queries, index]() {
            if (!tracks.isEmpty()) {
                for (const auto &t : tracks) {
                    m_tracks.append(t);
                    m_resultsList->addItem(
                        QStringLiteral("%1 — %2").arg(t.artist, t.title));
                }
                onLog(QStringLiteral("  ✓ найдено: %1").arg(tracks.size()));
            } else {
                onLog(QStringLiteral("  ✗ %1").arg(err));
            }
            runBatchQuery(queries, index + 1);
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
