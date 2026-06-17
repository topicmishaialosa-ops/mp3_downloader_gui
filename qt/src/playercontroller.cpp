#include "playercontroller.h"

#include "mpvhelper.h"

#include <QAudioOutput>
#include <QDir>
#include <QLabel>
#include <QMediaPlayer>
#include <QProcess>
#include <QSlider>
#include <QStandardPaths>
#include <QUrl>
#include <QVideoWidget>
#include <QWidget>
#include <QtGlobal>

PlayerController::PlayerController(QObject *parent) : QObject(parent) {
    m_player = new QMediaPlayer(this);
    m_audio = new QAudioOutput(this);
    m_player->setAudioOutput(m_audio);
    m_video = new QVideoWidget();
    m_video->setWindowTitle(QStringLiteral("MP3 Downloader — видео"));
    m_video->resize(960, 540);

    connect(m_player, &QMediaPlayer::positionChanged, this, &PlayerController::onPositionChanged);
    connect(m_player, &QMediaPlayer::durationChanged, this, &PlayerController::onDurationChanged);
    connect(m_player, &QMediaPlayer::playbackStateChanged, this, &PlayerController::onPlaybackStateChanged);

    m_tickTimer = new QTimer(this);
    m_tickTimer->setInterval(400);
    connect(m_tickTimer, &QTimer::timeout, this, &PlayerController::onTick);
}

PlayerController::~PlayerController() {
    stopMpv();
    delete m_video;
    m_video = nullptr;
}

void PlayerController::bindUi(QLabel *title, QLabel *subtitle, QSlider *seek, QWidget *playerBar) {
    m_title = title;
    m_subtitle = subtitle;
    m_seek = seek;
    m_bar = playerBar;
    if (m_seek) {
        connect(m_seek, &QSlider::sliderPressed, this, [this] { m_dragging = true; });
        connect(m_seek, &QSlider::sliderReleased, this, [this] {
            m_dragging = false;
            seekWhileDragging(m_seek->value());
        });
        connect(m_seek, &QSlider::valueChanged, this, [this](int v) {
            if (m_dragging) {
                seekWhileDragging(v);
            }
        });
    }
}

bool PlayerController::hasMpv() const {
    return MpvHelper::isAvailable();
}

QString PlayerController::mpvExecutable() const {
    return MpvHelper::resolveBinary();
}

QString PlayerController::mpvSocketPath() const {
#if defined(Q_OS_WIN)
    return QStringLiteral("\\\\.\\pipe\\mp3_downloader_gui_qt_mpv");
#else
    return QDir(QStandardPaths::writableLocation(QStandardPaths::TempLocation))
        .filePath(QStringLiteral("mp3_downloader_gui_qt_mpv.sock"));
#endif
}

void PlayerController::stopMpv() {
    if (!m_mpv) {
        m_usingMpv = false;
        return;
    }
    m_mpv->disconnect(this);
    if (m_mpv->state() != QProcess::NotRunning) {
        m_mpv->terminate();
        if (!m_mpv->waitForFinished(1500)) {
            m_mpv->kill();
            m_mpv->waitForFinished(1000);
        }
    }
    m_mpv->deleteLater();
    m_mpv = nullptr;
    m_usingMpv = false;
}

void PlayerController::stopQtPlayer() {
    m_player->stop();
    m_player->setVideoOutput(nullptr);
    m_video->hide();
}

void PlayerController::sendMpvCommand(const QStringList &args) const {
    const QString mpv = mpvExecutable();
    if (mpv.isEmpty()) {
        return;
    }
    QStringList cmd;
    cmd << QStringLiteral("--input-ipc-server=%1").arg(mpvSocketPath());
    cmd.append(args);
    QProcess::execute(mpv, cmd);
}

