use std::{io::IsTerminal, time::Duration};

use crate::{
    cli::InstallArgs,
    config::{DeploymentMode, InstallConfig, InstallRole},
};
use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Line, Modifier, Span, Style},
    widgets::{List, ListItem, Paragraph, Wrap},
};

use super::tui::{
    ACCENT, BG, DANGER, MUTED, TEXT, TuiSession, WARNING, brand, card, footer as footer_widget,
    frame_layout, muted_card, page_background, split_main,
};

pub fn prepare_install_args(args: InstallArgs) -> Result<InstallArgs> {
    if !should_open_wizard(&args) {
        return Ok(args);
    }
    RatatuiWizard::new(args).run()
}

fn should_open_wizard(args: &InstallArgs) -> bool {
    !args.non_interactive
        && std::io::stdin().is_terminal()
        && std::io::stderr().is_terminal()
        && (args.domain.as_deref().is_none_or(str::is_empty)
            && args.public_base_url.as_deref().is_none_or(str::is_empty))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    Domain,
    SuperadminEmail,
    SuperadminDisplayName,
    ResellerTenantName,
    Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Finish,
    Cancel,
}

struct RatatuiWizard {
    args: InstallArgs,
    prompt_index: usize,
    input: String,
    cursor: usize,
    error: Option<String>,
    last_prompt: Option<Prompt>,
}

impl RatatuiWizard {
    fn new(mut args: InstallArgs) -> Self {
        args.role.get_or_insert(InstallRole::ControlPlane);
        args.deployment_mode.get_or_insert(DeploymentMode::Native);
        Self {
            args,
            prompt_index: 0,
            input: String::new(),
            cursor: 0,
            error: None,
            last_prompt: None,
        }
    }

    fn run(mut self) -> Result<InstallArgs> {
        let mut session = TuiSession::new()?;
        loop {
            self.prepare_for_prompt();
            session.draw(|frame| self.render(frame))?;
            if !event::poll(Duration::from_millis(250))? {
                continue;
            }
            let event = event::read()?;
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                match self.handle_key(key)? {
                    Flow::Continue => {}
                    Flow::Finish => {
                        session.restore()?;
                        return Ok(self.args);
                    }
                    Flow::Cancel => {
                        session.restore()?;
                        bail!("installation cancelled");
                    }
                }
            }
        }
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

