#pragma once
// Native BrowserSession.ts at 4cb6945dc60ed03f567dc5daafd083e52308e82b.
// Request-scoped adapter: never rewrite the profile's global User-Agent.
#include <QString>
#include <QHash>
#include <QByteArrayView>
#include <QUrl>
#include <QtWebEngineCore/QWebEngineUrlRequestInfo>
#include <QtWebEngineCore/QWebEngineUrlRequestInterceptor>
namespace TeamsRequestHeaders {
inline bool ecmaWhitespace(QChar character) {
    const auto value = character.unicode();
    return (value >= 0x9 && value <= 0xd) || value == 0x20 || value == 0xa0
        || value == 0x1680 || (value >= 0x2000 && value <= 0x200a)
        || value == 0x2028 || value == 0x2029 || value == 0x202f
        || value == 0x205f || value == 0x3000 || value == 0xfeff;
}
inline QString withoutElectron(QString value) {
    const QString prefix = QStringLiteral(" Electron/");
    qsizetype search = 0;
    while (true) {
        const auto start = value.indexOf(prefix, search);
        if (start < 0) return value;
        auto end = start + prefix.size();
        const auto token = end;
        while (end < value.size() && !ecmaWhitespace(value[end])) ++end;
        if (end > token) { value.remove(start, end - start); return value; }
        search = token;
    }
}
// Preserve every byte outside the matched span, including non-UTF-8 header
// bytes. Only the ECMAScript whitespace encodings terminate its UTF-8 token.
inline bool ecmaWhitespaceAt(const QByteArray &value, qsizetype offset) {
    const auto byte = static_cast<unsigned char>(value[offset]);
    if ((byte >= 9 && byte <= 13) || byte == 0x20) return true;
    const auto remaining = QByteArrayView(value).sliced(offset);
    for (const auto *encoded : {"\xc2\xa0", "\xe1\x9a\x80", "\xe2\x80\x80", "\xe2\x80\x81", "\xe2\x80\x82", "\xe2\x80\x83", "\xe2\x80\x84", "\xe2\x80\x85", "\xe2\x80\x86", "\xe2\x80\x87", "\xe2\x80\x88", "\xe2\x80\x89", "\xe2\x80\x8a", "\xe2\x80\xa8", "\xe2\x80\xa9", "\xe2\x80\xaf", "\xe2\x81\x9f", "\xe3\x80\x80", "\xef\xbb\xbf"}) {
        if (remaining.startsWith(encoded)) return true;
    }
    return false;
}
inline QByteArray withoutElectronBytes(QByteArray value) {
    const QByteArray prefix(" Electron/");
    qsizetype search = 0;
    while (true) {
        const auto start = value.indexOf(prefix, search);
        if (start < 0) return value;
        auto end = start + prefix.size();
        const auto token = end;
        while (end < value.size() && !ecmaWhitespaceAt(value, end)) ++end;
        if (end > token) { value.remove(start, end - start); return value; }
        search = token;
    }
}
inline bool matches(const QUrl &url) {
    return url.scheme() == QStringLiteral("https")
        && (url.host() == QStringLiteral("teams.cloud.microsoft")
            || url.host() == QStringLiteral("teams.microsoft.com"));
}
class Interceptor final : public QWebEngineUrlRequestInterceptor {
public:
    explicit Interceptor(QObject *parent = nullptr) : QWebEngineUrlRequestInterceptor(parent) {}
    void interceptRequest(QWebEngineUrlRequestInfo &request) override {
        if (!matches(request.requestUrl())) return;
        const auto headers = request.httpHeaders();
        for (auto header = headers.constBegin(); header != headers.constEnd(); ++header) {
            if (header.key().compare("user-agent", Qt::CaseInsensitive) != 0) continue;
            const auto original = header.value();
            const auto adapted = withoutElectronBytes(original);
            if (adapted != original) request.setHttpHeader(header.key(), adapted);
            return;
        }
    }
};
}
