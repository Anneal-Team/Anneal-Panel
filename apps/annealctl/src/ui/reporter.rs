use std::{collections::BTreeSet, io::IsTerminal};

use dialoguer::console::{Term, style};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Line, Modifier, Span, Style},
    widgets::{Gauge, List, ListItem, Paragraph, Wrap},
};

use crate::{
    config::{InstallConfig, InstallRole},
    i18n::Translator,
    state::{InstallState, InstallStep, StepStatus},
};

use super::tui::{
    ACCENT, DANGER, MUTED, PANEL_ALT, TEXT, TuiSession, WARNING, brand, card, footer, frame_layout,
    muted_card, page_background, split_main,
};

pub trait InstallReporter {
    fn start(&mut self, _config: &InstallConfig, _state: &InstallState) {}

    fn step_started(&mut self, _step: InstallStep, _detail: &str) {}

    fn step_completed(&mut self, _step: InstallStep, _detail: &str) {}

    fn step_failed(&mut self, _step: InstallStep, _detail: &str) {}

    fn finish(&mut self, _config: &InstallConfig, _state: &InstallState) {}
}

pub struct NullInstallReporter;

impl InstallReporter for NullInstallReporter {}

pub struct LineInstallReporter {
    translator: Translator,
    active_steps: BTreeSet<InstallStep>,
    completed_steps: BTreeSet<InstallStep>,
    total_steps: usize,
}

impl LineInstallReporter {
    pub fn new(translator: Translator, role: InstallRole, state: &InstallState) -> Self {
        let active_steps = planned_steps(role).into_iter().collect::<BTreeSet<_>>();
        let completed_steps = state
            .steps
            .iter()
            .filter_map(|(step, step_state)| {
                (step_state.status == StepStatus::Completed).then_some(*step)
            })
            .collect::<BTreeSet<_>>();
        Self {
            translator,
            total_steps: active_steps.len(),
            active_steps,
            completed_steps,
        }
    }

    fn render_prefix(&self) -> String {
        format!(
            "[{}/{}]",
            self.completed_steps.len().min(self.total_steps),
            self.total_steps
        )
    }
}

impl InstallReporter for LineInstallReporter {
    fn step_started(&mut self, step: InstallStep, _detail: &str) {
        if !self.active_steps.contains(&step) {
            return;
        }
        println!(
            "{} {} {}",
            self.render_prefix(),
            self.translator.progress_prefix(),
            self.translator.install_step(step)
        );
    }

    fn step_completed(&mut self, step: InstallStep, _detail: &str) {
        if !self.active_steps.contains(&step) {
            return;
        }
        self.completed_steps.insert(step);
    }

    fn step_failed(&mut self, step: InstallStep, detail: &str) {
        if !self.active_steps.contains(&step) {
            return;
        }
        eprintln!(
            "{} {}: {}",
            style(self.translator.failed()).red().bold(),
            self.translator.install_step(step),
            detail
        );
    }

    fn finish(&mut self, _config: &InstallConfig, _state: &InstallState) {
        println!("{}", self.translator.completed());
    }
}

pub struct TerminalInstallReporter {
    translator: Translator,
    active_steps: Vec<InstallStep>,
    completed_steps: BTreeSet<InstallStep>,
    current_step: Option<InstallStep>,
    current_detail: String,
    summary: String,
    failed: Option<(InstallStep, String)>,
    term: Term,
    session: Option<TuiSession>,
}

impl TerminalInstallReporter {
    pub fn new(translator: Translator, role: InstallRole, state: &InstallState) -> Self {
        let active_steps = planned_steps(role);
        let completed_steps = state
            .steps
            .iter()
            .filter_map(|(step, step_state)| {
                (step_state.status == StepStatus::Completed).then_some(*step)
            })
            .collect::<BTreeSet<_>>();
        Self {
            translator,
            active_steps,
            completed_steps,
            current_step: None,
            current_detail: String::new(),
            summary: String::new(),
            failed: None,
            term: Term::stderr(),
            session: None,
        }
    }

