#include <gtest/gtest.h>
#include <uvw/idle.h>

TEST(Idle, StartAndStop) {
    auto loop = uvw::Loop::getDefault();
    auto handle = loop->resource<uvw::IdleHandle>();

    bool checkIdleEvent = false;

    handle->on<uvw::ErrorEvent>([](auto &&...) { FAIL(); });

    handle->on<uvw::IdleEvent>([&checkIdleEvent](const auto &, auto &hndl) {
        ASSERT_FALSE(checkIdleEvent);
        checkIdleEvent = true;
        hndl.stop();
        hndl.close();
        ASSERT_TRUE(hndl.closing());
    });

    handle->start();

    ASSERT_TRUE(handle->active());
    ASSERT_FALSE(handle->closing());

    loop->run();

    ASSERT_TRUE(checkIdleEvent);
}

TEST(Idle, Fake) {
    auto loop = uvw::Loop::getDefault();
    auto handle = loop->resource<uvw::IdleHandle>();

    handle->on<uvw::ErrorEvent>([](auto &&...) { FAIL(); });
    handle->on<uvw::IdleEvent>([](auto &&...) { FAIL(); });

    handle->start();
    handle->close();

    ASSERT_FALSE(handle->active());
    ASSERT_TRUE(handle->closing());

    loop->run();
}
