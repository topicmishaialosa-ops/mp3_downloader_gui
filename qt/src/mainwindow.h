#pragma once

#include <QMainWindow>
#include <QVector>

#include "batchqueries.h"
#include "downloadmanager.h"
#include "downloadsource.h"
#include "libraryscanner.h"
#include "playercontroller.h"
#include "track.h"

class QComboBox;
class QLineEdit;
class QListWidget;
class QPlainTextEdit;
class QProgressBar;
class QPushButton;
class QSlider;
class QTabWidget;
class QLabel;
class QWidget;

class MainWindow : public QMainWindow {
    Q_OBJECT
public:
    explicit MainWindow(QWidget *parent = nullptr);

private slots:
    void onSearch();
    void onBatchSearch();
    void onBrowseFolder();
    void onDownloadSelected();
    void onDownloadAll();
    void onStreamSelected();
    void onRefreshLibrary();
    void onOpenFolder();
    void onLog(const QString &line);
    void onPlayPause();
    void onStopPlayer();
    void onLoopMode();
    void onPlayNext();
    void onPlayPrev();
    void onAddToPlaylist();
    void onShareTracks();
    void onToggleShuffle();
    void onVolumeChanged(int value);
    void onShowPlaylist();

    void runBatchQuery(QVector<BatchQuery> queries, int index, bool autodownload = false);

public:
    void enqueueTracksFromTrack(const Track &track);
    void startStreamFromTrack(const Track &track);
    void addToPlaylistFromTrack(const Track &track);
    static Track parseImpeFile(const QString &path);
    void showImpeDialog(const Track &track);

private:
    void setupUi();
    void applyTheme(bool dark);
    void refreshLibrary();
    void enqueueTracks(const QList<class QListWidgetItem *> &items, bool downloadOnly);
    void startStream(const Track &track);
    bool ensureYtDlp();
    bool ensureMpv(bool allowSkip = true);
    DownloadSource currentSource() const;
    YtFormat currentYtFormat() const;

    QTabWidget *m_tabs = nullptr;
    QComboBox *m_sourceCombo = nullptr;
    QComboBox *m_ytFormatCombo = nullptr;
    QLineEdit *m_queryEdit = nullptr;
    QLineEdit *m_folderEdit = nullptr;
    QListWidget *m_resultsList = nullptr;
    QListWidget *m_libraryList = nullptr;
    QPlainTextEdit *m_logEdit = nullptr;
    QProgressBar *m_progress = nullptr;
    QPushButton *m_searchBtn = nullptr;
    QPushButton *m_loopBtn = nullptr;
    QPushButton *m_shuffleBtn = nullptr;
    QSlider *m_volumeSlider = nullptr;
    QPushButton *m_playlistBtn = nullptr;
    QLabel *m_playerTitle = nullptr;
    QLabel *m_playerSubtitle = nullptr;
    QSlider *m_seekSlider = nullptr;
    QWidget *m_playerBar = nullptr;

    PlayerController m_player;
    DownloadManager m_downloads;
    QVector<Track> m_tracks;
    QVector<LocalMediaFile> m_library;
    bool m_dark = true;
};