    fn progress_ratio(&self) -> f64 {
        if self.active_steps.is_empty() {
            return 0.0;
        }
        self.completed_steps.len() as f64 / self.active_steps.len() as f64
    }

    fn draw(&mut self) {
        let translator = self.translator;
        let active_steps = self.active_steps.clone();
        let completed_steps = self.completed_steps.clone();
        let current_step = self.current_step;
        let current_detail = self.current_detail.clone();
        let summary = self.summary.clone();
        let failed = self.failed.clone();
        let ratio = self.progress_ratio();
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let _ = session.draw(|frame| {
            let area = frame.area();
            frame.render_widget(page_background(), area);
            let [header_area, main_area, status_area, footer_area] = frame_layout(area);
            let header = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(28), Constraint::Min(30)])
                .split(header_area);
            frame.render_widget(brand(), header[0]);

            let title = match failed.as_ref() {
                Some((step, _)) => translator.install_step(*step),
                None => current_step
                    .map(|step| translator.install_step(step))
                    .unwrap_or(translator.progress_prefix()),
            };
            let hero = Paragraph::new(vec![
                Line::from(Span::styled(
                    format!("{}/{}", completed_steps.len(), active_steps.len()),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    title,
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    current_detail.clone(),
                    Style::default().fg(MUTED),
                )),
            ])
            .block(card("Install Progress"));
            frame.render_widget(hero, header[1]);

            let [sidebar_area, content_area] = split_main(main_area);
            let items = active_steps
                .iter()
                .map(|step| {
                    let style = if completed_steps.contains(step) {
                        Style::default().fg(TEXT)
                    } else if Some(*step) == current_step {
                        Style::default()
                            .fg(ACCENT)
                            .bg(PANEL_ALT)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(MUTED)
                    };
                    let icon = if completed_steps.contains(step) {
                        "●"
                    } else if Some(*step) == current_step {
                        "▶"
                    } else {
                        "○"
                    };
                    ListItem::new(Line::from(Span::styled(
                        format!("{icon} {}", translator.install_step(*step)),
                        style,
                    )))
                })
                .collect::<Vec<_>>();
            frame.render_widget(List::new(items).block(muted_card("Steps")), sidebar_area);

            let content = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(5), Constraint::Min(10)])
                .split(content_area);
            let gauge = Gauge::default()
                .block(card("Progress"))
                .gauge_style(Style::default().fg(ACCENT).bg(PANEL_ALT))
                .ratio(ratio)
                .label(format!("{:.0}%", ratio * 100.0));
            frame.render_widget(gauge, content[0]);
            let summary_widget = Paragraph::new(summary)
                .block(muted_card("Summary"))
                .wrap(Wrap { trim: false });
            frame.render_widget(summary_widget, content[1]);

            let (status_title, status_color, status_body) = match failed.as_ref() {
                Some((step, detail)) => (
                    translator.failed(),
                    DANGER,
                    format!("{}: {detail}", translator.install_step(*step)),
                ),
                None => (
                    translator.progress_prefix(),
                    WARNING,
                    current_step
                        .map(|step| translator.install_step(step).to_owned())
                        .unwrap_or_else(|| translator.progress_prefix().to_owned()),
                ),
            };
            let status = Paragraph::new(status_body)
                .style(Style::default().fg(status_color))
                .block(muted_card(status_title))
                .wrap(Wrap { trim: false });
            frame.render_widget(status, status_area);
            frame.render_widget(
                footer("installer active  ctrl+c interrupts current process"),
                footer_area,
            );
        });
    }

    fn restore(&mut self) {
        if let Some(session) = self.session.as_mut() {
            let _ = session.restore();
        }
        self.session = None;
    }

    fn summary_text(config: &InstallConfig, translator: Translator) -> String {
        let mut lines = vec![
            format!(
                "{}: {}",
                translator.role_label(),
                translator.install_role(config.role)
            ),
            format!(
                "{}: {}",
                translator.mode_label(),
                translator.deployment_mode(config.deployment_mode)
            ),
        ];
        if let Some(control_plane) = config.control_plane.as_ref() {
            lines.push(format!(
                "{}: {}",
                translator.public_url_label(),
                control_plane.public_base_url
            ));
            lines.push(format!(
                "{}: {}",
                translator.admin_email_label(),
                control_plane.superadmin.email
            ));
        }
        if let Some(node) = config.node.as_ref() {
            lines.push(format!("{}: {}", translator.node_label(), node.name));
        }
        lines.join("\n")
    }
}

