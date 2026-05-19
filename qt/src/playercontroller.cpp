#include "playercontroller.h"

#include <QAudioOutput>
#include <QLabel>
#include <QMediaPlayer>
#include <QSlider>
#include <QUrl>
#include <QVideoWidget>
#include <QVBoxLayout>
#include <QWidget>

PlayerController::PlayerController(QObject *parent) : QObject(parent) {
    m_player = new QMediaPlayer(this);
    m_audio = new QAudioOutput(this);
    m_player->setAudioOutput(m_audio);
    m_video = new QVideoWidget();
    m_video->setWindowTitle(QStringLiteral("MP3 Downloader — видео"));
    m_player->setVideoOutput(m_video);

    connect(m_player, &QMediaPlayer::positionChanged, this, &PlayerController::onPositionChanged);
    connect(m_player, &QMediaPlayer::durationChanged, this, &PlayerController::onDurationChanged);
    connect(m_player, &QMediaPlayer::playbackStateChanged, this, &PlayerController::onPlaybackStateChanged);
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

void PlayerController::playFile(const QString &path, const QString &title, bool isVideo) {
    stop();
    if (m_title) {
        m_title->setText(title);
    }
    if (m_subtitle) {
        m_subtitle->setText(isVideo ? QStringLiteral("Видео") : QStringLiteral("Локальный файл"));
    }
    if (isVideo) {
        m_video->show();
    }
    m_player->setSource(QUrl::fromLocalFile(path));
    m_player->play();
    showBar(true);
}

void PlayerController::playUrl(const QString &url,
                               const QString &title,
                               const QString &subtitle,
                               bool isVideo) {
    stop();
    if (m_title) {
        m_title->setText(title);
    }
    if (m_subtitle) {
        m_subtitle->setText(subtitle);
    }
    if (isVideo) {
        m_video->show();
    }
    m_player->setSource(QUrl(url));
    m_player->play();
    showBar(true);
}

void PlayerController::togglePause() {
    if (m_player->playbackState() == QMediaPlayer::PlayingState) {
        m_player->pause();
    } else {
        m_player->play();
    }
}

void PlayerController::stop() {
    m_player->stop();
    m_video->hide();
    showBar(false);
    if (m_seek) {
        m_seek->setValue(0);
    }
}

bool PlayerController::hasMedia() const {
    return m_player->playbackState() != QMediaPlayer::StoppedState
           || m_player->mediaStatus() != QMediaPlayer::NoMedia;
}

void PlayerController::onPositionChanged(qint64 pos) {
    if (!m_seek || m_dragging) {
        return;
    }
    m_seek->blockSignals(true);
    m_seek->setValue(static_cast<int>(pos));
    m_seek->blockSignals(false);
}

void PlayerController::onDurationChanged(qint64 dur) {
    if (!m_seek) {
        return;
    }
    m_seek->setRange(0, static_cast<int>(dur > 0 ? dur : 0));
}

void PlayerController::onPlaybackStateChanged() {
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
    m_player->setPosition(ms);
}
