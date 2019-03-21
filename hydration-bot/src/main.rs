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

use serenity::client::bridge::gateway::ShardManager;
use serenity::framework::standard::ArgError;
use serenity::framework::StandardFramework;
use serenity::http;
use serenity::model::channel::Message;
use serenity::model::event::ResumedEvent;
use serenity::model::gateway::Ready;
use serenity::model::id::{GuildId, UserId};
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use typemap::Key;

const PERMISSIONS: u64 = 50469888;

static RUN: AtomicBool = AtomicBool::new(true);

fn reply(msg: &Message, data: &str) {
    let _ = msg
        .reply(data)
        .map_err(|e| error!("Unable to send reply message: {}", e));
}

command!(drate(ctx, msg, args) {
    fn drate_on(msg: &Message, drated: &mut HashMap<UserId, Instant>) {
        drated.insert(msg.author.id, Instant::now());
        reply(msg, "Enabled drate reminders");
    }

    fn drate_off(msg: &Message, drated: &mut HashMap<UserId, Instant>) {
        drated.remove(&msg.author.id);
        reply(msg, "Disabled drate reminders");
    }

    let mut data = ctx.data.lock();
    let mut drated = data.get_mut::<DratedUsers>().unwrap();

    match args.single::<String>() {
        Err(ArgError::Eos) => drate_on(msg, &mut drated),
        Ok(ref arg) if arg == "on" => drate_on(msg, &mut drated),
        Ok(ref arg) if arg == "off" => drate_off(msg, &mut drated),
        _ => reply(msg, "Unknown argument. Usage: `!drate [on|off]`"),
    };
});

command!(quit(ctx, msg, _args) {
    reply(msg, "Shutting down!");
    RUN.store(false, Ordering::Relaxed);
    let mut data = ctx.data.lock();
    let shard_manager = data.get_mut::<ShardManagerContainer>().unwrap().clone();
    let mut shard_manager = shard_manager.lock();
    shard_manager.shutdown_all();
});

struct ShardManagerContainer;

impl Key for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct DratedUsers;

impl Key for DratedUsers {
    type Value = HashMap<UserId, Instant>;
}

struct TalkingUsers;

impl Key for TalkingUsers {
    type Value = HashSet<UserId>;
}

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    fn voice_state_update(&self, ctx: Context, _: Option<GuildId>, voice: VoiceState) {
        info!("{:?}", voice);

        let mut data = ctx.data.lock();
        let talking = data.get_mut::<TalkingUsers>().unwrap();
        if voice.channel_id.is_some() {
            talking.insert(voice.user_id);
        } else {
            talking.remove(&voice.user_id);
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

    let mut client = Client::new(&token, Handler).expect("Err creating client");

    {
        let mut data = client.data.lock();
        data.insert::<DratedUsers>(HashMap::new());
        data.insert::<TalkingUsers>(HashSet::new());
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
    }

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
            .configure(|c| c.owners(owners).prefix("!"))
            .command("drate", |c| c.cmd(drate))
            .command("quit", |c| c.cmd(quit).owners_only(true)),
    );

    let data = client.data.clone();

    let remind_thread = thread::spawn(move || {
        info!("Drate remind thread running");
        while RUN.load(Ordering::Relaxed) {
            {
                let mut data = data.lock();
                let mut drated_users = data.get_mut::<DratedUsers>().unwrap();
                if let Err(err) = remind(&mut drated_users) {
                    error!("Drate remind error: {:?}", err);
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
        info!("Drate remind thread shutting down");
    });

    if let Err(err) = client.start_autosharded() {
        error!("Client error: {:?}", err);
    }

    if let Err(err) = remind_thread.join() {
        error!("Drate remind thread error: {:?}", err);
    }
}

fn remind(drated_users: &mut HashMap<UserId, Instant>) -> Result<(), Box<dyn Error>> {
    let now = Instant::now();
    let drate_duration = Duration::from_secs(60 * 30);

    for (user, time) in drated_users.iter_mut() {
        if now.duration_since(*time) >= drate_duration {
            info!("Reminding user {:?} to stay drated", *user);
            *time = now;
            let private_channel = user.create_dm_channel()?;
            private_channel.send_message(|m| m.content("Drink Water").tts(true))?;
        }
    }

    Ok(())
}