impl InstallReporter for TerminalInstallReporter {
    fn start(&mut self, config: &InstallConfig, _state: &InstallState) {
        self.summary = Self::summary_text(config, self.translator);
        self.session = TuiSession::new().ok();
        self.draw();
    }

    fn step_started(&mut self, step: InstallStep, detail: &str) {
        self.current_step = Some(step);
        self.current_detail = detail.to_owned();
        self.draw();
    }

    fn step_completed(&mut self, step: InstallStep, detail: &str) {
        self.completed_steps.insert(step);
        self.current_step = Some(step);
        self.current_detail = detail.to_owned();
        self.draw();
    }

    fn step_failed(&mut self, step: InstallStep, detail: &str) {
        self.failed = Some((step, detail.to_owned()));
        self.current_step = Some(step);
        self.current_detail = detail.to_owned();
        self.draw();
        self.restore();
        let _ = self.term.write_line(&format!(
            "{} {}: {}",
            style(self.translator.failed()).red().bold(),
            self.translator.install_step(step),
            detail
        ));
    }

    fn finish(&mut self, _config: &InstallConfig, _state: &InstallState) {
        self.draw();
        self.restore();
        let _ = self.term.write_line(self.translator.completed());
    }
}

pub fn make_reporter(
    translator: Translator,
    role: InstallRole,
    state: &InstallState,
) -> Box<dyn InstallReporter> {
    if std::io::stderr().is_terminal() {
        return Box::new(TerminalInstallReporter::new(translator, role, state));
    }
    Box::new(LineInstallReporter::new(translator, role, state))
}

pub fn planned_steps(role: InstallRole) -> Vec<InstallStep> {
    let mut steps = vec![
        InstallStep::Prepare,
        InstallStep::Packages,
        InstallStep::Files,
        InstallStep::Services,
    ];
    if role.includes_control_plane() {
        steps.push(InstallStep::ControlPlaneBootstrap);
    }
    if role.includes_node() {
        steps.push(InstallStep::NodeBootstrap);
    }
    if role == InstallRole::AllInOne {
        steps.push(InstallStep::StarterSubscription);
    }
    steps.push(InstallStep::Summary);
    steps.push(InstallStep::Cleanup);
    steps
}

#[cfg(test)]
mod tests {
    use crate::config::InstallRole;

    use super::*;

    #[test]
    fn planned_steps_match_roles() {
        assert_eq!(planned_steps(InstallRole::ControlPlane).len(), 7);
        assert_eq!(planned_steps(InstallRole::Node).len(), 7);
        assert_eq!(planned_steps(InstallRole::AllInOne).len(), 9);
    }

    #[test]
    fn terminal_reporter_keeps_completed_steps_from_state() {
        let state = InstallState::load_or_new(
            std::path::Path::new("unused"),
            InstallRole::AllInOne,
            crate::config::DeploymentMode::Native,
        )
        .expect("state");
        let reporter = TerminalInstallReporter::new(
            Translator::with_unicode(crate::i18n::Language::Ru, true),
            InstallRole::AllInOne,
            &state,
        );

        assert_eq!(reporter.active_steps.len(), 9);
        assert!(reporter.completed_steps.is_empty());
    }
}
