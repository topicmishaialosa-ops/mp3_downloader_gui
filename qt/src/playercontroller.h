#pragma once

#include <QObject>
#include <QString>

class QAudioOutput;
class QLabel;
class QMediaPlayer;
class QSlider;
class QVideoWidget;

class PlayerController : public QObject {
    Q_OBJECT
public:
    explicit PlayerController(QObject *parent = nullptr);

    void bindUi(QLabel *title, QLabel *subtitle, QSlider *seek, QWidget *playerBar);
    void playFile(const QString &path, const QString &title, bool isVideo);
    void playUrl(const QString &url, const QString &title, const QString &subtitle, bool isVideo);
    void togglePause();
    void stop();
    bool hasMedia() const;

private slots:
    void onPositionChanged(qint64 pos);
    void onDurationChanged(qint64 dur);
    void onPlaybackStateChanged();

private:
    void showBar(bool show);
    void seekWhileDragging(int value);

    QMediaPlayer *m_player = nullptr;
    QAudioOutput *m_audio = nullptr;
    QVideoWidget *m_video = nullptr;
    QLabel *m_title = nullptr;
    QLabel *m_subtitle = nullptr;
    QSlider *m_seek = nullptr;
    QWidget *m_bar = nullptr;
    bool m_dragging = false;
};
