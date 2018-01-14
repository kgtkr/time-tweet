extern crate chrono;
#[macro_use]
extern crate clap;
extern crate egg_mode;
extern crate futures;
extern crate tokio_core;

use clap::{App, Arg};
use tokio_core::reactor::Core;
use chrono::prelude::*;
use std::{thread, time};

fn main() {
    let app = App::new("time-tweet")
        .version("0.1.0")
        .author("tkr <kgtkr.jp@gmail.com>")
        .about("正確な時間にツイート")
        .arg(
            Arg::with_name("consumer_key")
                .help("コンシューマーキー")
                .long("consumer-key")
                .visible_alias("ck")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("consumer_secret")
                .help("コンシューマーシークレット")
                .long("consumer-secret")
                .visible_alias("cs")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("token_key")
                .help("トークンキー")
                .long("token_key")
                .visible_alias("tk")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("token_secret")
                .help("トークンシークレット")
                .long("token-secret")
                .visible_alias("ts")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("msg")
                .help("ツイート内容")
                .long("msg")
                .short("m")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("hour")
                .help("時")
                .long("#![feature(i128_type)]hour")
                .short("H")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("minute")
                .help("分")
                .long("minute")
                .short("M")
                .takes_value(true)
                .required(true),
        );

    let matches = app.get_matches();
    let ck = matches.value_of("consumer_key").unwrap();
    let cs = matches.value_of("consumer_secret").unwrap();
    let tk = matches.value_of("token_key").unwrap();
    let ts = matches.value_of("token_secret").unwrap();
    let msg = matches.value_of("msg").unwrap();
    let tweet_date = {
        let hour = value_t!(matches, "hour", u32).unwrap_or_else(|e| e.exit());
        let minute = value_t!(matches, "minute", u32).unwrap_or_else(|e| e.exit());
        Local::today().and_hms(hour, minute, 0).with_timezone(&Utc)
    };

    let token = egg_mode::Token::Access {
        consumer: egg_mode::KeyPair::new(ck.to_string(), cs.to_string()),
        access: egg_mode::KeyPair::new(tk.to_string(), ts.to_string()),
    };

    let diff = {
        let now = Utc::now();
        let date = tweet(&now.with_timezone(&Local).to_string(), &token);

        timestamp_millis(&date) - timestamp_millis(&now)
    };
    println!("diff:{}", diff);

    thread::sleep(time::Duration::from_millis(
        (timestamp_millis(&tweet_date) - timestamp_millis(&Utc::now()) - diff) as u64,
    ));

    let date = tweet(msg, &token);
    println!(
        "本番:{}",
        date.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S.%f")
            .to_string()
    );
}

fn tweet(msg: &str, token: &egg_mode::Token) -> DateTime<Utc> {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    tweet_id_to_date(
        core.run(egg_mode::tweet::DraftTweet::new(msg).send(token, &handle))
            .unwrap()
            .response
            .id,
    )
}

fn tweet_id_to_date(id: u64) -> DateTime<Utc> {
    let ms = ((id >> 22) + 1288834974657) as i64;
    Utc.timestamp(ms / 1000, ((ms % 1000) * 1000000) as u32)
}

fn timestamp_millis(date: &DateTime<Utc>) -> i64 {
    date.timestamp() * 1000 + (date.timestamp_subsec_millis() as i64)
}
