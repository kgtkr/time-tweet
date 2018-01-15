extern crate chrono;
#[macro_use]
extern crate clap;
extern crate cron;
extern crate egg_mode;
extern crate futures;
extern crate tokio_core;
use clap::{App, Arg};
use tokio_core::reactor::Core;
use chrono::prelude::*;
use chrono::Duration;
use std::thread;
use cron::Schedule;
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
                .default_value("${H}${M}"),
        )
        .arg(
            Arg::with_name("time")
                .help("ツイート時間。cronを指定(秒と年の拡張あり)")
                .long("time")
                .short("t")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("test_time")
                .help("本番何秒前にテストツイートを行うか")
                .long("test-time")
                .alias("tt")
                .takes_value(true)
                .default_value("10"),
        );

    let matches = app.get_matches();

    let ck = matches.value_of("consumer_key").unwrap();
    let cs = matches.value_of("consumer_secret").unwrap();
    let tk = matches.value_of("token_key").unwrap();
    let ts = matches.value_of("token_secret").unwrap();
    let msg = matches.value_of("msg").unwrap();
    let time = value_t!(matches, "time", Schedule).unwrap_or_else(|e| e.exit());
    let test_time = value_t!(matches, "test_time", u32).unwrap_or_else(|e| e.exit());

    let token = egg_mode::Token::Access {
        consumer: egg_mode::KeyPair::new(ck.to_string(), cs.to_string()),
        access: egg_mode::KeyPair::new(tk.to_string(), ts.to_string()),
    };

    for tweet_date_local in time.upcoming(Local) {
        println!("Next:{}", tweet_date_local);
        let tweet_date = tweet_date_local.with_timezone(&Utc);
        let msg = msg.replace("${H}", &format!("{:>02}", tweet_date_local.hour()));
        let msg = msg.replace("${M}", &format!("{:>02}", tweet_date_local.minute()));

        let test_tweet_date = tweet_date
            .checked_sub_signed(Duration::seconds(test_time as i64))
            .unwrap();

        let result = {
            test_tweet_date
                .signed_duration_since(Utc::now())
                .to_std()
                .map_err(|_| "既に過ぎている")
                .and_then(|wait| {
                    thread::sleep(wait);
                    tweet(&test_tweet_date.with_timezone(&Local).to_string(), &token)
                        .map_err(|_| "ツイートに失敗")
                })
                .map(|date| date.signed_duration_since(test_tweet_date))
        }.and_then(|diff| {
            println!("diff:{}", diff);
            tweet_date
                .signed_duration_since(Utc::now() - diff)
                .to_std()
                .map_err(|_| "既に過ぎている")
                .and_then(|wait| {
                    thread::sleep(wait);
                    tweet(&msg, &token)
                        .map_err(|_| "ツイートに失敗")
                        .map(|date| {
                            println!(
                                "本番:{}",
                                date.with_timezone(&Local)
                                    .format("%Y-%m-%d %H:%M:%S.%f")
                                    .to_string()
                            );
                        })
                })
        });

        if let Err(msg) = result {
            eprintln!("{}", msg);
        }
    }
}

fn tweet(msg: &str, token: &egg_mode::Token) -> Result<DateTime<Utc>, egg_mode::error::Error> {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    core.run(egg_mode::tweet::DraftTweet::new(msg).send(token, &handle))
        .map(|res| tweet_id_to_date(res.response.id))
}

fn tweet_id_to_date(id: u64) -> DateTime<Utc> {
    let ms = ((id >> 22) + 1288834974657) as i64;
    Utc.timestamp(ms / 1000, ((ms % 1000) * 1000000) as u32)
}
