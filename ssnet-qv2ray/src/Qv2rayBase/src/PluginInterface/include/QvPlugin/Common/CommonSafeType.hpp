#pragma once

#include "QvPlugin/Utils/BindableProps.hpp"
#include "QvPlugin/Utils/JsonConversion.hpp"

#include <QHashFunctions>
#include <QJsonObject>
#include <QString>

namespace Qv2rayPlugin::Common::_base_types::safetype
{
    template<typename enum_t, typename tuple_t>
    struct EnumVariantMap : public QMap<enum_t, QVariant>
    {
        template<enum_t f>
        using result_type_t = typename std::tuple_element_t<f, tuple_t>;

        template<enum_t f>
        std::tuple_element_t<f, tuple_t> GetValue() const
        {
            return this->value(f).template value<std::tuple_element_t<f, tuple_t>>();
        };

        template<enum_t f>
        void SetValue(const typename std::tuple_element_t<f, tuple_t> &t)
        {
            this->insert(f, QVariant::fromValue(t));
        };
    };

    using namespace std::chrono;
    template<typename>
    struct SafeJsonType : public QJsonObject
    {
        // clang-format off
        template<class... Args> explicit SafeJsonType(Args... args) : QJsonObject(args...) {};
        const QJsonObject &raw() const { return *this; }
        QJsonObject toJson() const { return *this; }
        void loadJson(const QJsonValue &d) { *this = std::remove_cv_t<std::remove_reference_t<decltype (*this)>> { d.toObject() }; }
        template<typename TTarget> TTarget CopyAs() const { return TTarget(raw()); }
        // clang-format on
        template<typename TTarget>
        TTarget ForceCopyAs() const
        {
            TTarget t;
            JsonStructHelper::Deserialize(t, raw());
            return t;
        }
    };

    template<typename T>
    struct IDType
    {
        // clang-format off
        IDType() : m_id(u"null"_qs){};
        explicit IDType(const QString &id) : m_id(id){};
        ~IDType() = default;
        inline bool operator==(const IDType<T> &rhs) const { return m_id == rhs.m_id; }
        inline bool operator!=(const IDType<T> &rhs) const { return m_id != rhs.m_id; }
        inline const QString toString() const { return m_id; }
        inline bool isNull() const { return m_id == u"null"_qs; }
        inline QJsonValue toJson() const { return m_id; }
        inline void loadJson(const QJsonValue &d) { m_id = d.toString(); }
        // clang-format on

      private:
        QString m_id;
    };

    template<typename T>
    inline size_t qHash(const IDType<T> &key) noexcept
    {
        return ::qHash(key.toString());
    }

    template<typename T>
    inline QDebug operator<<(QDebug debug, const IDType<T> &key)
    {
        return debug << key.toString();
    }
} // namespace Qv2rayPlugin::Common::_base_types::safetype

using namespace Qv2rayPlugin::Common::_base_types::safetype;

#define DeclareSafeJson(CLASS)                                                                                                                                           \
    namespace Qv2rayPlugin::Common::_base_types::safetype                                                                                                                \
    {                                                                                                                                                                    \
        class __##CLASS##__;                                                                                                                                             \
        typedef Qv2rayPlugin::Common::_base_types::safetype::SafeJsonType<__##CLASS##__> CLASS;                                                                          \
    }                                                                                                                                                                    \
    Q_DECLARE_METATYPE(Qv2rayPlugin::Common::_base_types::safetype::CLASS)

#define DeclareSafeID(type)                                                                                                                                              \
    namespace Qv2rayPlugin::Common::_base_types::safetype                                                                                                                \
    {                                                                                                                                                                    \
        class __##type;                                                                                                                                                  \
        typedef Qv2rayPlugin::Common::_base_types::safetype::IDType<__##type> type;                                                                                      \
    }                                                                                                                                                                    \
    Q_DECLARE_METATYPE(Qv2rayPlugin::Common::_base_types::safetype::type)

DeclareSafeJson(IOProtocolSettings);
DeclareSafeJson(IOStreamSettings);
DeclareSafeJson(RuleExtraSettings);
DeclareSafeJson(BalancerSelectorSettings);
DeclareSafeJson(SubscriptionProviderOptions);

DeclareSafeID(GroupId);
DeclareSafeID(ConnectionId);
DeclareSafeID(RoutingId);
DeclareSafeID(PluginId);
DeclareSafeID(KernelId);
DeclareSafeID(LatencyTestEngineId);
DeclareSafeID(SubscriptionProviderId);

#undef DeclareSafeJson
#undef DeclareSafeID
