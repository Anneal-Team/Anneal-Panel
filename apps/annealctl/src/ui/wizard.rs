use std::{io::IsTerminal, time::Duration};

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Line, Modifier, Span, Style},
    widgets::{List, ListItem, Paragraph, Wrap},
};

use crate::{
    cli::InstallArgs,
    config::{InstallConfig, InstallRole, engines_csv},
    i18n::{Language, Translator, terminal_supports_utf8},
};

use super::tui::{
    ACCENT, BG, DANGER, MUTED, PANEL_ALT, TEXT, TuiSession, WARNING, brand, card,
    footer as footer_widget, frame_layout, muted_card, page_background, split_main,
};

pub fn prepare_install_args(args: InstallArgs) -> Result<(InstallArgs, Translator)> {
    let interactive =
        !args.non_interactive && std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let unicode = terminal_supports_utf8();
    let explicit_language = args.lang.is_some() || !unicode;
    let language = if unicode {
        args.lang.unwrap_or_else(|| Language::resolve(None))
    } else {
        Language::En
    };
    if !interactive || !unicode {
        let mut args = args;
        args.lang = Some(language);
        let translator = Translator::new(language);
        return Ok((args, translator));
    }
    RatatuiWizard::new(args, language, explicit_language).run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    Language,
    Role,
    Mode,
    Domain,
    SuperadminEmail,
    SuperadminDisplayName,
    TenantName,
    NodeGroup,
    ServerUrl,
    BootstrapToken,
    NodeName,
    Engines,
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
    language: Language,
    explicit_language: bool,
    prompt_role: bool,
    prompt_mode: bool,
    prompt_index: usize,
    selection_index: usize,
    input: String,
    cursor: usize,
    error: Option<String>,
    last_prompt: Option<Prompt>,
}

impl RatatuiWizard {
    fn new(args: InstallArgs, language: Language, explicit_language: bool) -> Self {
        let prompt_role = args.role.is_none();
        let prompt_mode = args.deployment_mode.is_none();
        Self {
            args,
            language,
            explicit_language,
            prompt_role,
            prompt_mode,
            prompt_index: 0,
            selection_index: 0,
            input: String::new(),
            cursor: 0,
            error: None,
            last_prompt: None,
        }
    }

