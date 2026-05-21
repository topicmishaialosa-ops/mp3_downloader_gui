#pragma once

#include <QString>

class MpvHelper {
public:
    static QString installDir();
    static QString resolveBinary(QString *error = nullptr);
    static bool isAvailable();
    static bool install(QString *error = nullptr);
};
