use anyhow::{Context, Result};
use delay_timer::prelude::TaskBuilder;
use tauri::{Listener, Manager};

use crate::{
    config::Config,
    core::{handle, timer::Timer},
    log_err,
};

const LIGHT_WEIGHT_TASK_UID: &str = "light_weight_task";

pub fn enable_auto_light_weight_mode() {
    println!("[lightweight_mode] 开启自动轻量模式");
    log::info!(target: "app", "[lightweight_mode] 开启自动轻量模式");
    setup_window_close_listener();
    setup_webview_focus_listener();
}

pub fn disable_auto_light_weight_mode() {
    println!("[lightweight_mode] 关闭自动轻量模式");
    log::info!(target: "app", "[lightweight_mode] 关闭自动轻量模式");
    let _ = cancel_light_weight_timer();
    cancel_window_close_listener();
}

pub fn entry_lightweight_mode() {
    if let Some(window) = handle::Handle::global().get_window() {
        if let Some(webview) = window.get_webview_window("main") {
            let _ = webview.destroy();
            let _ = window.hide();
            println!("[lightweight_mode] 轻量模式已开启");
            log::info!(target: "app", "[lightweight_mode] 轻量模式已开启");
        }
    }
    let _ = cancel_light_weight_timer();
}

fn setup_window_close_listener() -> u32 {
    if let Some(window) = handle::Handle::global().get_window() {
        let handler = window.listen("tauri://close-requested", move |_event| {
            let _ = setup_light_weight_timer();
            println!("[lightweight_mode] 监听到关闭请求，开始轻量模式计时");
            log::info!(target: "app", "[lightweight_mode] 监听到关闭请求，开始轻量模式计时");
        });
        return handler;
    }
    0
}

fn setup_webview_focus_listener() -> u32 {
    if let Some(window) = handle::Handle::global().get_window() {
        let handler = window.listen("tauri://focus", move |_event| {
            log_err!(cancel_light_weight_timer());
            println!("[lightweight_mode] 监听到窗口获得焦点，取消轻量模式计时");
            log::info!(target: "app", "[lightweight_mode] 监听到窗口获得焦点，取消轻量模式计时");
        });
        return handler;
    }
    0
}

fn cancel_window_close_listener() {
    if let Some(window) = handle::Handle::global().get_window() {
        window.unlisten(setup_window_close_listener());
        println!("[lightweight_mode] 取消了窗口关闭监听");
        log::info!(target: "app", "[lightweight_mode] 取消了窗口关闭监听");
    }
}

fn setup_light_weight_timer() -> Result<()> {
    Timer::global().init()?;

    let mut timer_map = Timer::global().timer_map.write();
    let delay_timer = Timer::global().delay_timer.write();
    let mut timer_count = Timer::global().timer_count.lock();

    let task_id = *timer_count;
    *timer_count += 1;

    let once_by_minutes = Config::verge()
        .latest()
        .auto_light_weight_minutes
        .clone()
        .unwrap_or(10);

    let task = TaskBuilder::default()
        .set_task_id(task_id)
        .set_maximum_parallel_runnable_num(1)
        .set_frequency_once_by_minutes(once_by_minutes)
        .spawn_async_routine(move || async move {
            println!("[lightweight_mode] 计时器到期，开始进入轻量模式");
            log::info!(target: "app",
                "[lightweight_mode] 计时器到期，开始进入轻量模式"
            );
            entry_lightweight_mode();
        })
        .context("failed to create light weight timer task")?;

    delay_timer
        .add_task(task)
        .context("failed to add light weight timer task")?;

    let timer_task = crate::core::timer::TimerTask {
        task_id,
        interval_minutes: once_by_minutes,
        last_run: chrono::Local::now().timestamp(),
    };

    timer_map.insert(LIGHT_WEIGHT_TASK_UID.to_string(), timer_task);

    println!(
        "[lightweight_mode] 轻量模式计时器已设置，{} 分钟后将自动进入轻量模式",
        once_by_minutes
    );
    log::info!(target: "app",
        "[lightweight_mode] 轻量模式计时器已设置，{} 分钟后将自动进入轻量模式",
        once_by_minutes
    );

    Ok(())
}

fn cancel_light_weight_timer() -> Result<()> {
    let mut timer_map = Timer::global().timer_map.write();
    let delay_timer = Timer::global().delay_timer.write();

    if let Some(task) = timer_map.remove(LIGHT_WEIGHT_TASK_UID) {
        delay_timer
            .remove_task(task.task_id)
            .context("failed to remove light weight timer task")?;
        println!("[lightweight_mode] 轻量模式计时器已取消");
        log::info!(target: "app", "[lightweight_mode] 轻量模式计时器已取消");
    }

    Ok(())
}