bool PlayerController::startMpv(const QString &url, const QString &title, bool isVideo) {
    const QString mpv = mpvExecutable();
    if (mpv.isEmpty()) {
        return false;
    }

    stopMpv();
    stopQtPlayer();

    QStringList args;
    args << QStringLiteral("--really-quiet")
         << QStringLiteral("--no-terminal")
         << QStringLiteral("--title")
         << title
         << QStringLiteral("--input-ipc-server=%1").arg(mpvSocketPath());

    if (!isVideo) {
        args << QStringLiteral("--no-video") << QStringLiteral("--force-window=no");
    }

    args << url;

    m_mpv = new QProcess(this);
    connect(m_mpv, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished), this,
            &PlayerController::onMpvFinished);
    m_mpv->start(mpv, args);
    if (!m_mpv->waitForStarted(5000)) {
        stopMpv();
        return false;
    }

    m_usingMpv = true;
    m_video->hide();
    m_tickTimer->start();
    return true;
}

void PlayerController::playWithQt(const QUrl &source,
                                  const QString &title,
                                  const QString &subtitle,
                                  bool allowVideo) {
    stopMpv();
    stopQtPlayer();

    if (m_title) {
        m_title->setText(title);
    }
    if (m_subtitle) {
        m_subtitle->setText(subtitle);
    }

    if (allowVideo) {
        m_player->setVideoOutput(m_video);
        m_video->show();
    } else {
        m_player->setVideoOutput(nullptr);
        m_video->hide();
    }

    m_player->setSource(source);
    m_player->play();
    showBar(true);
    m_tickTimer->start();
}

void PlayerController::playFile(const QString &path, const QString &title, bool isVideo) {
    stop();
    m_playlist.clear();
    PlaylistItem item;
    item.pathOrUrl = path;
    item.title = title;
    item.subtitle = isVideo ? QStringLiteral("Локальное видео") : QStringLiteral("Локальный файл");
    item.isVideo = isVideo;
    item.isUrl = false;
    m_playlist.append(item);
    m_playlistIndex = 0;

    if (isVideo && startMpv(QUrl::fromLocalFile(path).toString(), title, true)) {
        if (m_title) {
            m_title->setText(title);
        }
        if (m_subtitle) {
            m_subtitle->setText(QStringLiteral("Локальное видео (mpv)"));
        }
        showBar(true);
        return;
    }

    playWithQt(QUrl::fromLocalFile(path), title,
               isVideo ? QStringLiteral("Локальное видео") : QStringLiteral("Локальный файл"),
               isVideo);
}

void PlayerController::playUrl(const QString &url,
                               const QString &title,
                               const QString &subtitle,
                               bool isVideo) {
    stop();
    m_playlist.clear();
    PlaylistItem item;
    item.pathOrUrl = url;
    item.title = title;
    item.subtitle = subtitle;
    item.isVideo = isVideo;
    item.isUrl = true;
    m_playlist.append(item);
    m_playlistIndex = 0;

    if (m_title) {
        m_title->setText(title);
    }
    if (m_subtitle) {
        m_subtitle->setText(subtitle + QStringLiteral(" (mpv)"));
    }

    if (startMpv(url, title, isVideo)) {
        showBar(true);
        return;
    }

    if (m_subtitle) {
        m_subtitle->setText(subtitle + QStringLiteral(" (Qt)"));
    }
    playWithQt(QUrl(url), title, subtitle, false);
}

void PlayerController::togglePause() {
    if (m_usingMpv) {
        sendMpvCommand({QStringLiteral("cycle"), QStringLiteral("pause")});
        return;
    }
    if (m_player->playbackState() == QMediaPlayer::PlayingState) {
        m_player->pause();
    } else {
        m_player->play();
    }
}

void PlayerController::stop() {
    m_tickTimer->stop();
    stopMpv();
    stopQtPlayer();
    showBar(false);
    if (m_seek) {
        m_seek->setValue(0);
    }
    m_playlist.clear();
}

