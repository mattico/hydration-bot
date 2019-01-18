//! Requires the 'framework' feature flag be enabled in your project's
//! `Cargo.toml`.
//!
//! This can be enabled by specifying the feature in the dependency section:
//!
//! ```toml
//! [dependencies.serenity]
//! git = "https://github.com/serenity-rs/serenity.git"
//! features = ["framework", "standard_framework"]
//! ```

#[macro_use]
extern crate log;
#[macro_use]
extern crate serenity;

extern crate env_logger;
extern crate kankyo;

mod commands;

use serenity::framework::StandardFramework;
use serenity::http;
use serenity::model::id::{ChannelId, UserId, GuildId};
use serenity::model::event::ResumedEvent;
use serenity::model::gateway::Ready;
use serenity::model::user::User;
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use std::collections::HashSet;
use std::env;
use std::sync::Mutex;

const PERMISSIONS: u64 = 138240;

struct Handler {
    drated_users: Mutex<HashSet<UserId>>,
    talking_users: Mutex<HashSet<UserId>>,
}

impl Handler {
    fn new() -> Self {
        Handler {
            drated_users: Mutex::new(HashSet::new()),
            talking_users: Mutex::new(HashSet::new()),
        }
    }
}

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    fn voice_state_update(&self, _ctx: Context, _: Option<GuildId>, voice: VoiceState) {
        info!("{:?}", voice);
        if voice.channel_id.is_some() {
            self.talking_users.lock().unwrap().insert(voice.user_id);
        } else {
            self.talking_users.lock().unwrap().remove(&voice.user_id);
        }
    }
}

fn main() {
    use serenity::client::validate_token;

    // This will load the environment variables located at `./.env`, relative to
    // the CWD. See `./.env.example` for an example on how to structure this.
    kankyo::load().expect("Failed to load .env file");

    // Initialize the logger to use environment variables.
    //
    // In this case, a good default is setting the environment variable
    // `RUST_LOG` to debug`.
    env_logger::init();

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    assert!(validate_token(&token).is_ok(), "Token Invalid");
    if let Ok(id) = env::var("CLIENT_ID") {
        println!("Bot Authentication URL:");
        println!("https://discordapp.com/api/oauth2/authorize?client_id={}&permissions={}&scope=bot\n\n\n", id, PERMISSIONS);
    };

    let handler = Handler::new();
    let mut client = Client::new(&token, handler).expect("Err creating client");

    let owners = match http::get_current_application_info() {
        Ok(info) => {
            let mut set = HashSet::new();
            set.insert(info.owner.id);

            set
        }
        Err(why) => panic!("Couldn't get application info: {:?}", why),
    };

    client.with_framework(
        StandardFramework::new()
            .configure(|c| c.owners(owners).prefix("~"))
            .command("quit", |c| c.cmd(commands::owner::quit).owners_only(true)),
    );

    if let Err(why) = client.start() {
        error!("Client error: {:?}", why);
    }
}
