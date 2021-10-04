#pragma once
#include <QList>
#include <QMap>
#include <QMetaType>
#include <QString>
#include <QVariant>

namespace Qv2rayPlugin::Common::EditorCreator
{
    enum class ElementType
    {
        Bool = QMetaType::Bool,
        Integer = QMetaType::LongLong,
        Double = QMetaType::Double,
        String = QMetaType::QString,
        Array = QMetaType::QVariantList,
        Object = QMetaType::QVariantMap,
    };

    struct EditorInfo
    {
        QString key;
        QString name;
        ElementType type;
        QList<EditorInfo> objectElements;
        ElementType arrayElementType;

        template<ElementType t, typename = typename std::enable_if_t<t == ElementType::Array, void>>
        static inline EditorInfo Create(const QString &k, const QString &name, ElementType arrayType, const QList<EditorInfo> &children = {})
        {
            EditorInfo info;
            info.type = t;
            info.key = k;
            info.name = name;
            info.arrayElementType = arrayType;
            info.objectElements = children;
            return info;
        }

        template<ElementType t, typename = typename std::enable_if_t<t == ElementType::Object, void>>
        static inline EditorInfo Create(const QString &k, const QString &name, const QList<EditorInfo> &children)
        {
            EditorInfo info;
            info.type = t;
            info.key = k;
            info.name = name;
            info.objectElements = children;
            return info;
        }

        template<ElementType t, typename = typename std::enable_if_t<t != ElementType::Object && t != ElementType::Array, void>>
        static inline EditorInfo Create(const QString &k, const QString &name)
        {
            EditorInfo info;
            info.type = t;
            info.key = k;
            info.name = name;
            return info;
        }

        explicit EditorInfo(){};
    };

    typedef QList<EditorInfo> EditorInfoList;
} // namespace Qv2rayPlugin::Common::EditorCreator
