//! High-level command routing system for WhatsApp bots.
//!
//! Inspired by Baileys and WhatsMeow bot ecosystems, `BotRouter` provides a fluent
//! pattern for registering prefix-based commands, argument parsing, automatic
//! self-message suppression, and message context helpers.
//!
//! # Example
//! ```rust,ignore
//! use whatsapp_rust::bot::{Bot, BotRouter};
//!
//! let mut router = BotRouter::new();
//! router.prefixes(&["!", "/", "."]);
//!
//! router.command("ping", |cmd| async move {
//!     cmd.react("🏓").await?;
//!     cmd.reply("Pong! 🦀 Rust bot is blazing fast!").await?;
//!     Ok(())
//! });
//!
//! router.command("poll", |cmd| async move {
//!     let options = vec!["Rust 🦀".into(), "Go 🐹".into(), "Node.js 🟩".into()];
//!     cmd.send_poll("Favorite WhatsApp Bot Language?", &options, 1).await?;
//!     Ok(())
//! });
//!
//! let bot = Bot::builder()
//!     .with_backend(backend)
//!     .with_router(router)
//!     .build()
//!     .await?;
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use crate::bot::MessageContext;

pub type RouterResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub type CommandHandler = Arc<
    dyn Fn(CommandContext) -> BoxFuture<'static, RouterResult>
        + Send
        + Sync,
>;

/// Context passed to a command handler when an incoming message matches a prefix and command name.
#[derive(Clone)]
pub struct CommandContext {
    /// Underlying [`MessageContext`] carrying message protobuf and client reference.
    pub ctx: MessageContext,
    /// The invoked command name in lowercase (e.g. "ping").
    pub command: String,
    /// Parsed command arguments split by whitespace.
    pub args: Vec<String>,
    /// Raw unparsed string following the command name.
    pub raw_args: String,
    /// The prefix used to invoke this command (e.g. "!" or "/").
    pub prefix: String,
}

impl CommandContext {
    /// Reply to the message, quoting the original message.
    pub async fn reply(
        &self,
        text: impl Into<String>,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.ctx.reply_quoting(text).await
    }

    /// Reply with plain text without quoting.
    pub async fn reply_plain(
        &self,
        text: impl Into<String>,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.ctx.reply(text).await
    }

    /// React to the incoming message with an emoji.
    pub async fn react(
        &self,
        emoji: &str,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.ctx.react(emoji).await
    }

    /// Send a poll directly to this chat.
    pub async fn send_poll(
        &self,
        name: &str,
        options: &[String],
        selectable_count: u32,
    ) -> Result<crate::send::SendResult, crate::features::PollError> {
        self.ctx.send_poll(name, options, selectable_count).await
    }

    /// Send typing presence (`composing`) to this chat.
    pub async fn send_typing(&self) -> Result<(), crate::features::ChatStateError> {
        self.ctx.send_typing().await
    }

    /// Send recording audio presence to this chat.
    pub async fn send_recording(&self) -> Result<(), crate::features::ChatStateError> {
        self.ctx.send_recording().await
    }

    /// Send paused/idle presence to this chat.
    pub async fn send_paused(&self) -> Result<(), crate::features::ChatStateError> {
        self.ctx.send_paused().await
    }

    /// Send read receipt (blue checkmarks) for this message.
    pub async fn mark_as_read(&self) -> Result<(), anyhow::Error> {
        self.ctx.mark_as_read().await
    }

    /// Get extracted text content from the message.
    pub fn text(&self) -> Option<&str> {
        self.ctx.text()
    }

    /// Whether this message originated from a group chat.
    pub fn is_group(&self) -> bool {
        self.ctx.is_group()
    }

    /// Whether this message was sent by the bot itself.
    pub fn is_from_me(&self) -> bool {
        self.ctx.is_from_me()
    }

    /// Sender JID (in groups, this is the participant JID).
    pub fn sender(&self) -> &wacore_binary::Jid {
        self.ctx.sender()
    }

    /// Chat JID (DM peer, group, or newsletter).
    pub fn chat(&self) -> &wacore_binary::Jid {
        self.ctx.chat()
    }
}