        let title = self.current_prompt().title();
        let hero = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{}/{}", self.prompt_index + 1, self.prompts().len()),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                title,
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                self.current_prompt().subtitle(),
                Style::default().fg(MUTED),
            )),
        ])
        .block(card("Installer"));
        frame.render_widget(hero, header[1]);

        let [sidebar_area, content_area] = split_main(main_area);
        self.render_sidebar(frame, sidebar_area);

        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(12)])
            .split(content_area);
        match self.current_prompt() {
            Prompt::Confirm => self.render_confirm(frame, content[0]),
            _ => self.render_text_prompt(frame, content[0]),
        }
        self.render_preview(frame, content[1]);
        self.render_status(frame, status_area);
        frame.render_widget(
            footer_widget("enter save/confirm  esc back  ctrl+c cancel"),
            footer_area,
        );
    }

    fn render_sidebar(&self, frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let items = self
            .prompts()
            .iter()
            .enumerate()
            .map(|(index, prompt)| {
                let prefix = if index < self.prompt_index {
                    "*"
                } else if index == self.prompt_index {
                    ">"
                } else {
                    "-"
                };
                let style = if index == self.prompt_index {
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                } else if index < self.prompt_index {
                    Style::default().fg(TEXT)
                } else {
                    Style::default().fg(MUTED)
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{prefix} {}", prompt.sidebar_label()),
                    style,
                )))
            })
            .collect::<Vec<_>>();
        frame.render_widget(List::new(items).block(muted_card("Setup")), area);
    }

    fn render_text_prompt(&self, frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(3)])
            .split(area);
        let help = Paragraph::new(vec![
            Line::from(Span::styled(
                self.current_prompt().title(),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                self.current_prompt().subtitle(),
                Style::default().fg(MUTED),
            )),
        ])
        .block(card("Field"));
        frame.render_widget(help, chunks[0]);
        let input = Paragraph::new(self.render_input_line())
            .block(card("Value"))
            .wrap(Wrap { trim: false });
        frame.render_widget(input, chunks[1]);
    }

    fn render_confirm(&self, frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let paragraph = Paragraph::new(self.summary_text())
            .block(card("Review"))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_preview(&self, frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let paragraph = Paragraph::new(self.summary_text())
            .block(muted_card("Preview"))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_status(&self, frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let (title, color, body) = match self.error.as_deref() {
            Some(message) => ("Error", DANGER, message.to_owned()),
            None => ("Hint", WARNING, self.current_prompt().hint().to_owned()),
        };
        let paragraph = Paragraph::new(body)
            .style(Style::default().fg(color))
            .block(muted_card(title))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<Flow> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'q'))
        {
            return Ok(Flow::Cancel);
        }
        if matches!(key.code, KeyCode::Esc | KeyCode::BackTab) {
            self.go_back();
            return Ok(Flow::Continue);
        }
        if self.current_prompt() == Prompt::Confirm {
            return self.handle_confirm_key(key);
        }
        self.handle_text_key(key)
    }

    fn handle_text_key(&mut self, key: KeyEvent) -> Result<Flow> {
        match key.code {
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right if self.cursor < self.input.len() => self.cursor += 1,
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.len(),
            KeyCode::Backspace if self.cursor > 0 && self.cursor <= self.input.len() => {
                self.cursor -= 1;
                self.input.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.input.len() => {
                self.input.remove(self.cursor);
            }
            KeyCode::Char(char) => {
                self.input.insert(self.cursor, char);
                self.cursor += char.len_utf8();
            }
            _ if is_submit_key(key) => {
                if let Err(error) = self.commit_text() {
                    self.error = Some(error.to_string());
                    return Ok(Flow::Continue);
                }
                self.advance();
            }
            _ => {}
        }
        Ok(Flow::Continue)
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> Result<Flow> {
        if !is_submit_key(key) {
            return Ok(Flow::Continue);
        }
        match InstallConfig::from_args(self.args.clone()) {
            Ok(_) => Ok(Flow::Finish),
            Err(error) => {
                self.error = Some(error.to_string());
                Ok(Flow::Continue)
            }
        }
    }

    fn prepare_for_prompt(&mut self) {
        self.prime_defaults();
        let prompt = self.current_prompt();
        if self.last_prompt == Some(prompt) {
            return;
        }
        self.last_prompt = Some(prompt);
        self.error = None;
        match prompt {
            Prompt::Domain => self.set_input(
                self.args
                    .domain
                    .clone()
                    .unwrap_or_else(|| "panel.example.com".into()),
            ),
            Prompt::SuperadminEmail => self.set_input(
                self.args
                    .superadmin_email
                    .clone()
                    .unwrap_or_else(|| "admin@panel.example.com".into()),
            ),
            Prompt::SuperadminDisplayName => {
                self.set_input(self.args.superadmin_display_name.clone())
            }
            Prompt::ResellerTenantName => self.set_input(
                self.args
                    .reseller_tenant_name
                    .clone()
                    .unwrap_or_else(|| "Default Tenant".into()),
            ),
            Prompt::Confirm => {}
        }
    }

    fn commit_text(&mut self) -> Result<()> {
        let value = self.input.trim().to_owned();
        match self.current_prompt() {
            Prompt::Domain => {
                if value.is_empty() {
                    bail!("domain is required");
                }
                if value.starts_with("http://") || value.starts_with("https://") {
                    bail!("enter only a domain, not a URL");
                }
                if value.contains('/') {
                    bail!("domain must not contain path segments");
                }
                self.args.domain = Some(value);
                self.args.public_base_url = None;
            }
            Prompt::SuperadminEmail => {
                if value.is_empty() || !value.contains('@') {
                    bail!("admin email is required");
                }
                self.args.superadmin_email = Some(value);
            }
            Prompt::SuperadminDisplayName => {
                if value.is_empty() {
                    bail!("admin display name is required");
                }
                self.args.superadmin_display_name = value;
            }
            Prompt::ResellerTenantName => {
                if value.is_empty() {
                    bail!("tenant name is required");
                }
                self.args.reseller_tenant_name = Some(value);
            }
            Prompt::Confirm => {}
        }
        Ok(())
    }

    fn prime_defaults(&mut self) {
        self.args.role.get_or_insert(InstallRole::ControlPlane);
        self.args
            .deployment_mode
            .get_or_insert(DeploymentMode::Native);
        if self.args.superadmin_display_name.trim().is_empty() {
            self.args.superadmin_display_name = "Superadmin".into();
        }
    }

    fn current_prompt(&self) -> Prompt {
        self.prompts()[self.prompt_index.min(self.prompts().len() - 1)]
    }

    fn prompts(&self) -> Vec<Prompt> {
        let mut prompts = Vec::new();
        if self.args.domain.as_deref().is_none_or(str::is_empty)
            && self
                .args
                .public_base_url
                .as_deref()
                .is_none_or(str::is_empty)
        {
            prompts.push(Prompt::Domain);
        }
        if self
            .args
            .superadmin_email
            .as_deref()
            .is_none_or(str::is_empty)
        {
            prompts.push(Prompt::SuperadminEmail);
        }
        if self.args.superadmin_display_name.trim().is_empty() {
            prompts.push(Prompt::SuperadminDisplayName);
        }
        if self
            .args
            .reseller_tenant_name
            .as_deref()
            .is_none_or(str::is_empty)
        {
            prompts.push(Prompt::ResellerTenantName);
        }
        prompts.push(Prompt::Confirm);
        prompts
    }

    fn advance(&mut self) {
        let last = self.prompts().len().saturating_sub(1);
        self.prompt_index = (self.prompt_index + 1).min(last);
        self.last_prompt = None;
    }

    fn go_back(&mut self) {
        self.prompt_index = self.prompt_index.saturating_sub(1);
        self.last_prompt = None;
    }

    fn set_input(&mut self, value: String) {
        self.input = value;
        self.cursor = self.input.len();
    }

    fn render_input_line(&self) -> Line<'static> {
        let cursor = self.cursor.min(self.input.len());
        let before = self.input[..cursor].to_owned();
        let after = self.input[cursor..].to_owned();
        if after.is_empty() {
            return Line::from(vec![
                Span::styled(before, Style::default().fg(TEXT)),
                Span::styled(" ", Style::default().bg(ACCENT).fg(BG)),
            ]);
        }
        let mut chars = after.chars();
        let current = chars.next().unwrap_or(' ');
        let rest = chars.collect::<String>();
        Line::from(vec![
            Span::styled(before, Style::default().fg(TEXT)),
            Span::styled(current.to_string(), Style::default().bg(ACCENT).fg(BG)),
            Span::styled(rest, Style::default().fg(TEXT)),
        ])
    }

    fn summary_text(&self) -> String {
        match InstallConfig::from_args(self.args.clone()) {
            Ok(config) => {
                let control_plane = config.control_plane;
                let reseller = control_plane
                    .reseller
                    .as_ref()
                    .map(|item| item.tenant_name.as_str())
                    .unwrap_or("not configured");
                format!(
                    "Mode: native control-plane\nDomain: {}\nPanel URL: {}\nPanel path: {}\nAdmin email: {}\nAdmin password: {}\nTenant: {}\nDatabase: {}",
                    control_plane.domain,
                    control_plane.public_base_url,
                    control_plane.panel_path,
                    control_plane.superadmin.email,
                    control_plane.superadmin.password,
                    reseller,
                    control_plane.database_url,
                )
            }
            Err(error) => format!("Fill required values to preview install config.\n\n{error}"),
        }
    }
}

