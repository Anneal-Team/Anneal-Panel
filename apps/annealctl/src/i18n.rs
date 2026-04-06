use std::env;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::config::{DeploymentMode, InstallRole};
use crate::state::InstallStep;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Ru,
    En,
}

impl Language {
    pub fn resolve(explicit: Option<Self>) -> Self {
        explicit.or_else(Self::from_env).unwrap_or(Self::En)
    }

    pub fn from_env() -> Option<Self> {
        ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
            .into_iter()
            .find_map(|key| {
                env::var(key)
                    .ok()
                    .and_then(|value| Self::from_locale(&value))
            })
    }

    pub fn from_locale(value: &str) -> Option<Self> {
        let locale = value.trim().to_ascii_lowercase();
        if locale.starts_with("ru") {
            return Some(Self::Ru);
        }
        if locale.starts_with("en") {
            return Some(Self::En);
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Translator {
    language: Language,
    #[cfg(test)]
    unicode: bool,
}

impl Translator {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            #[cfg(test)]
            unicode: terminal_supports_utf8(),
        }
    }

    #[cfg(test)]
    pub fn with_unicode(language: Language, unicode: bool) -> Self {
        Self { language, unicode }
    }

    pub fn language(self) -> Language {
        self.language
    }

    #[cfg(test)]
    pub fn banner(self) -> &'static str {
        if self.unicode {
            "▂▄▆█ Anneal"
        } else {
            "Anneal"
        }
    }

    pub fn install_role_prompt(self) -> &'static str {
        self.pick("Роль установки", "Install role")
    }

    pub fn deployment_mode_prompt(self) -> &'static str {
        self.pick("Режим развертывания", "Deployment mode")
    }

    pub fn domain_prompt(self) -> &'static str {
        self.pick("Домен или URL панели", "Domain or panel URL")
    }

    pub fn server_url_prompt(self) -> &'static str {
        self.pick("URL control-plane", "Control-plane URL")
    }

    pub fn bootstrap_token_prompt(self) -> &'static str {
        self.pick("Bootstrap token ноды", "Node bootstrap token")
    }

    pub fn superadmin_email_prompt(self) -> &'static str {
        self.pick("Email суперадмина", "Superadmin email")
    }

    pub fn superadmin_display_name_prompt(self) -> &'static str {
        self.pick("Отображаемое имя суперадмина", "Superadmin display name")
    }

    pub fn tenant_name_prompt(self) -> &'static str {
        self.pick("Имя tenant по умолчанию", "Default tenant name")
    }

    pub fn node_group_prompt(self) -> &'static str {
        self.pick("Имя локальной ноды", "Local node name")
    }

    pub fn node_name_prompt(self) -> &'static str {
        self.pick("Имя ноды", "Node name")
    }

    pub fn engines_prompt(self) -> &'static str {
        self.pick("Движки proxy", "Proxy engines")
    }

    pub fn summary_title(self) -> &'static str {
        self.pick("Проверь конфигурацию", "Review configuration")
    }

    pub fn cancelled(self) -> &'static str {
        self.pick("Установка отменена", "Installation cancelled")
    }

    pub fn progress_prefix(self) -> &'static str {
        self.pick("Установка", "Install")
    }

    pub fn completed(self) -> &'static str {
        self.pick("Установка завершена", "Installation completed")
    }

    pub fn failed(self) -> &'static str {
        self.pick("Установка завершилась с ошибкой", "Installation failed")
    }

    pub fn role_label(self) -> &'static str {
        self.pick("Роль", "Role")
    }

    pub fn mode_label(self) -> &'static str {
        self.pick("Режим", "Mode")
    }

    pub fn public_url_label(self) -> &'static str {
        self.pick("Публичный URL", "Public URL")
    }

    pub fn panel_path_label(self) -> &'static str {
        self.pick("Путь панели", "Panel path")
    }

    pub fn admin_email_label(self) -> &'static str {
        self.pick("Email администратора", "Admin email")
    }

    pub fn admin_password_label(self) -> &'static str {
        self.pick("Пароль администратора", "Admin password")
    }

    pub fn tenant_label(self) -> &'static str {
        self.pick("Tenant", "Tenant")
    }

    pub fn node_label(self) -> &'static str {
        self.pick("Нода", "Node")
    }

    pub fn engines_label(self) -> &'static str {
        self.pick("Движки", "Engines")
    }

    pub fn database_label(self) -> &'static str {
        self.pick("База данных", "Database")
    }

    pub fn install_step(self, step: InstallStep) -> &'static str {
        match step {
            InstallStep::Prepare => self.pick("Подготовка", "Prepare"),
            InstallStep::Packages => self.pick("Пакеты", "Packages"),
            InstallStep::Files => self.pick("Файлы", "Files"),
            InstallStep::Services => self.pick("Сервисы", "Services"),
            InstallStep::ControlPlaneBootstrap => {
                self.pick("Bootstrap control-plane", "Control-plane bootstrap")
            }
            InstallStep::NodeBootstrap => self.pick("Bootstrap ноды", "Node bootstrap"),
            InstallStep::StarterSubscription => {
                self.pick("Starter subscription", "Starter subscription")
            }
            InstallStep::Summary => self.pick("Сводка", "Summary"),
            InstallStep::Cleanup => self.pick("Очистка", "Cleanup"),
        }
    }

    pub fn install_role(self, role: InstallRole) -> &'static str {
        match role {
            InstallRole::AllInOne => "all-in-one",
            InstallRole::ControlPlane => "control-plane",
            InstallRole::Node => "node",
        }
    }

    pub fn deployment_mode(self, mode: DeploymentMode) -> &'static str {
        match mode {
            DeploymentMode::Native => "native",
            DeploymentMode::Docker => "docker",
        }
    }

    fn pick(self, ru: &'static str, en: &'static str) -> &'static str {
        match self.language {
            Language::Ru => ru,
            Language::En => en,
        }
    }
}

pub fn terminal_supports_utf8() -> bool {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .find_map(|key| env::var(key).ok())
        .map(|value| {
            let normalized = value.to_ascii_lowercase();
            normalized.contains("utf-8") || normalized.contains("utf8")
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_detection_prefers_russian() {
        assert_eq!(Language::from_locale("ru_RU.UTF-8"), Some(Language::Ru));
        assert_eq!(Language::from_locale("en_US.UTF-8"), Some(Language::En));
        assert_eq!(Language::from_locale("de_DE.UTF-8"), None);
    }

    #[test]
    fn banner_uses_ascii_fallback() {
        assert_eq!(
            Translator::with_unicode(Language::Ru, false).banner(),
            "Anneal"
        );
        assert_eq!(Translator::new(Language::Ru).banner(), "▂▄▆█ Anneal");
    }

    #[test]
    fn russian_prompts_stay_readable() {
        let translator = Translator::with_unicode(Language::Ru, true);

        assert_eq!(translator.install_role_prompt(), "Роль установки");
        assert_eq!(translator.failed(), "Установка завершилась с ошибкой");
    }
}