/// A router for dispatching incoming WhatsApp messages to registered command handlers.
#[derive(Clone, Default)]
pub struct BotRouter {
    prefixes: Vec<String>,
    ignore_self: bool,
    commands: HashMap<String, CommandHandler>,
    aliases: HashMap<String, String>,
    default_handler: Option<CommandHandler>,
}

impl BotRouter {
    /// Create a new router with standard default prefixes (`!`, `/`, `.`) and `ignore_self: true`.
    pub fn new() -> Self {
        Self {
            prefixes: vec!["!".to_string(), "/".to_string(), ".".to_string()],
            ignore_self: true,
            commands: HashMap::new(),
            aliases: HashMap::new(),
            default_handler: None,
        }
    }

    /// Set allowed command prefixes (e.g. `&["!", "#", "/"]`).
    pub fn prefixes(&mut self, prefixes: &[&str]) -> &mut Self {
        self.prefixes = prefixes.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add a single command prefix.
    pub fn prefix(&mut self, prefix: impl Into<String>) -> &mut Self {
        self.prefixes.push(prefix.into());
        self
    }

    /// Configure whether messages sent from this account/bot are ignored (default: `true`).
    pub fn ignore_self(&mut self, ignore: bool) -> &mut Self {
        self.ignore_self = ignore;
        self
    }

    /// Register a command handler. Command names are case-insensitive.
    pub fn command<F, Fut>(&mut self, name: impl Into<String>, handler: F) -> &mut Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RouterResult> + Send + 'static,
    {
        let cmd_name = name.into().to_lowercase();
        let boxed_handler: CommandHandler = Arc::new(move |ctx| Box::pin(handler(ctx)));
        self.commands.insert(cmd_name, boxed_handler);
        self
    }

    /// Register a command handler with one or more aliases.
    pub fn command_with_aliases<F, Fut>(
        &mut self,
        name: impl Into<String>,
        aliases: &[&str],
        handler: F,
    ) -> &mut Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RouterResult> + Send + 'static,
    {
        let primary = name.into().to_lowercase();
        let boxed_handler: CommandHandler = Arc::new(move |ctx| Box::pin(handler(ctx)));
        self.commands.insert(primary.clone(), boxed_handler);
        for alias in aliases {
            self.aliases.insert(alias.to_lowercase(), primary.clone());
        }
        self
    }

    /// Set a fallback handler invoked when a message has a valid prefix but no matched command.
    pub fn default<F, Fut>(&mut self, handler: F) -> &mut Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RouterResult> + Send + 'static,
    {
        self.default_handler = Some(Arc::new(move |ctx| Box::pin(handler(ctx))));
        self
    }

    /// Dispatch an incoming message through the router.
    ///
    /// Returns `Ok(true)` if a command matched and completed successfully,
    /// `Ok(false)` if the message was not a recognized command or was skipped,
    /// or `Err(...)` if the command execution failed.
    pub async fn dispatch(&self, ctx: MessageContext) -> RouterResult {
        if self.ignore_self && ctx.is_from_me() {
            return Ok(());
        }

        let Some(text) = ctx.text() else {
            return Ok(());
        };

        let trimmed = text.trim();
        let matched_prefix = self.prefixes.iter().find(|&p| trimmed.starts_with(p));

        let Some(prefix) = matched_prefix else {
            return Ok(());
        };

        let without_prefix = &trimmed[prefix.len()..];
        let mut parts = without_prefix.split_whitespace();
        let Some(cmd_raw) = parts.next() else {
            return Ok(());
        };

        let cmd_key = cmd_raw.to_lowercase();
        let args: Vec<String> = parts.map(String::from).collect();
        let raw_args = without_prefix[cmd_raw.len()..].trim().to_string();

        let resolved_name = self.aliases.get(&cmd_key).cloned().unwrap_or(cmd_key);

        let command_ctx = CommandContext {
            ctx,
            command: resolved_name.clone(),
            args,
            raw_args,
            prefix: prefix.clone(),
        };

        if let Some(handler) = self.commands.get(&resolved_name) {
            handler(command_ctx).await?;
            Ok(())
        } else if let Some(ref fallback) = self.default_handler {
            fallback(command_ctx).await?;
            Ok(())
        } else {
            Ok(())
        }
    }
}
