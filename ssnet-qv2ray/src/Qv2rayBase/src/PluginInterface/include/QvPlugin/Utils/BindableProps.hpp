#pragma once

#include <QJsonValue>
#include <QObject>
#include <QVariant>

class INotifiable : public QObject
{
    Q_OBJECT
  public:
    explicit INotifiable() : QObject(){};
    explicit INotifiable(INotifiable &) : QObject(){};
    Q_SIGNAL void notify();
};

template<typename T>
struct Bindable : public INotifiable
{
  public:
    typedef T value_type;
    Bindable(const T &def = T{}) : value(def), defaultvalue(def){};
    Bindable(const Bindable<T> &another) : value(another.value), defaultvalue(another.defaultvalue){};

    bool isDefault() const
    {
        return value == defaultvalue;
    }

    // clang-format off
    const T* operator->() const { return &value; }
          T* operator->()       { return &value; }

    const T& operator*() const { return value; }
          T& operator*()       { return value; }

    operator const T()   const { return value; }
    operator       T()         { return value; }

    T & operator=(const T& f)           { return set(f); }
    T & operator=(const T&&f)           { return set(std::move(f)); }
    T & operator=(const Bindable<T> &f) { return set(f.value); }

    Bindable<T> &operator++() { value++; return *this; }
    Bindable<T> &operator--() { value--; return *this; } 

    template<typename Y> void operator<<(const Y &another) { value << another; }
    template<typename Y> void operator>>(const Y &another) { value >> another; }
    template<typename V> T operator+=(const V &v) { value += v; return value; }
    template<typename V> T operator-=(const V &v) { value -= v; return value; }
    template<typename V> T operator*=(const V &v) { value *= v; return value; }
    template<typename V> T operator/=(const V &v) { value /= v; return value; }
    template<typename V> T operator&=(const V &v) { value &= v; return value; }
    template<typename V> T operator%=(const V &v) { value %= v; return value; }
    template<typename V> T operator|=(const V &v) { value |= v; return value; }
    // clang-format on

    // clang-format off
    bool operator==(const T& val) const { return val == value ; }
    bool operator!=(const T& val) const { return val != value ; }
    bool operator==(const Bindable<T>& left) const { return   left.value == value ; }
    bool operator!=(const Bindable<T>& left) const { return !(left.value == value); }
    // clang-format on

  public:
    void EmitNotify()
    {
        emit notify();
    }

    template<typename TCallback>
    inline void Observe(TCallback callback) const
    {
        static_assert(std::is_invocable<TCallback, const T &>::value, "Callback function must be callable with a const reference parameter T");
        QObject::connect(this, &INotifiable::notify, [this, callback] { callback(value); });
        callback(value);
    }

    inline void WriteBind(Bindable<T> *propTarget)
    {
        propTarget->set(value);
        QObject::connect(this, &INotifiable::notify, [this, propTarget]() { propTarget->set(value); });
    }

    inline void ReadBind(const Bindable<T> *target)
    {
        QObject::connect(target, &INotifiable::notify, [this, target]() { set(target->value); });
    }

    inline void ReadWriteBind(Bindable<T> *target)
    {
        WriteBind(target);
        ReadBind(target);
    }

    ///
    /// \brief Change the value of target property when this value has changed.
    ///
    template<typename TTarget>
    inline void WriteBind(TTarget *target, const char *target_prop)
    {
        static_assert(std::is_base_of_v<QObject, TTarget>, "Wrong Usage: Target must be a QObject");
        const auto conv = QMetaType::canConvert(QMetaType(qMetaTypeId<T>()), ((QObject *) target)->property(target_prop).metaType());
        Q_ASSERT_X(conv, "WriteBind", "ID doesn't match.");

        // Firstly, sync target properties.
        ((QObject *) target)->setProperty(target_prop, value);

        QObject::connect(this, &INotifiable::notify,
                         [this, target, target_prop]()
                         {
                             if (auto obj = dynamic_cast<QObject *>(target); obj)
                                 obj->setProperty(target_prop, value);
                         });
    }

    ///
    /// \brief Change the value of current property of something happened in target, triggered by target signal
    ///
    template<typename TTarget, typename Func>
    inline void ReadBind(const TTarget *target, const char *target_prop, Func trigger_signal)
    {
        static_assert(std::is_base_of_v<QObject, TTarget>, "Wrong Usage: Target must be a QObject");
        const auto conv = QMetaType::canConvert(((QObject *) target)->property(target_prop).metaType(), QMetaType(qMetaTypeId<T>()));
        Q_ASSERT_X(conv, "ReadBind", "ID doesn't match.");

        QObject::connect(target, trigger_signal,
                         [this, target, target_prop]()
                         {
                             if (auto obj = dynamic_cast<const QObject *>(target); obj)
                                 set(obj->property(target_prop).value<T>());
                         });
    }

    template<typename TTarget, typename Func>
    inline void ReadWriteBind(TTarget *target, const char *target_prop, Func trigger_signal)
    {
        WriteBind(target, target_prop);
        ReadBind(target, target_prop, trigger_signal);
    }

    const T defaultvalue;

  private:
    T &set(const T &v)
    {
        if (value == v)
            return value;
        value = v;
        emit notify();
        return value;
    }
    T value;
};
