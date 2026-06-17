#pragma once

#include <QObject>
#include <QProcess>
#include <QString>
#include <QStringList>
#include <QVector>
#include <QTimer>

class QAudioOutput;
class QLabel;
class QMediaPlayer;
class QSlider;
class QUrl;
class QVideoWidget;
class QWidget;

enum class LoopMode {
    NoRepeat,
    RepeatAll,
    RepeatOne,
};

struct PlaylistItem {
    QString pathOrUrl;
    QString title;
    QString subtitle;
    bool isVideo = false;
    bool isUrl = false;
};

class PlayerController : public QObject {
    Q_OBJECT
public:
    explicit PlayerController(QObject *parent = nullptr);
    ~PlayerController() override;

    void bindUi(QLabel *title, QLabel *subtitle, QSlider *seek, QWidget *playerBar);
    void playFile(const QString &path, const QString &title, bool isVideo);
    void playUrl(const QString &url, const QString &title, const QString &subtitle, bool isVideo);
    void togglePause();
    void stop();
    bool hasMedia() const;

    void playNext();
    void playPrev();
    void addToPlaylist(const PlaylistItem &item);
    void clearPlaylist();
    void setLoopMode(LoopMode mode);
    LoopMode loopMode() const { return m_loopMode; }
    const QVector<PlaylistItem> &playlist() const { return m_playlist; }
    int playlistIndex() const { return m_playlistIndex; }

signals:
    void playlistChanged();

private slots:
    void onPositionChanged(qint64 pos);
    void onDurationChanged(qint64 dur);
    void onPlaybackStateChanged();
    void onMpvFinished(int exitCode, QProcess::ExitStatus status);
    void onTick();

private:
    void showBar(bool show);
    void seekWhileDragging(int value);
    void stopQtPlayer();
    void stopMpv();
    bool hasMpv() const;
    QString mpvExecutable() const;
    QString mpvSocketPath() const;
    bool startMpv(const QString &url, const QString &title, bool isVideo);
    void sendMpvCommand(const QStringList &args) const;
    void playWithQt(const QUrl &source, const QString &title, const QString &subtitle, bool allowVideo);
    void playCurrent();

    QMediaPlayer *m_player = nullptr;
    QAudioOutput *m_audio = nullptr;
    QVideoWidget *m_video = nullptr;
    QProcess *m_mpv = nullptr;
    QLabel *m_title = nullptr;
    QLabel *m_subtitle = nullptr;
    QSlider *m_seek = nullptr;
    QWidget *m_bar = nullptr;
    bool m_dragging = false;
    bool m_usingMpv = false;

    QTimer *m_tickTimer = nullptr;
    LoopMode m_loopMode = LoopMode::NoRepeat;
    QVector<PlaylistItem> m_playlist;
    int m_playlistIndex = 0;
};
