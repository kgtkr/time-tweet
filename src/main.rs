#![recursion_limit = "1024"]

extern crate chrono;
#[macro_use]
extern crate clap;
extern crate cron;
extern crate egg_mode;
extern crate futures;
extern crate iter_merge_sort;
extern crate time;
extern crate tokio_core;
use clap::{App, Arg};
use tokio_core::reactor::Core;
use chrono::prelude::*;
use chrono::Duration;
use std::thread;
use cron::Schedule;
use iter_merge_sort::*;
#[macro_use]
extern crate error_chain;

const FORMAT: &str = "%Y-%m-%d %H:%M:%S%.3f";

fn main() {
    let app = App::new("time-tweet")
        .version(env!("CARGO_PKG_VERSION"))
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
            Arg::with_name("times")
                .help("ツイート時間。cronを指定(秒と年の拡張あり)")
                .long("times")
                .short("t")
                .takes_value(true)
                .required(true)
                .multiple(true),
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
    let times = values_t!(matches, "times", Schedule).unwrap_or_else(|e| e.exit());
    let test_time = value_t!(matches, "test_time", u32).unwrap_or_else(|e| e.exit());

    let token = egg_mode::Token::Access {
        consumer: egg_mode::KeyPair::new(ck.to_string(), cs.to_string()),
        access: egg_mode::KeyPair::new(tk.to_string(), ts.to_string()),
    };

    for tweet_date_local in times
        .iter()
        .map(|time| time.upcoming(Local))
        .collect::<Vec<_>>()
        .merge_sort(false)
    {
        println!("Next:{}", tweet_date_local);
        let tweet_date = tweet_date_local.with_timezone(&Utc);
        let msg = msg.replace("${H}", &tweet_date_local.hour().to_string());
        let msg = msg.replace("${M}", &format!("{:>02}", tweet_date_local.minute()));

        let pre_test_tweet_date = tweet_date
            .checked_sub_signed(Duration::seconds((test_time * 2) as i64))
            .unwrap();

        let test_tweet_date = tweet_date
            .checked_sub_signed(Duration::seconds(test_time as i64))
            .unwrap();

        println!("【プレテスト】");
        let result = time_tweet(
            &pre_test_tweet_date.with_timezone(&Local).to_string(),
            &token,
            true,
            &pre_test_tweet_date,
        ).and_then(|_| {
            println!("【テスト】");
            time_tweet(
                &test_tweet_date.with_timezone(&Local).to_string(),
                &token,
                true,
                &test_tweet_date,
            )
        })
            .map(|date| date.signed_duration_since(test_tweet_date))
            .and_then(|diff| {
                println!("【本番】");
                time_tweet(&msg, &token, false, &(tweet_date - diff))
            })
            .and_then(|date| {
                println!("【リザルト】");
                time_log(&tweet_date, &date);
                tweet(
                    &format!(
                        "{} {}ms",
                        date.with_timezone(&Local).format(FORMAT),
                        date.signed_duration_since(tweet_date.clone())
                            .num_milliseconds()
                    ),
                    &token,
                    false,
                ).map_err(|e| time_tweet_error::Error::from(e))
            });

        if let Err(err) = result {
            match err {
                time_tweet_error::Error(time_tweet_error::ErrorKind::TwitterError(err), _) => {
                    eprintln!("ツイートエラー:{}", err)
                }

                time_tweet_error::Error(time_tweet_error::ErrorKind::LateDateError(_), _) => {
                    eprintln!("既に過ぎています")
                }

                _ => eprintln!("何らかのエラー"),
            }
        }
    }
}

mod time_tweet_error {
    error_chain! {
        foreign_links {
            TwitterError(::egg_mode::error::Error);
            LateDateError(::time::OutOfRangeError);
        }
    }
}

fn time_log(tweet_date: &DateTime<Utc>, date: &DateTime<Utc>) {
    let tweet_date = tweet_date.with_timezone(&Local);
    let date = date.with_timezone(&Local);
    println!("予定:{}", tweet_date.format(FORMAT));
    println!("実際:{}", date.format(FORMAT));
    let diff = date.signed_duration_since(tweet_date.clone());
    println!("Diff:{}ms", diff.num_milliseconds());
}

fn time_tweet(
    msg: &str,
    token: &egg_mode::Token,
    remove: bool,
    tweet_date: &DateTime<Utc>,
) -> time_tweet_error::Result<DateTime<Utc>> {
    let wait = tweet_date.signed_duration_since(Utc::now()).to_std()?;
    thread::sleep(wait);
    let date = tweet(msg, &token, remove)?;
    time_log(tweet_date, &date);
    Ok(date)
}

fn tweet(
    msg: &str,
    token: &egg_mode::Token,
    remove: bool,
) -> Result<DateTime<Utc>, egg_mode::error::Error> {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let res = core.run(egg_mode::tweet::DraftTweet::new(msg).send(token, &handle))?;
    if remove {
        core.run(egg_mode::tweet::delete(res.response.id, token, &handle))?;
    }
    Ok(tweet_id_to_date(res.response.id))
}

fn tweet_id_to_date(id: u64) -> DateTime<Utc> {
    let ms = ((id >> 22) + 1288834974657) as i64;
    Utc.timestamp(ms / 1000, ((ms % 1000) * 1_000_000) as u32)
}