impl Prompt {
    fn title(self) -> &'static str {
        match self {
            Self::Domain => "Panel domain",
            Self::SuperadminEmail => "Superadmin email",
            Self::SuperadminDisplayName => "Superadmin display name",
            Self::ResellerTenantName => "Default tenant",
            Self::Confirm => "Review installation",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Domain => "Enter the domain that points to this VPS.",
            Self::SuperadminEmail => "This email becomes the first admin login.",
            Self::SuperadminDisplayName => "This name is shown in the panel and audit log.",
            Self::ResellerTenantName => "The installer creates this tenant automatically.",
            Self::Confirm => "Press Enter to start installation.",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Domain => "Example: panel.example.com. Do not enter https:// or a path.",
            Self::SuperadminEmail => "Use an email you will remember for the first login.",
            Self::SuperadminDisplayName => "Default value is fine for most installs.",
            Self::ResellerTenantName => "You can rename tenants later from the panel.",
            Self::Confirm => "The password shown here will also be written to admin-summary.env.",
        }
    }

    fn sidebar_label(self) -> &'static str {
        match self {
            Self::Domain => "Domain",
            Self::SuperadminEmail => "Admin email",
            Self::SuperadminDisplayName => "Admin name",
            Self::ResellerTenantName => "Tenant",
            Self::Confirm => "Review",
        }
    }
}