    fn run(mut self) -> Result<(InstallArgs, Translator)> {
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
                        self.args.lang = Some(self.language);
                        let translator = Translator::new(self.language);
                        return Ok((self.args, translator));
                    }
                    Flow::Cancel => {
                        session.restore()?;
                        bail!(Translator::new(self.language).cancelled().to_owned());
                    }
                }
            }
        }
    }

    fn render(&self, frame: &mut ratatui::Frame<'_>) {
        let translator = Translator::new(self.language);
        let area = frame.area();
        frame.render_widget(page_background(), area);

        let [header_area, main_area, status_area, footer_area] = frame_layout(area);
        let header = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(30)])
            .split(header_area);
        frame.render_widget(brand(), header[0]);

        let title = self.current_prompt().title(translator);
        let hero = Paragraph::new(vec![
            Line::from(Span::styled(
                format!(
                    "{}/{}",
                    self.prompt_index + 1,
                    self.prompts().len().max(self.prompt_index + 1)
                ),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                title,
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                self.current_prompt().subtitle(translator),
                Style::default().fg(MUTED),
            )),
        ])
        .block(card("Installer"));
        frame.render_widget(hero, header[1]);

        let [sidebar_area, content_area] = split_main(main_area);
        self.render_sidebar(frame, sidebar_area, translator);

        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(12)])
            .split(content_area);
        self.render_prompt(frame, content[0], translator);
        self.render_preview(frame, content[1], translator);

        self.render_status(frame, status_area, translator);
        frame.render_widget(footer_widget(self.footer(translator)), footer_area);
    }

    fn render_sidebar(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
        translator: Translator,
    ) {
        let items = self
            .prompts()
            .iter()
            .enumerate()
            .map(|(index, prompt)| {
                let prefix = if index < self.prompt_index {
                    "●"
                } else if index == self.prompt_index {
                    "▶"
                } else {
                    "○"
                };
                let style = if index == self.prompt_index {
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                } else if index < self.prompt_index {
                    Style::default().fg(TEXT)
                } else {
                    Style::default().fg(MUTED)
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{prefix} {}", prompt.sidebar_label(translator)),
                    style,
                )))
            })
            .collect::<Vec<_>>();
        frame.render_widget(List::new(items).block(muted_card("Flow")), area);
    }

    fn render_prompt(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
        translator: Translator,
    ) {
        match self.current_prompt() {
            Prompt::Language | Prompt::Role | Prompt::Mode => {
                self.render_select_prompt(frame, area, translator);
            }
            Prompt::Engines => {
                self.render_engine_prompt(frame, area, translator);
            }
            Prompt::Confirm => {
                self.render_confirm(frame, area, translator);
            }
            _ => {
                self.render_text_prompt(frame, area, translator);
            }
        }
    }

    fn render_select_prompt(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
        translator: Translator,
    ) {
        let options = self.current_prompt().options(translator, self.language);
        let items = options
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let selected = index == self.selection_index;
                let style = if selected {
                    Style::default()
                        .fg(ACCENT)
                        .bg(PANEL_ALT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(TEXT)
                };
                let marker = if selected { "›" } else { " " };
                ListItem::new(Line::from(Span::styled(format!("{marker} {value}"), style)))
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(items).block(card(self.current_prompt().title(translator))),
            area,
        );
    }

    fn render_engine_prompt(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
        translator: Translator,
    ) {
        let items = ["xray", "singbox"]
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let selected = index == self.selection_index;
                let enabled = self.selected_engines().contains(&value.to_owned());
                let checkbox = if enabled { "[x]" } else { "[ ]" };
                let style = if selected {
                    Style::default()
                        .fg(ACCENT)
                        .bg(PANEL_ALT)
                        .add_modifier(Modifier::BOLD)
                } else if enabled {
                    Style::default().fg(TEXT)
                } else {
                    Style::default().fg(MUTED)
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{checkbox} {value}"),
                    style,
                )))
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(items).block(card(self.current_prompt().title(translator))),
            area,
        );
    }

    fn render_text_prompt(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
        translator: Translator,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(3)])
            .split(area);
        let help = Paragraph::new(vec![
            Line::from(Span::styled(
                self.current_prompt().title(translator),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                self.current_prompt().subtitle(translator),
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

    fn render_confirm(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
        translator: Translator,
    ) {
        let content = match preview_config(&self.args) {
            Ok(config) => summary_text(translator, &config),
            Err(error) => error.to_string(),
        };
        let paragraph = Paragraph::new(content)
            .block(card(translator.summary_title()))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_preview(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
        translator: Translator,
    ) {
        let paragraph = Paragraph::new(self.preview_text(translator))
            .block(muted_card(pick(translator, "Предпросмотр", "Preview")))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_status(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
        translator: Translator,
    ) {
        let (title, color, body) = match self.error.as_deref() {
            Some(message) => (
                pick(translator, "Ошибка", "Error"),
                DANGER,
                message.to_owned(),
            ),
            None => (
                pick(translator, "Подсказка", "Hint"),
                WARNING,
                self.current_prompt().hint(translator),
            ),
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
        if matches!(key.code, KeyCode::Char('q')) {
            return Ok(Flow::Cancel);
        }
        if matches!(key.code, KeyCode::Esc | KeyCode::BackTab) {
            self.go_back();
            return Ok(Flow::Continue);
        }
        match self.current_prompt() {
            Prompt::Language | Prompt::Role | Prompt::Mode => self.handle_select_key(key),
            Prompt::Engines => self.handle_engine_key(key),
            Prompt::Confirm => self.handle_confirm_key(key),
            _ => self.handle_text_key(key),
        }
    }

    fn handle_select_key(&mut self, key: KeyEvent) -> Result<Flow> {
        let max = self
            .current_prompt()
            .options(Translator::new(self.language), self.language)
            .len();
        match key.code {
            KeyCode::Up => {
                self.selection_index = self.selection_index.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.selection_index + 1 < max {
                    self.selection_index += 1;
                }
            }
            _ if is_submit_key(key) => {
                match self.current_prompt() {
                    Prompt::Language => {
                        self.language = if self.selection_index == 0 {
                            Language::Ru
                        } else {
                            Language::En
                        };
                    }
                    Prompt::Role => {
                        self.args.role = Some(match self.selection_index {
                            0 => InstallRole::AllInOne,
                            1 => InstallRole::ControlPlane,
                            _ => InstallRole::Node,
                        });
                    }
                    Prompt::Mode => {
                        self.args.deployment_mode = Some(if self.selection_index == 0 {
                            crate::config::DeploymentMode::Native
                        } else {
                            crate::config::DeploymentMode::Docker
                        });
                    }
                    _ => {}
                }
                self.advance();
            }
            _ => {}
        }
        Ok(Flow::Continue)
    }

    fn handle_engine_key(&mut self, key: KeyEvent) -> Result<Flow> {
        match key.code {
            KeyCode::Up => {
                self.selection_index = self.selection_index.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.selection_index < 1 {
                    self.selection_index += 1;
                }
            }
            KeyCode::Char(' ') => {
                let engine = if self.selection_index == 0 {
                    "xray"
                } else {
                    "singbox"
                };
                self.toggle_engine(engine);
            }
            _ if is_submit_key(key) => {
                if self.selected_engines().is_empty() {
                    self.error = Some(
                        pick(
                            Translator::new(self.language),
                            "Выбери хотя бы один движок.",
                            "Select at least one engine.",
                        )
                        .to_owned(),
                    );
                } else {
                    self.advance();
                }
            }
            _ => {}
        }
        Ok(Flow::Continue)
    }

    fn handle_text_key(&mut self, key: KeyEvent) -> Result<Flow> {
        match key.code {
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => {
                self.cursor = 0;
            }
            KeyCode::End => {
                self.cursor = self.input.len();
            }
            KeyCode::Backspace => {
                if self.cursor > 0 && self.cursor <= self.input.len() {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
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
        match preview_config(&self.args) {
            Ok(_) => Ok(Flow::Finish),
            Err(error) => {
                self.error = Some(error.to_string());
                Ok(Flow::Continue)
            }
        }
    }

    fn prepare_for_prompt(&mut self) {
        self.prime_defaults();
        let prompts = self.prompts();
        if prompts.is_empty() {
            return;
        }
        if self.prompt_index >= prompts.len() {
            self.prompt_index = prompts.len() - 1;
        }
        let prompt = prompts[self.prompt_index];
        if self.last_prompt == Some(prompt) {
            return;
        }
        self.last_prompt = Some(prompt);
        self.error = None;
        match prompt {
            Prompt::Language => {
                self.selection_index = if self.language == Language::Ru { 0 } else { 1 };
            }
            Prompt::Role => {
                self.selection_index = match self.args.role.unwrap_or(InstallRole::AllInOne) {
                    InstallRole::AllInOne => 0,
                    InstallRole::ControlPlane => 1,
                    InstallRole::Node => 2,
                };
            }
            Prompt::Mode => {
                self.selection_index = match self
                    .args
                    .deployment_mode
                    .unwrap_or(crate::config::DeploymentMode::Native)
                {
                    crate::config::DeploymentMode::Native => 0,
                    crate::config::DeploymentMode::Docker => 1,
                };
            }
            Prompt::Domain => self.set_input(self.domain_value()),
            Prompt::SuperadminEmail => self.set_input(self.superadmin_email_value()),
            Prompt::SuperadminDisplayName => {
                self.set_input(self.args.superadmin_display_name.clone())
            }
            Prompt::TenantName => self.set_input(self.tenant_name_value()),
            Prompt::NodeGroup => self.set_input(self.node_group_value()),
            Prompt::ServerUrl => self.set_input(
                self.args
                    .agent_server_url
                    .clone()
                    .unwrap_or_else(|| "https://panel.example.com/private-path".into()),
            ),
            Prompt::BootstrapToken => {
                self.set_input(self.args.agent_bootstrap_token.clone().unwrap_or_default())
            }
            Prompt::NodeName => self.set_input(self.node_name_value()),
            Prompt::Engines => {
                self.selection_index = 0;
            }
            Prompt::Confirm => {}
        }
    }

    fn commit_text(&mut self) -> Result<()> {
        let value = self.input.trim().to_owned();
        if value.is_empty() {
            bail!(pick(
                Translator::new(self.language),
                "Поле не должно быть пустым.",
                "Field must not be empty.",
            ));
        }
        match self.current_prompt() {
            Prompt::Domain => {
                self.args.domain = Some(value);
            }
            Prompt::SuperadminEmail => {
                self.args.superadmin_email = Some(value);
            }
            Prompt::SuperadminDisplayName => {
                self.args.superadmin_display_name = value;
            }
            Prompt::TenantName => {
                self.args.reseller_tenant_name = Some(value);
            }
            Prompt::NodeGroup => {
                self.args.node_group_name = Some(value);
            }
            Prompt::ServerUrl => {
                self.args.agent_server_url = Some(value);
            }
            Prompt::BootstrapToken => {
                self.args.agent_bootstrap_token = Some(value);
            }
            Prompt::NodeName => {
                self.args.agent_name = Some(value);
            }
            _ => {}
        }
        Ok(())
    }

    fn advance(&mut self) {
        self.prompt_index += 1;
        self.last_prompt = None;
        self.error = None;
    }

    fn go_back(&mut self) {
        if self.prompt_index > 0 {
            self.prompt_index -= 1;
            self.last_prompt = None;
            self.error = None;
        }
    }

    fn current_prompt(&self) -> Prompt {
        let prompts = self.prompts();
        prompts[self.prompt_index.min(prompts.len().saturating_sub(1))]
    }

    fn prompts(&self) -> Vec<Prompt> {
        let mut prompts = Vec::new();
        if !self.explicit_language {
            prompts.push(Prompt::Language);
        }
        if self.prompt_role {
            prompts.push(Prompt::Role);
        }
        if self.prompt_mode {
            prompts.push(Prompt::Mode);
        }
        if let Some(role) = self.args.role {
            if role.includes_control_plane() {
                prompts.extend([
                    Prompt::Domain,
                    Prompt::SuperadminEmail,
                    Prompt::SuperadminDisplayName,
                ]);
            }
            match role {
                InstallRole::AllInOne => {
                    prompts.extend([Prompt::TenantName, Prompt::NodeGroup, Prompt::Engines]);
                }
                InstallRole::Node => {
                    prompts.extend([
                        Prompt::ServerUrl,
                        Prompt::BootstrapToken,
                        Prompt::NodeName,
                        Prompt::Engines,
                    ]);
                }
                InstallRole::ControlPlane => {}
            }
        }
        if self.args.role.is_some() && self.args.deployment_mode.is_some() {
            prompts.push(Prompt::Confirm);
        }
        prompts
    }

    fn footer(&self, translator: Translator) -> String {
        match self.current_prompt() {
            Prompt::Engines => pick(
                translator,
                "↑/↓ выбор  space переключить  enter дальше  esc назад  q выход",
                "↑/↓ select  space toggle  enter continue  esc back  q quit",
            ),
            Prompt::Confirm => pick(
                translator,
                "enter подтвердить  esc назад  q выход",
                "enter confirm  esc back  q quit",
            ),
            Prompt::Language | Prompt::Role | Prompt::Mode => pick(
                translator,
                "↑/↓ выбор  enter продолжить  esc назад  q выход",
                "↑/↓ select  enter continue  esc back  q quit",
            ),
            _ => pick(
                translator,
                "печать для ввода  enter сохранить  esc назад  q выход",
                "type to edit  enter save  esc back  q quit",
            ),
        }
        .to_owned()
    }

    fn preview_text(&self, translator: Translator) -> String {
        if let Ok(config) = preview_config(&self.args) {
            return summary_text(translator, &config);
        }
        let mut lines = Vec::new();
        lines.push(format!(
            "{}: {}",
            pick(translator, "Язык", "Language"),
            match self.language {
                Language::Ru => "Русский",
                Language::En => "English",
            }
        ));
        if let Some(role) = self.args.role {
            lines.push(format!(
                "{}: {}",
                translator.role_label(),
                translator.install_role(role)
            ));
        }
        if let Some(mode) = self.args.deployment_mode {
            lines.push(format!(
                "{}: {}",
                translator.mode_label(),
                translator.deployment_mode(mode)
            ));
        }
        if let Some(domain) = self.args.domain.as_deref() {
            lines.push(format!("{}: {domain}", translator.domain_prompt()));
        }
        if let Some(url) = self.args.agent_server_url.as_deref() {
            lines.push(format!("{}: {url}", translator.server_url_prompt()));
        }
        if !self.selected_engines().is_empty() {
            lines.push(format!(
                "{}: {}",
                translator.engines_label(),
                self.selected_engines().join(",")
            ));
        }
        lines.join("\n")
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

    fn set_input(&mut self, value: String) {
        self.input = value;
        self.cursor = self.input.len();
    }

    fn prime_defaults(&mut self) {
        if self.args.role.is_some() {
            self.args.lang = Some(self.language);
        }
        if self
            .args
            .role
            .is_some_and(InstallRole::includes_control_plane)
            && self.args.superadmin_display_name.trim().is_empty()
        {
            self.args.superadmin_display_name = "Superadmin".into();
        }
        if self.args.role.is_some_and(InstallRole::includes_node)
            && empty(self.args.agent_engines.as_deref())
        {
            self.args.agent_engines = Some("xray,singbox".into());
        }
        if self
            .args
            .role
            .is_some_and(InstallRole::includes_control_plane)
            && empty(self.args.domain.as_deref())
        {
            self.args.domain = Some(
                self.args
                    .public_base_url
                    .clone()
                    .unwrap_or_else(|| "panel.example.com".into()),
            );
        }
        if empty(self.args.superadmin_email.as_deref()) {
            if let Ok(config) = preview_config(&self.args) {
                if let Some(control_plane) = config.control_plane {
                    self.args.superadmin_email = Some(control_plane.superadmin.email);
                }
            }
        }
        if self.args.role == Some(InstallRole::AllInOne) {
            if empty(self.args.reseller_tenant_name.as_deref()) {
                if let Ok(config) = preview_config(&self.args) {
                    if let Some(reseller) = config.control_plane.and_then(|item| item.reseller) {
                        self.args.reseller_tenant_name = Some(reseller.tenant_name);
                    }
                }
            }
            if empty(self.args.node_group_name.as_deref()) {
                if let Ok(config) = preview_config(&self.args) {
                    if let Some(node) = config.node {
                        self.args.node_group_name = Some(node.group_name.unwrap_or(node.name));
                    }
                }
            }
        }
        if self.args.role == Some(InstallRole::Node) {
            if empty(self.args.agent_server_url.as_deref()) {
                self.args.agent_server_url = Some("https://panel.example.com/private-path".into());
            }
            if empty(self.args.agent_name.as_deref())
                && !empty(self.args.agent_bootstrap_token.as_deref())
            {
                if let Ok(config) = preview_config(&self.args) {
                    if let Some(node) = config.node {
                        self.args.agent_name = Some(node.name);
                    }
                }
            }
        }
    }

    fn domain_value(&self) -> String {
        self.args
            .domain
            .clone()
            .or_else(|| self.args.public_base_url.clone())
            .unwrap_or_else(|| "panel.example.com".into())
    }

    fn superadmin_email_value(&self) -> String {
        self.args
            .superadmin_email
            .clone()
            .unwrap_or_else(|| "admin@panel.example.com".into())
    }

    fn tenant_name_value(&self) -> String {
        self.args
            .reseller_tenant_name
            .clone()
            .unwrap_or_else(|| "Default Tenant".into())
    }

    fn node_group_value(&self) -> String {
        self.args
            .node_group_name
            .clone()
            .or_else(|| self.args.agent_name.clone())
            .unwrap_or_else(|| "edge-main".into())
    }

    fn node_name_value(&self) -> String {
        self.args
            .agent_name
            .clone()
            .unwrap_or_else(|| "node-main".into())
    }

    fn selected_engines(&self) -> Vec<String> {
        let mut engines = Vec::new();
        let current = self
            .args
            .agent_engines
            .clone()
            .unwrap_or_else(|| "xray,singbox".into());
        if current.split(',').any(|value| value.trim() == "xray") {
            engines.push("xray".into());
        }
        if current
            .split(',')
            .any(|value| matches!(value.trim(), "singbox" | "sing-box"))
        {
            engines.push("singbox".into());
        }
        engines
    }

    fn toggle_engine(&mut self, engine: &str) {
        let mut engines = self.selected_engines();
        if let Some(index) = engines.iter().position(|value| value == engine) {
            engines.remove(index);
        } else {
            engines.push(engine.to_owned());
        }
        let ordered = ["xray", "singbox"]
            .into_iter()
            .filter(|value| engines.iter().any(|item| item == value))
            .collect::<Vec<_>>();
        self.args.agent_engines = Some(ordered.join(","));
    }
}

fn is_submit_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Enter)
        || matches!(key.code, KeyCode::Char('\n' | '\r'))
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('m' | 'j')))
}

impl Prompt {
    fn title(self, translator: Translator) -> &'static str {
        match self {
            Self::Language => "Language / Язык",
            Self::Role => translator.install_role_prompt(),
            Self::Mode => translator.deployment_mode_prompt(),
            Self::Domain => translator.domain_prompt(),
            Self::SuperadminEmail => translator.superadmin_email_prompt(),
            Self::SuperadminDisplayName => translator.superadmin_display_name_prompt(),
            Self::TenantName => translator.tenant_name_prompt(),
            Self::NodeGroup => translator.node_group_prompt(),
            Self::ServerUrl => translator.server_url_prompt(),
            Self::BootstrapToken => translator.bootstrap_token_prompt(),
            Self::NodeName => translator.node_name_prompt(),
            Self::Engines => translator.engines_prompt(),
            Self::Confirm => translator.summary_title(),
        }
    }

    fn subtitle(self, translator: Translator) -> String {
        match self {
            Self::Language => pick(
                translator,
                "Выбери язык мастера установки.",
                "Choose the installer language.",
            )
            .into(),
            Self::Role => pick(
                translator,
                "Определи целевой сценарий установки.",
                "Choose the deployment role.",
            )
            .into(),
            Self::Mode => pick(
                translator,
                "Native ставит systemd-сервисы, Docker поднимает compose-стек.",
                "Native installs systemd services, Docker starts a compose stack.",
            )
            .into(),
            Self::Domain => pick(
                translator,
                "Можно ввести домен или полный URL панели.",
                "You can enter a bare domain or a full panel URL.",
            )
            .into(),
            Self::SuperadminEmail => pick(
                translator,
                "Этот email будет логином первого суперадмина.",
                "This email becomes the first superadmin login.",
            )
            .into(),
            Self::SuperadminDisplayName => pick(
                translator,
                "Имя будет видно в панели и аудите.",
                "This name is shown in the panel and audit log.",
            )
            .into(),
            Self::TenantName => pick(
                translator,
                "Tenant создаётся автоматически для локальной ноды.",
                "The tenant is created automatically for the local node.",
            )
            .into(),
            Self::NodeGroup => pick(
                translator,
                "Название группы для локальной ноды и bootstrap-сессии.",
                "This group name is used for the local node bootstrap.",
            )
            .into(),
            Self::ServerUrl => pick(
                translator,
                "URL должен указывать на control-plane с https.",
                "The URL must point to the control-plane over https.",
            )
            .into(),
            Self::BootstrapToken => pick(
                translator,
                "Токен bootstrap выдаётся control-plane перед регистрацией ноды.",
                "The bootstrap token is issued by the control-plane.",
            )
            .into(),
            Self::NodeName => pick(
                translator,
                "Имя станет базой для runtime-регистрации.",
                "This name is used for runtime registration.",
            )
            .into(),
            Self::Engines => pick(
                translator,
                "Выбери runtime-ядра, которые надо запускать на ноде.",
                "Choose which runtime engines should be enabled.",
            )
            .into(),
            Self::Confirm => pick(
                translator,
                "Проверь значения перед стартом установки.",
                "Review values before starting installation.",
            )
            .into(),
        }
    }

    fn hint(self, translator: Translator) -> String {
        match self {
            Self::Domain => pick(
                translator,
                "Если введёшь полный URL, panel path подставится автоматически.",
                "If you enter a full URL, the panel path is derived automatically.",
            )
            .into(),
            Self::Mode => pick(
                translator,
                "Для быстрого single-host чаще нужен native.",
                "For a simple single-host setup, native is usually the right choice.",
            )
            .into(),
            Self::Confirm => pick(
                translator,
                "После подтверждения installer сохранит конфиг и пойдёт по шагам bootstrap.",
                "After confirmation the installer saves config and starts bootstrap.",
            )
            .into(),
            _ => pick(
                translator,
                "Текущие значения справа обновляются на лету.",
                "The preview on the right updates live.",
            )
            .into(),
        }
    }

    fn sidebar_label(self, translator: Translator) -> &'static str {
        match self {
            Self::Language => "Language",
            Self::Role => translator.role_label(),
            Self::Mode => translator.mode_label(),
            Self::Domain => translator.domain_prompt(),
            Self::SuperadminEmail => translator.admin_email_label(),
            Self::SuperadminDisplayName => translator.superadmin_display_name_prompt(),
            Self::TenantName => translator.tenant_label(),
            Self::NodeGroup => translator.node_group_prompt(),
            Self::ServerUrl => translator.server_url_prompt(),
            Self::BootstrapToken => translator.bootstrap_token_prompt(),
            Self::NodeName => translator.node_label(),
            Self::Engines => translator.engines_label(),
            Self::Confirm => translator.summary_title(),
        }
    }

    fn options(self, translator: Translator, language: Language) -> Vec<String> {
        match self {
            Self::Language => vec!["Русский".into(), "English".into()],
            Self::Role => vec!["all-in-one".into(), "control-plane".into(), "node".into()],
            Self::Mode => vec!["native".into(), "docker".into()],
            _ => vec![
                pick(translator, "Русский", "English").into(),
                match language {
                    Language::Ru => "English".into(),
                    Language::En => "Русский".into(),
                },
            ],
        }
    }
}

fn preview_config(args: &InstallArgs) -> Result<InstallConfig> {
    let mut args = args.clone();
    args.lang = Some(args.lang.unwrap_or(Language::En));
    args.non_interactive = true;
    InstallConfig::from_args(args)
}

fn summary_text(translator: Translator, config: &InstallConfig) -> String {
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
            translator.panel_path_label(),
            control_plane.panel_path
        ));
        lines.push(format!(
            "{}: {}",
            translator.admin_email_label(),
            control_plane.superadmin.email
        ));
        lines.push(format!(
            "{}: {}",
            translator.admin_password_label(),
            control_plane.superadmin.password
        ));
        lines.push(format!(
            "{}: {}",
            translator.database_label(),
            control_plane.database_url
        ));
        if let Some(reseller) = control_plane.reseller.as_ref() {
            lines.push(format!(
                "{}: {}",
                translator.tenant_label(),
                reseller.tenant_name
            ));
        }
    }
    if let Some(node) = config.node.as_ref() {
        lines.push(format!("{}: {}", translator.node_label(), node.name));
        lines.push(format!(
            "{}: {}",
            translator.engines_label(),
            engines_csv(&node.engines)
        ));
    }
    lines.join("\n")
}

fn empty(value: Option<&str>) -> bool {
    value.is_none_or(|item| item.trim().is_empty())
}

fn pick(translator: Translator, ru: &'static str, en: &'static str) -> &'static str {
    match translator.language() {
        Language::Ru => ru,
        Language::En => en,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::DeploymentMode;

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
            agent_server_url: None,
            agent_name: None,
            node_group_name: None,
            agent_engines: None,
            agent_protocols_xray: None,
            agent_protocols_singbox: None,
            agent_bootstrap_token: None,
            starter_subscription_name: None,
            starter_subscription_traffic_limit_bytes: None,
            starter_subscription_days: None,
            non_interactive: false,
        }
    }

    #[test]
    fn prompt_sequence_for_all_in_one_contains_control_plane_and_node_steps() {
        let mut args = sample_args();
        args.role = Some(InstallRole::AllInOne);
        args.deployment_mode = Some(DeploymentMode::Native);
        let wizard = RatatuiWizard::new(args, Language::En, false);

        assert_eq!(
            wizard.prompts(),
            vec![
                Prompt::Language,
                Prompt::Domain,
                Prompt::SuperadminEmail,
                Prompt::SuperadminDisplayName,
                Prompt::TenantName,
                Prompt::NodeGroup,
                Prompt::Engines,
                Prompt::Confirm
            ]
        );
    }

    #[test]
    fn prompt_sequence_for_node_keeps_bootstrap_token_step() {
        let mut args = sample_args();
        args.role = Some(InstallRole::Node);
        args.deployment_mode = Some(DeploymentMode::Docker);
        let wizard = RatatuiWizard::new(args, Language::En, true);

        assert_eq!(
            wizard.prompts(),
            vec![
                Prompt::ServerUrl,
                Prompt::BootstrapToken,
                Prompt::NodeName,
                Prompt::Engines,
                Prompt::Confirm
            ]
        );
    }

    #[test]
    fn mode_prompt_stays_in_flow_after_selection() {
        let mut wizard = RatatuiWizard::new(sample_args(), Language::En, false);
        wizard.args.role = Some(InstallRole::AllInOne);
        wizard.args.deployment_mode = Some(DeploymentMode::Native);

        assert!(wizard.prompts().contains(&Prompt::Mode));
        assert_eq!(wizard.prompts().last(), Some(&Prompt::Confirm));
    }
}
