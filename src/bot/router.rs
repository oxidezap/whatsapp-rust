//! High-level command routing system for WhatsApp bots.
//!
//! Inspired by Baileys and WhatsMeow bot ecosystems, `BotRouter` provides a fluent
//! pattern for registering prefix-based commands, argument parsing, automatic
//! self-message suppression, admin guards, and group management helpers.
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
//! // Only group admins can run this:
//! router.admin_command("tagall", |cmd| async move {
//!     cmd.mention_all(Some("Attention all members:")).await?;
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
use wacore_binary::{Jid, JidExt};

pub type RouterResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub type CommandHandler = Arc<
    dyn Fn(CommandContext) -> BoxFuture<'static, RouterResult>
        + Send
        + Sync,
>;

/// Configuration rules and restrictions for a bot command.
#[derive(Clone, Debug, Default)]
pub struct CommandConfig {
    /// Only group administrators can invoke this command.
    pub admin_only: bool,
    /// Command can only be invoked inside group chats.
    pub group_only: bool,
    /// Command can only be invoked in private direct messages (DMs).
    pub dm_only: bool,
    /// Human-readable documentation for this command.
    pub description: String,
}

#[derive(Clone)]
struct RegisteredCommand {
    handler: CommandHandler,
    config: CommandConfig,
}

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
    ) -> Result<crate::features::CreatedPoll, crate::features::PollError> {
        self.ctx.send_poll(name, options, selectable_count).await
    }

    /// Reply with user mentions attached in the context info.
    pub async fn reply_with_mentions(
        &self,
        text: impl Into<String>,
        mentions: &[Jid],
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.ctx.reply_with_mentions(text, mentions).await
    }

    /// Mentions all participants in the group chat (the classic hidetag / tagall).
    pub async fn mention_all(
        &self,
        header: Option<&str>,
    ) -> Result<crate::send::SendResult, Box<dyn std::error::Error + Send + Sync>> {
        self.ctx.mention_all(header).await
    }

    /// Fetch group metadata for the current chat.
    pub async fn group_metadata(&self) -> Result<crate::features::GroupMetadata, crate::features::GroupError> {
        self.ctx.group_metadata().await
    }

    /// Checks if the message sender is an admin in this group.
    pub async fn is_sender_admin(&self) -> Result<bool, crate::features::GroupError> {
        self.ctx.is_sender_admin().await
    }

    /// Checks if the bot itself is an admin in this group.
    pub async fn is_bot_admin(&self) -> Result<bool, crate::features::GroupError> {
        self.ctx.is_bot_admin().await
    }

    /// Kick/remove participants from the current group.
    pub async fn group_kick(&self, participants: &[Jid]) -> Result<Vec<crate::features::ParticipantChangeResponse>, crate::features::GroupError> {
        self.ctx.group_kick(participants).await
    }

    /// Add participants to the current group.
    pub async fn group_add(&self, participants: &[Jid]) -> Result<Vec<crate::features::ParticipantChangeResponse>, crate::features::GroupError> {
        self.ctx.group_add(participants).await
    }

    /// Promote participants to group admins.
    pub async fn group_promote(&self, participants: &[Jid]) -> Result<Vec<crate::features::ParticipantChangeResponse>, crate::features::GroupError> {
        self.ctx.group_promote(participants).await
    }

    /// Demote group admins back to regular members.
    pub async fn group_demote(&self, participants: &[Jid]) -> Result<Vec<crate::features::ParticipantChangeResponse>, crate::features::GroupError> {
        self.ctx.group_demote(participants).await
    }

    /// Retrieve the group invite link code.
    pub async fn group_invite_code(&self) -> Result<String, crate::features::GroupError> {
        self.ctx.group_invite_code().await
    }

    /// Send a location pin message.
    pub async fn reply_location(
        &self,
        latitude: f64,
        longitude: f64,
        name: Option<String>,
        address: Option<String>,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.ctx.reply_location(latitude, longitude, name, address).await
    }

    /// Send a contact card (vCard) message.
    pub async fn reply_contact(
        &self,
        display_name: impl Into<String>,
        vcard: impl Into<String>,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.ctx.reply_contact(display_name, vcard).await
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
    pub fn sender(&self) -> &Jid {
        self.ctx.sender()
    }

    /// Sender phone number string (without server suffix).
    pub fn sender_phone(&self) -> &str {
        &self.ctx.sender().user
    }

    /// Chat JID (DM peer, group, or newsletter).
    pub fn chat(&self) -> &Jid {
        self.ctx.chat()
    }
}

