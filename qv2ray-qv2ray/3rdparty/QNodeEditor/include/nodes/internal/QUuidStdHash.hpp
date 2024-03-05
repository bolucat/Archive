#pragma once
#include <QtCore/QUuid>
#include <QtCore/QVariant>
#include <functional>
namespace std
{
    template<>
    struct hash<QUuid>
    {
        inline std::size_t operator()(QUuid const &uid) const
        {
            return qHash(uid);
        }
    };
} // namespace std
