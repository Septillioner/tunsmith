//! TTY-aware CLI chrome. Roles and labels only; no per-line rainbow.
//!
//! `console` omits ANSI when stdout is not a TTY or when `NO_COLOR` is set.
//! Dialoguer prompts use stderr and the same crate's color detection.

use std::fmt;

use console::{style, Style};
use dialoguer::theme::ColorfulTheme;

const TAG_WIDTH: usize = 8;
const LABEL_WIDTH: usize = 16;

pub const STAGE_PKI: &str = "pki";
pub const STAGE_BUILD: &str = "build";
pub const STAGE_CERTS: &str = "certs";
pub const STAGE_SSH: &str = "ssh";
pub const STAGE_PROBE: &str = "probe";
pub const STAGE_DEPS: &str = "deps";
pub const STAGE_NET: &str = "net";
pub const STAGE_DEPLOY: &str = "deploy";
pub const STAGE_UPDATE: &str = "update";
pub const STAGE_CLEAN: &str = "clean";

const TAG_ERROR: &str = "error";
const TAG_WARN: &str = "warn";
const TAG_OK: &str = "ok";

const PROMPT_PREFIX: &str = ">";
const SUCCESS_PREFIX: &str = "+";
const ERROR_PREFIX: &str = "x";
const PROMPT_SUFFIX: &str = ":";
const ACTIVE_ITEM_PREFIX: &str = ">";
const INACTIVE_ITEM_PREFIX: &str = " ";
const CHECKED_ITEM_PREFIX: &str = "[x]";
const UNCHECKED_ITEM_PREFIX: &str = "[ ]";

pub fn theme() -> ColorfulTheme {
    ColorfulTheme {
        defaults_style: Style::new().for_stderr().cyan(),
        prompt_style: Style::new().for_stderr().bold(),
        prompt_prefix: style(PROMPT_PREFIX.to_string()).for_stderr().cyan().bold(),
        prompt_suffix: style(PROMPT_SUFFIX.to_string()).for_stderr().dim(),
        success_prefix: style(SUCCESS_PREFIX.to_string()).for_stderr().green(),
        success_suffix: style(String::new()).for_stderr(),
        error_prefix: style(ERROR_PREFIX.to_string()).for_stderr().red().bold(),
        error_style: Style::new().for_stderr().red().bold(),
        hint_style: Style::new().for_stderr().dim(),
        values_style: Style::new().for_stderr().cyan(),
        active_item_style: Style::new().for_stderr().cyan().bold(),
        inactive_item_style: Style::new().for_stderr(),
        active_item_prefix: style(ACTIVE_ITEM_PREFIX.to_string()).for_stderr().cyan(),
        inactive_item_prefix: style(INACTIVE_ITEM_PREFIX.to_string()).for_stderr(),
        checked_item_prefix: style(CHECKED_ITEM_PREFIX.to_string()).for_stderr().green(),
        unchecked_item_prefix: style(UNCHECKED_ITEM_PREFIX.to_string()).for_stderr().dim(),
        picked_item_prefix: style(ACTIVE_ITEM_PREFIX.to_string()).for_stderr().cyan(),
        unpicked_item_prefix: style(INACTIVE_ITEM_PREFIX.to_string()).for_stderr(),
    }
}

pub fn error(message: impl AsRef<str>) {
    println!(
        "{} {}",
        style(pad_tag(TAG_ERROR)).red().bold(),
        message.as_ref()
    );
}

pub fn warn(message: impl AsRef<str>) {
    println!(
        "{} {}",
        style(pad_tag(TAG_WARN)).yellow().bold(),
        message.as_ref()
    );
}

pub fn success(message: impl AsRef<str>) {
    println!(
        "{} {}",
        style(pad_tag(TAG_OK)).green().bold(),
        message.as_ref()
    );
}

pub fn info(message: impl AsRef<str>) {
    println!("{}", style(message.as_ref()).dim());
}

pub fn step(stage: &str, message: impl AsRef<str>) {
    println!(
        "{} {}",
        style(pad_tag(stage)).cyan().bold(),
        message.as_ref()
    );
}

pub fn detail(message: impl AsRef<str>) {
    println!(
        "{} {}",
        " ".repeat(TAG_WIDTH),
        style(message.as_ref()).dim()
    );
}

pub fn heading(title: &str) {
    println!();
    println!("{}", style(title).cyan().bold());
}

pub fn field(label: &str, value: impl fmt::Display) {
    let label = style(format!("{label:<LABEL_WIDTH$}")).dim();
    println!("{label} {value}");
}

pub fn key_field(label: &str, value: impl fmt::Display) {
    let label = style(format!("{label:<LABEL_WIDTH$}")).cyan().bold();
    println!("{label} {value}");
}

pub fn warn_value(value: impl AsRef<str>) -> console::StyledObject<String> {
    style(value.as_ref().to_string()).yellow()
}

pub fn rule(width: usize) {
    println!("{}", style("-".repeat(width)).dim());
}

fn pad_tag(tag: &str) -> String {
    format!("{tag:>TAG_WIDTH$}")
}