/// A router for dispatching incoming WhatsApp messages to registered command handlers.
#[derive(Clone, Default)]
pub struct BotRouter {
    prefixes: Vec<String>,
    ignore_self: bool,
    commands: HashMap<String, RegisteredCommand>,
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

    /// Register a command handler with default configuration.
    pub fn command<F, Fut>(&mut self, name: impl Into<String>, handler: F) -> &mut Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RouterResult> + Send + 'static,
    {
        self.command_with_config(name, CommandConfig::default(), handler)
    }

    /// Register an admin-only command (only group administrators can invoke).
    pub fn admin_command<F, Fut>(&mut self, name: impl Into<String>, handler: F) -> &mut Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RouterResult> + Send + 'static,
    {
        let config = CommandConfig {
            admin_only: true,
            group_only: true,
            dm_only: false,
            description: "Admin only command".to_string(),
        };
        self.command_with_config(name, config, handler)
    }

    /// Register a group-only command (cannot be invoked in private chat).
    pub fn group_command<F, Fut>(&mut self, name: impl Into<String>, handler: F) -> &mut Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RouterResult> + Send + 'static,
    {
        let config = CommandConfig {
            admin_only: false,
            group_only: true,
            dm_only: false,
            description: "Group only command".to_string(),
        };
        self.command_with_config(name, config, handler)
    }

    /// Register a DM-only command (cannot be invoked in groups).
    pub fn dm_command<F, Fut>(&mut self, name: impl Into<String>, handler: F) -> &mut Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RouterResult> + Send + 'static,
    {
        let config = CommandConfig {
            admin_only: false,
            group_only: false,
            dm_only: true,
            description: "Direct message only command".to_string(),
        };
        self.command_with_config(name, config, handler)
    }

    /// Register a command handler with custom restrictions.
    pub fn command_with_config<F, Fut>(
        &mut self,
        name: impl Into<String>,
        config: CommandConfig,
        handler: F,
    ) -> &mut Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RouterResult> + Send + 'static,
    {
        let cmd_name = name.into().to_lowercase();
        let boxed_handler: CommandHandler = Arc::new(move |ctx| Box::pin(handler(ctx)));
        self.commands.insert(cmd_name, RegisteredCommand {
            handler: boxed_handler,
            config,
        });
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
        self.commands.insert(primary.clone(), RegisteredCommand {
            handler: boxed_handler,
            config: CommandConfig::default(),
        });
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
            ctx: ctx.clone(),
            command: resolved_name.clone(),
            args,
            raw_args,
            prefix: prefix.clone(),
        };

        if let Some(reg) = self.commands.get(&resolved_name) {
            // Guard: Group-only check
            if reg.config.group_only && !command_ctx.is_group() {
                command_ctx.reply("⚠️ Perintah ini hanya dapat digunakan di dalam Grup!").await?;
                return Ok(());
            }

            // Guard: DM-only check
            if reg.config.dm_only && command_ctx.is_group() {
                command_ctx.reply("⚠️ Perintah ini hanya dapat digunakan di Private Chat (DM)!").await?;
                return Ok(());
            }

            // Guard: Admin-only check
            if reg.config.admin_only {
                let is_admin = match command_ctx.is_sender_admin().await {
                    Ok(val) => val,
                    Err(e) => {
                        log::warn!(target: "Bot/Router", "Failed to check admin status: {e:?}");
                        false
                    }
                };
                if !is_admin {
                    command_ctx.reply("⛔ Perintah ini hanya dapat dijalankan oleh Admin Grup!").await?;
                    return Ok(());
                }
            }

            (reg.handler)(command_ctx).await?;
            Ok(())
        } else if let Some(ref fallback) = self.default_handler {
            fallback(command_ctx).await?;
            Ok(())
        } else {
            Ok(())
        }
    }
}