bool PlayerController::hasMedia() const {
    if (m_usingMpv && m_mpv && m_mpv->state() != QProcess::NotRunning) {
        return true;
    }
    return m_player->playbackState() != QMediaPlayer::StoppedState
           || m_player->mediaStatus() != QMediaPlayer::NoMedia;
}

void PlayerController::addToPlaylist(const PlaylistItem &item) {
    m_playlist.append(item);
    emit playlistChanged();
}

void PlayerController::clearPlaylist() {
    m_playlist.clear();
    emit playlistChanged();
}

void PlayerController::setLoopMode(LoopMode mode) {
    m_loopMode = mode;
}

void PlayerController::playCurrent() {
    if (m_playlistIndex < 0 || m_playlistIndex >= m_playlist.size()) {
        stop();
        return;
    }
    const auto &item = m_playlist[m_playlistIndex];
    if (item.isUrl) {
        playUrl(item.pathOrUrl, item.title, item.subtitle, item.isVideo);
    } else {
        playFile(item.pathOrUrl, item.title, item.isVideo);
    }
}

void PlayerController::playNext() {
    if (m_playlist.isEmpty()) {
        return;
    }
    switch (m_loopMode) {
    case LoopMode::NoRepeat:
        if (m_playlistIndex + 1 >= m_playlist.size()) {
            stop();
            return;
        }
        m_playlistIndex++;
        break;
    case LoopMode::RepeatAll:
        m_playlistIndex = (m_playlistIndex + 1) % m_playlist.size();
        break;
    case LoopMode::RepeatOne:
        if (m_playlistIndex >= m_playlist.size()) {
            m_playlistIndex = 0;
        }
        break;
    }
    playCurrent();
}

void PlayerController::playPrev() {
    if (m_playlist.isEmpty()) {
        return;
    }
    if (m_loopMode == LoopMode::RepeatAll) {
        if (m_playlistIndex == 0) {
            m_playlistIndex = m_playlist.size() - 1;
        } else {
            m_playlistIndex--;
        }
    } else {
        if (m_playlistIndex > 0) {
            m_playlistIndex--;
        }
    }
    playCurrent();
}

void PlayerController::onTick() {
    if (m_usingMpv && m_mpv && m_mpv->state() == QProcess::NotRunning) {
        m_tickTimer->stop();
        playNext();
        return;
    }
    if (!m_usingMpv && m_player->playbackState() == QMediaPlayer::StoppedState
        && m_player->mediaStatus() == QMediaPlayer::EndOfMedia) {
        m_tickTimer->stop();
        playNext();
    }
}

void PlayerController::onMpvFinished(int /*exitCode*/, QProcess::ExitStatus /*status*/) {
    stopMpv();
    showBar(false);
}

void PlayerController::onPositionChanged(qint64 pos) {
    if (m_usingMpv || !m_seek || m_dragging) {
        return;
    }
    m_seek->blockSignals(true);
    m_seek->setValue(static_cast<int>(pos));
    m_seek->blockSignals(false);
}

void PlayerController::onDurationChanged(qint64 dur) {
    if (m_usingMpv || !m_seek) {
        return;
    }
    m_seek->setRange(0, static_cast<int>(dur > 0 ? dur : 0));
}

void PlayerController::onPlaybackStateChanged() {
    if (m_usingMpv) {
        return;
    }
    if (m_player->playbackState() == QMediaPlayer::StoppedState
        && m_player->mediaStatus() == QMediaPlayer::EndOfMedia) {
        showBar(false);
    }
}

void PlayerController::showBar(bool show) {
    if (m_bar) {
        m_bar->setVisible(show);
    }
}

void PlayerController::seekWhileDragging(int ms) {
    if (m_usingMpv) {
        sendMpvCommand({QStringLiteral("seek"),
                        QString::number(ms / 1000.0, 'f', 2),
                        QStringLiteral("absolute")});
        return;
    }
    m_player->setPosition(ms);
}
