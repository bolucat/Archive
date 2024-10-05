// SPDX-FileCopyrightText: Copyright 2020 yuzu Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later

#pragma once

#include <atomic>
#include <chrono>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <thread>

#include <boost/heap/fibonacci_heap.hpp>

#include "common/common_types.h"
#include "common/thread.h"
#include "common/wall_clock.h"

namespace Core::Timing {

/// A callback that may be scheduled for a particular core timing event.
using TimedCallback = std::function<std::optional<std::chrono::nanoseconds>(
    s64 time, std::chrono::nanoseconds ns_late)>;

/// Contains the characteristics of a particular event.
struct EventType {
    explicit EventType(TimedCallback&& callback_, std::string&& name_)
        : callback{std::move(callback_)}, name{std::move(name_)}, sequence_number{0} {}

    /// The event's callback function.
    TimedCallback callback;
    /// A pointer to the name of the event.
    const std::string name;
    /// A monotonic sequence number, incremented when this event is
    /// changed externally.
    size_t sequence_number;
};

enum class UnscheduleEventType {
    Wait,
    NoWait,
};

/**
 * This is a system to schedule events into the emulated machine's future. Time is measured
 * in main CPU clock cycles.
 *
 * To schedule an event, you first have to register its type. This is where you pass in the
 * callback. You then schedule events using the type ID you get back.
 *
 * The s64 ns_late that the callbacks get is how many ns late it was.
 * So to schedule a new event on a regular basis:
 * inside callback:
 *   ScheduleEvent(period_in_ns - ns_late, callback, "whatever")
 */
class CoreTiming {
public:
    CoreTiming();
    ~CoreTiming();

    CoreTiming(const CoreTiming&) = delete;
    CoreTiming(CoreTiming&&) = delete;

    CoreTiming& operator=(const CoreTiming&) = delete;
    CoreTiming& operator=(CoreTiming&&) = delete;

    /// CoreTiming begins at the boundary of timing slice -1. An initial call to Advance() is
    /// required to end slice - 1 and start slice 0 before the first cycle of code is executed.
    void Initialize(std::function<void()>&& on_thread_init_);

    /// Clear all pending events. This should ONLY be done on exit.
    void ClearPendingEvents();

    /// Sets if emulation is multicore or single core, must be set before Initialize
    void SetMulticore(bool is_multicore_) {
        is_multicore = is_multicore_;
    }

    /// Pauses/Unpauses the execution of the timer thread.
    void Pause(bool is_paused);

    /// Pauses/Unpauses the execution of the timer thread and waits until paused.
    void SyncPause(bool is_paused);

    /// Checks if core timing is running.
    bool IsRunning() const;

    /// Checks if the timer thread has started.
    bool HasStarted() const {
        return has_started;
    }

    /// Checks if there are any pending time events.
    bool HasPendingEvents() const;

    /// Schedules an event in core timing
    void ScheduleEvent(std::chrono::nanoseconds ns_into_future,
                       const std::shared_ptr<EventType>& event_type, bool absolute_time = false);

    /// Schedules an event which will automatically re-schedule itself with the given time, until
    /// unscheduled
    void ScheduleLoopingEvent(std::chrono::nanoseconds start_time,
                              std::chrono::nanoseconds resched_time,
                              const std::shared_ptr<EventType>& event_type,
                              bool absolute_time = false);

    void UnscheduleEvent(const std::shared_ptr<EventType>& event_type,
                         UnscheduleEventType type = UnscheduleEventType::Wait);

    void AddTicks(u64 ticks_to_add);

    void ResetTicks();

    void Idle();

    s64 GetDowncount() const {
        return downcount;
    }

    /// Returns the current CNTPCT tick value.
    u64 GetClockTicks() const;

    /// Returns the current GPU tick value.
    u64 GetGPUTicks() const;

    /// Returns current time in microseconds.
    std::chrono::microseconds GetGlobalTimeUs() const;

    /// Returns current time in nanoseconds.
    std::chrono::nanoseconds GetGlobalTimeNs() const;

    /// Checks for events manually and returns time in nanoseconds for next event, threadsafe.
    std::optional<s64> Advance();

#ifdef _WIN32
    void SetTimerResolutionNs(std::chrono::nanoseconds ns);
#endif

private:
    struct Event;

    static void ThreadEntry(CoreTiming& instance);
    void ThreadLoop();

    void Reset();

    std::unique_ptr<Common::WallClock> clock;

    s64 global_timer = 0;

#ifdef _WIN32
    s64 timer_resolution_ns;
#endif

    using heap_t =
        boost::heap::fibonacci_heap<CoreTiming::Event, boost::heap::compare<std::greater<>>>;

    heap_t event_queue;
    u64 event_fifo_id = 0;

    Common::Event event{};
    Common::Event pause_event{};
    mutable std::mutex basic_lock;
    std::mutex advance_lock;
    std::unique_ptr<std::jthread> timer_thread;
    std::atomic<bool> paused{};
    std::atomic<bool> paused_set{};
    std::atomic<bool> wait_set{};
    std::atomic<bool> shutting_down{};
    std::atomic<bool> has_started{};
    std::function<void()> on_thread_init{};

    bool is_multicore{};
    s64 pause_end_time{};

    /// Cycle timing
    u64 cpu_ticks{};
    s64 downcount{};
};

/// Creates a core timing event with the given name and callback.
///
/// @param name     The name of the core timing event to create.
/// @param callback The callback to execute for the event.
///
/// @returns An EventType instance representing the created event.
///
std::shared_ptr<EventType> CreateEvent(std::string name, TimedCallback&& callback);

} // namespace Core::Timing
