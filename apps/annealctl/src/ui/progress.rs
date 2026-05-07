use std::{
    collections::VecDeque,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Line, Modifier, Span, Style},
    widgets::{Gauge, List, ListItem, Paragraph, Wrap},
};

use crate::{
    config::InstallConfig,
    state::{InstallState, InstallStep},
};

use super::tui::{
    ACCENT, DANGER, MUTED, PANEL_ALT, TEXT, TuiSession, WARNING, brand, card, footer, frame_layout,
    muted_card, page_background, split_main,
};

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Started {
        domain: String,
        panel_url: String,
    },
    StepStarted {
        step: InstallStep,
        detail: String,
    },
    StepCompleted {
        step: InstallStep,
        detail: String,
    },
    StepFailed {
        step: InstallStep,
        detail: String,
    },
    Log(String),
    Finished {
        panel_url: String,
        summary_path: String,
    },
}

pub struct InstallProgress {
    sender: Sender<ProgressEvent>,
    handle: Option<thread::JoinHandle<()>>,
}

impl InstallProgress {
    pub fn start(config: &InstallConfig, state: &InstallState, summary_path: String) -> Self {
        let (sender, receiver) = mpsc::channel();
        let initial = ProgressSnapshot {
            steps: planned_steps(),
            completed: state
                .steps
                .iter()
                .filter_map(|(step, state)| state.status.is_completed().then_some(*step))
                .collect(),
            current_step: None,
            current_detail: String::new(),
            domain: config.control_plane.domain.clone(),
            panel_url: config.control_plane.public_base_url.clone(),
            summary_path,
            logs: VecDeque::new(),
            done: false,
            failed: false,
        };
        let handle = thread::spawn(move || {
            let Ok(mut session) = TuiSession::new() else {
                while let Ok(event) = receiver.recv() {
                    if matches!(event, ProgressEvent::Finished { .. }) {
                        break;
                    }
                }
                return;
            };
            let mut snapshot = initial;
            loop {
                match receiver.recv_timeout(Duration::from_millis(120)) {
                    Ok(event) => {
                        let should_stop = snapshot.apply(event);
                        let _ = session.draw(|frame| snapshot.render(frame));
                        if should_stop {
                            thread::sleep(Duration::from_millis(900));
                            let _ = session.restore();
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        let _ = session.draw(|frame| snapshot.render(frame));
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        let _ = session.restore();
                        break;
                    }
                }
            }
        });
        Self {
            sender,
            handle: Some(handle),
        }
    }

    pub fn sender(&self) -> Sender<ProgressEvent> {
        self.sender.clone()
    }

    pub fn send(&self, event: ProgressEvent) {
        let _ = self.sender.send(event);
    }

    pub fn finish(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct ProgressSnapshot {
    steps: Vec<InstallStep>,
    completed: Vec<InstallStep>,
    current_step: Option<InstallStep>,
    current_detail: String,
    domain: String,
    panel_url: String,
    summary_path: String,
    logs: VecDeque<String>,
    done: bool,
    failed: bool,
}

impl ProgressSnapshot {
    fn apply(&mut self, event: ProgressEvent) -> bool {
        match event {
            ProgressEvent::Started { domain, panel_url } => {
                self.domain = domain;
                self.panel_url = panel_url;
                self.push_log("installation started");
            }
            ProgressEvent::StepStarted { step, detail } => {
                self.current_step = Some(step);
                self.current_detail = detail.clone();
                self.push_log(format!("-> {}: {detail}", step_label(step)));
            }
            ProgressEvent::StepCompleted { step, detail } => {
                if !self.completed.contains(&step) {
                    self.completed.push(step);
                }
                self.current_step = Some(step);
                self.current_detail = detail.clone();
                self.push_log(format!("OK {}: {detail}", step_label(step)));
            }
            ProgressEvent::StepFailed { step, detail } => {
                self.current_step = Some(step);
                self.current_detail = detail.clone();
                self.failed = true;
                self.done = true;
                self.push_log(format!("FAIL {}: {detail}", step_label(step)));
            }
            ProgressEvent::Log(line) => self.push_log(line),
            ProgressEvent::Finished {
                panel_url,
                summary_path,
            } => {
                self.panel_url = panel_url;
                self.summary_path = summary_path;
                self.done = true;
                self.push_log("installation completed");
            }
        }
        self.done
    }

    fn render(&self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        frame.render_widget(page_background(), area);
        let [header_area, main_area, status_area, footer_area] = frame_layout(area);
        let header = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(30)])
            .split(header_area);
        frame.render_widget(brand(), header[0]);

        let title = if self.failed {
            "Installation failed"
        } else if self.done {
            "Installation completed"
        } else {
            self.current_step.map(step_label).unwrap_or("Installing")
        };
        let hero = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{}/{}", self.completed.len(), self.steps.len()),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                title,
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                self.current_detail.clone(),
                Style::default().fg(MUTED),
            )),
        ])
        .block(card("Install Progress"));
        frame.render_widget(hero, header[1]);

        let [sidebar_area, content_area] = split_main(main_area);
        self.render_steps(frame, sidebar_area);
        self.render_content(frame, content_area);

        let status_color = if self.failed { DANGER } else { WARNING };
        let status_text = if self.done {
            format!(
                "Panel: {} | Credentials: {}",
                self.panel_url, self.summary_path
            )
        } else {
            format!("Domain: {} | Panel: {}", self.domain, self.panel_url)
        };
        frame.render_widget(
            Paragraph::new(status_text)
                .style(Style::default().fg(status_color))
                .block(muted_card("Status"))
                .wrap(Wrap { trim: false }),
            status_area,
        );
        frame.render_widget(
            footer("installer active  command output is streamed below"),
            footer_area,
        );
    }

    fn render_steps(&self, frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let items = self
            .steps
            .iter()
            .map(|step| {
                let style = if self.completed.contains(step) {
                    Style::default().fg(TEXT)
                } else if Some(*step) == self.current_step {
                    Style::default()
                        .fg(ACCENT)
                        .bg(PANEL_ALT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(MUTED)
                };
                let marker = if self.completed.contains(step) {
                    "*"
                } else if Some(*step) == self.current_step {
                    ">"
                } else {
                    "-"
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{marker} {}", step_label(*step)),
                    style,
                )))
            })
            .collect::<Vec<_>>();
        frame.render_widget(List::new(items).block(muted_card("Steps")), area);
    }

    fn render_content(&self, frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(10)])
            .split(area);
        let ratio = if self.steps.is_empty() {
            0.0
        } else {
            self.completed.len() as f64 / self.steps.len() as f64
        };
        frame.render_widget(
            Gauge::default()
                .block(card("Progress"))
                .gauge_style(Style::default().fg(ACCENT).bg(PANEL_ALT))
                .ratio(ratio)
                .label(format!("{:.0}%", ratio * 100.0)),
            content[0],
        );
        let lines = self.logs.iter().cloned().collect::<Vec<_>>().join("\n");
        frame.render_widget(
            Paragraph::new(lines)
                .block(muted_card("Live log"))
                .wrap(Wrap { trim: false }),
            content[1],
        );
    }

    fn push_log(&mut self, line: impl Into<String>) {
        self.logs.push_back(line.into());
        while self.logs.len() > 120 {
            self.logs.pop_front();
        }
    }
}

fn planned_steps() -> Vec<InstallStep> {
    vec![
        InstallStep::Prepare,
        InstallStep::Packages,
        InstallStep::Files,
        InstallStep::Services,
        InstallStep::ControlPlaneBootstrap,
        InstallStep::StarterSubscription,
        InstallStep::Summary,
        InstallStep::Cleanup,
    ]
}

pub fn step_label(step: InstallStep) -> &'static str {
    match step {
        InstallStep::Prepare => "prepare",
        InstallStep::Packages => "packages",
        InstallStep::Files => "files",
        InstallStep::Services => "services",
        InstallStep::ControlPlaneBootstrap => "control-plane bootstrap",
        InstallStep::StarterSubscription => "starter subscription",
        InstallStep::Summary => "summary",
        InstallStep::Cleanup => "cleanup",
    }
}

trait StepStatusExt {
    fn is_completed(&self) -> bool;
}

impl StepStatusExt for crate::state::StepStatus {
    fn is_completed(&self) -> bool {
        *self == crate::state::StepStatus::Completed
    }
}
