#include "mainwindow.h"

#include <QApplication>
#include <QComboBox>
#include <QDir>
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
#include "ytdlphelper.h"

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
    setupUi();
    m_folderEdit->setText(QDir::homePath() + QStringLiteral("/mp3_downloader_gui/downloads"));
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
    connect(m_searchBtn, &QPushButton::clicked, this, &MainWindow::onSearch);
    top->addWidget(new QLabel(QStringLiteral("Источник:")));
    top->addWidget(m_sourceCombo);
    top->addWidget(new QLabel(QStringLiteral("YT:")));
    top->addWidget(m_ytFormatCombo);
    top->addWidget(m_queryEdit, 1);
    top->addWidget(m_searchBtn);

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
    connect(dlSel, &QPushButton::clicked, this, &MainWindow::onDownloadSelected);
    connect(streamBtn, &QPushButton::clicked, this, &MainWindow::onStreamSelected);
    connect(dlAll, &QPushButton::clicked, this, &MainWindow::onDownloadAll);
    btnRow->addWidget(dlSel);
    btnRow->addWidget(streamBtn);
    btnRow->addWidget(dlAll);
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
        m_player.playFile(f.path, f.displayName, f.isVideo);
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
    connect(playBtn, &QPushButton::clicked, this, &MainWindow::onPlayPause);
    connect(stopBtn, &QPushButton::clicked, this, &MainWindow::onStopPlayer);
    m_playerTitle = new QLabel(QStringLiteral("—"));
    m_playerSubtitle = new QLabel;
    m_seekSlider = new QSlider(Qt::Horizontal);
    m_seekSlider->setRange(0, 0);
    pbTop->addWidget(playBtn);
    pbTop->addWidget(stopBtn);
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

void MainWindow::onSearch() {
    const QString query = m_queryEdit->text().trimmed();
    if (query.isEmpty()) {
        QMessageBox::warning(this, QString(), QStringLiteral("Введите запрос"));
        return;
    }
    m_searchBtn->setEnabled(false);
    m_progress->setVisible(true);
    m_resultsList->clear();
    m_tracks.clear();
    const auto src = currentSource();

    QtConcurrent::run([this, query, src]() {
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

void MainWindow::startStream(const Track &track) {
    m_progress->setVisible(true);
    const auto src = currentSource();
    const auto fmt = currentYtFormat();
    const QString title = track.artist + QStringLiteral(" — ") + track.title;

    QtConcurrent::run([this, track, src, fmt, title]() {
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