fn is_submit_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Enter)
        || matches!(key.code, KeyCode::Char('\n' | '\r'))
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('m' | 'j')))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_args() -> InstallArgs {
        InstallArgs {
            bundle_root: None,
            lang: None,
            role: None,
            deployment_mode: None,
            domain: None,
            panel_path: None,
            public_base_url: None,
            database_url: None,
            otlp_endpoint: None,
            bootstrap_token: None,
            data_encryption_key: None,
            token_hash_key: None,
            access_jwt_secret: None,
            pre_auth_jwt_secret: None,
            superadmin_email: None,
            superadmin_display_name: "Superadmin".into(),
            superadmin_password: None,
            reseller_tenant_name: None,
            reseller_email: None,
            reseller_display_name: None,
            reseller_password: None,
            starter_subscription_name: None,
            starter_subscription_traffic_limit_bytes: None,
            starter_subscription_days: None,
            non_interactive: false,
        }
    }

    #[test]
    fn wizard_prompts_required_interactive_values() {
        let wizard = RatatuiWizard::new(sample_args());

        assert_eq!(
            wizard.prompts(),
            vec![
                Prompt::Domain,
                Prompt::SuperadminEmail,
                Prompt::ResellerTenantName,
                Prompt::Confirm,
            ]
        );
    }

    #[test]
    fn wizard_keeps_non_interactive_args_unchanged() {
        let mut args = sample_args();
        args.non_interactive = true;

        let prepared = prepare_install_args(args.clone()).expect("prepared args");

        assert_eq!(prepared.domain, args.domain);
        assert_eq!(prepared.public_base_url, args.public_base_url);
    }

    #[test]
    fn domain_prompt_rejects_urls() {
        let mut wizard = RatatuiWizard::new(sample_args());
        wizard.set_input("https://panel.example.com/panel".into());

        let error = wizard.commit_text().expect_err("url must be rejected");

        assert!(error.to_string().contains("not a URL"));
    }

    #[test]
    fn domain_prompt_accepts_bare_domain() {
        let mut wizard = RatatuiWizard::new(sample_args());
        wizard.set_input("panel.example.com".into());

        wizard.commit_text().expect("domain accepted");

        assert_eq!(wizard.args.domain.as_deref(), Some("panel.example.com"));
        assert!(wizard.args.public_base_url.is_none());
    }
}
