#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod metrics;
mod sources;

use std::sync::mpsc;
use std::time::Duration;

use chrono::Local;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use metrics::{bar, FetchError, Metric, MetricSource};

const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// Register metric sources here. Order determines menu order.
fn sources() -> Vec<Box<dyn MetricSource>> {
    vec![Box::new(sources::plan_usage::PlanUsage)]
}

type SourceResult = (String, Result<Vec<Metric>, FetchError>);

enum AppEvent {
    Update(Vec<SourceResult>),
    Menu(MenuEvent),
}

fn main() {
    let mut event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

    // Menu bar accessory: no Dock icon, no app menu.
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    let menu_proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e| {
        let _ = menu_proxy.send_event(AppEvent::Menu(e));
    }));

    // Background fetcher: polls every REFRESH_INTERVAL, or immediately when
    // poked through `refresh_tx` ("Refresh now" menu item).
    let fetch_proxy = event_loop.create_proxy();
    let (refresh_tx, refresh_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let sources = sources();
        loop {
            let results: Vec<SourceResult> = sources
                .iter()
                .map(|s| (s.name().to_string(), s.fetch()))
                .collect();
            if fetch_proxy.send_event(AppEvent::Update(results)).is_err() {
                return; // event loop is gone
            }
            let _ = refresh_rx.recv_timeout(REFRESH_INTERVAL);
        }
    });

    let mut tray: Option<TrayIcon> = None;
    let refresh_id = MenuId::new("refresh");
    let quit_id = MenuId::new("quit");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            // On macOS the tray must be created after the event loop starts.
            Event::NewEvents(StartCause::Init) => {
                tray = Some(
                    TrayIconBuilder::new()
                        .with_title("… %")
                        .build()
                        .expect("failed to create menu bar item"),
                );
            }
            Event::UserEvent(AppEvent::Update(results)) => {
                // A rate-limited fetch means the data is stale, not wrong:
                // keep the previous title/menu instead of flashing an error.
                let rate_limited = results
                    .iter()
                    .any(|(_, r)| matches!(r, Err(FetchError::RateLimited)));
                if let Some(tray) = &tray {
                    if !rate_limited {
                        tray.set_title(Some(title_for(&results)));
                        tray.set_menu(Some(Box::new(build_menu(
                            &results,
                            &refresh_id,
                            &quit_id,
                        ))));
                    }
                }
            }
            Event::UserEvent(AppEvent::Menu(e)) => {
                if *e.id() == quit_id {
                    *control_flow = ControlFlow::Exit;
                } else if *e.id() == refresh_id {
                    let _ = refresh_tx.send(());
                }
            }
            _ => {}
        }
    });
}

/// Compact menu bar title from the emphasized metric, e.g. "▓▓░░░ 34%".
fn title_for(results: &[SourceResult]) -> String {
    let emphasized = results
        .iter()
        .filter_map(|(_, r)| r.as_ref().ok())
        .flatten()
        .filter(|m| m.emphasized)
        .max_by(|a, b| a.percent.total_cmp(&b.percent));
    match emphasized {
        Some(m) => format!("{} {:.0}%", bar(m.percent, 5), m.percent),
        None => "⚠ Claude".to_string(),
    }
}

fn build_menu(results: &[SourceResult], refresh_id: &MenuId, quit_id: &MenuId) -> Menu {
    let menu = Menu::new();
    for (name, result) in results {
        let _ = menu.append(&MenuItem::new(name, false, None));
        match result {
            Ok(metrics) => {
                for m in metrics {
                    let _ = menu.append(&MenuItem::new(item_text(m), false, None));
                }
            }
            Err(err) => {
                let _ = menu.append(&MenuItem::new(format!("⚠ {err}"), false, None));
            }
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    let _ = menu.append(&MenuItem::new(
        format!("Updated {}", Local::now().format("%-I:%M %p")),
        false,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(refresh_id.clone(), "Refresh Now", true, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(quit_id.clone(), "Quit Claude Monitor", true, None));
    menu
}

fn item_text(m: &Metric) -> String {
    let mut text = format!("{}  {:>3.0}%   {}", bar(m.percent, 10), m.percent, m.label);
    if let Some(resets) = m.resets_at {
        let now = Local::now();
        let fmt = if resets.date_naive() == now.date_naive() {
            "%-I:%M %p"
        } else {
            "%a %-I:%M %p"
        };
        text.push_str(&format!(" · resets {}", resets.format(fmt)));
    }
    text
}
